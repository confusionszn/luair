use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use candle_core::{quantized::gguf_file, Device, Tensor};
use candle_transformers::{
    generation::LogitsProcessor, models::quantized_qwen2, utils::apply_repeat_penalty,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokenizers::Tokenizer;

use crate::{
    cloak, hotkeys,
    schema::{CrosshairConfig, CrosshairProfile, LuairState, ThemeConfig},
    state::AppState,
    storage,
};

pub const DEFAULT_SUPPORT_EMAIL: &str = "luairsupport@gmail.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArch {
    Qwen2,
}

impl ModelArch {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelArch::Qwen2 => "qwen2",
        }
    }
}

pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub arch: ModelArch,
    pub quantization: &'static str,
    pub approx_size_mb: u32,
    pub gguf_url: &'static str,
    pub tokenizer_url: &'static str,
    pub description: &'static str,
}

pub const MODEL_CATALOG: &[ModelSpec] = &[ModelSpec {
    id: "luair-candle",
    name: "Luair Candle",
    arch: ModelArch::Qwen2,
    quantization: "Q4_K_M",
    approx_size_mb: 475,
    gguf_url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf",
    tokenizer_url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/main/tokenizer.json",
    description: "Embedded local Candle AI engine for private offline chat, crosshair creation, and theme design. No API keys or cloud servers required.",
}];

pub fn build_http_agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .max_redirects(10)
        .timeout_global(None)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(45)))
        .user_agent("Luair/2.0.4")
        .build();
    config.into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelInfo {
    pub id: String,
    pub name: String,
    pub architecture: String,
    pub quantization: String,
    pub approx_size_mb: u32,
    pub description: String,
    pub downloaded: bool,
    pub loaded: bool,
    pub local_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEngineStatus {
    pub models: Vec<AiModelInfo>,
    pub loaded_model_id: Option<String>,
    pub downloading_model_id: Option<String>,
    pub active_backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDownloadProgress {
    pub model_id: String,
    pub stage: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatResponse {
    pub reply: String,
    pub engine_used: String,
    pub applied_action: Option<String>,
    pub updated_state: Option<LuairState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiApplyResult {
    pub summary: String,
    pub engine_used: String,
    pub bound_hotkey: Option<String>,
    pub state: LuairState,
}

pub struct LoadedCandleModel {
    pub model_id: String,
    pub model_name: String,
    weights: quantized_qwen2::ModelWeights,
    tokenizer: Tokenizer,
    eos_tokens: Vec<u32>,
}

fn deduplicate_history<'a>(history: &'a [ChatMessage], user_prompt: &str) -> &'a [ChatMessage] {
    let trimmed = user_prompt.trim();
    let mut end = history.len();
    while end > 0 {
        let last = &history[end - 1];
        if last.role == "user" && last.content.trim() == trimmed {
            end -= 1;
        } else {
            break;
        }
    }
    &history[..end]
}

fn format_chatml_turn(role: &str, content: &str) -> String {
    let norm_role = if role == "assistant" {
        "assistant"
    } else {
        "user"
    };
    format!("<|im_start|>{norm_role}\n{content}<|im_end|>\n")
}

#[cfg(test)]
pub fn build_chatml_prompt(
    system_prompt: &str,
    history: &[ChatMessage],
    user_prompt: &str,
) -> String {
    let deduped = deduplicate_history(history, user_prompt);
    let recent = &deduped[deduped.len().saturating_sub(6)..];
    let mut formatted = format!("<|im_start|>system\n{system_prompt}<|im_end|>\n");
    for msg in recent {
        formatted.push_str(&format_chatml_turn(&msg.role, &msg.content));
    }
    formatted.push_str(&format!(
        "<|im_start|>user\n{user_prompt}<|im_end|>\n<|im_start|>assistant\n"
    ));
    formatted
}

impl LoadedCandleModel {
    pub fn load_from_dir(spec: &ModelSpec, dir: &Path) -> Result<Self, String> {
        let gguf_path = dir.join("model.gguf");
        let tok_path = dir.join("tokenizer.json");

        if !gguf_path.exists() || !tok_path.exists() {
            return Err(format!(
                "{} is incomplete on disk. Please download it first.",
                spec.name
            ));
        }

        let mut file = File::open(&gguf_path).map_err(|e| format!("Failed opening model weights: {e}"))?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| format!("Failed parsing GGUF header: {e}"))?;

        let device = Device::Cpu;
        let weights = match spec.arch {
            ModelArch::Qwen2 => quantized_qwen2::ModelWeights::from_gguf(content, &mut file, &device)
                .map_err(|e| format!("Failed loading {} weights: {e}", spec.name))?,
        };

        let tokenizer = Tokenizer::from_file(&tok_path)
            .map_err(|e| format!("Failed loading tokenizer: {e}"))?;

        let mut eos_tokens = Vec::new();
        for tok in ["<|im_end|>", "<|endoftext|>", "</s>", "<|eot_id|>"] {
            if let Some(id) = tokenizer.token_to_id(tok) {
                if !eos_tokens.contains(&id) {
                    eos_tokens.push(id);
                }
            }
        }

        Ok(Self {
            model_id: spec.id.to_string(),
            model_name: spec.name.to_string(),
            weights,
            tokenizer,
            eos_tokens,
        })
    }

    pub fn generate(
        &mut self,
        system_prompt: &str,
        history: &[ChatMessage],
        user_prompt: &str,
        max_tokens: usize,
        temperature: f64,
    ) -> Result<String, String> {
        const MAX_PROMPT_TOKENS: usize = 1400;

        let deduped = deduplicate_history(history, user_prompt);
        let recent = &deduped[deduped.len().saturating_sub(6)..];

        let head = format!("<|im_start|>system\n{system_prompt}<|im_end|>\n");
        let tail = format!("<|im_start|>user\n{user_prompt}<|im_end|>\n<|im_start|>assistant\n");

        let base_prompt = format!("{head}{tail}");
        let base_count = self
            .tokenizer
            .encode(base_prompt.as_str(), true)
            .map_err(|e| format!("Tokenization error: {e}"))?
            .get_ids()
            .len();

        let mut selected_turns: Vec<String> = Vec::new();
        let mut running_tokens = base_count;
        for msg in recent.iter().rev() {
            let turn = format_chatml_turn(&msg.role, &msg.content);
            let turn_tokens = self
                .tokenizer
                .encode(turn.as_str(), true)
                .map_err(|e| format!("Tokenization error: {e}"))?
                .get_ids()
                .len();
            if running_tokens + turn_tokens > MAX_PROMPT_TOKENS {
                break;
            }
            running_tokens += turn_tokens;
            selected_turns.push(turn);
        }
        selected_turns.reverse();

        let mut formatted = head;
        for turn in selected_turns {
            formatted.push_str(&turn);
        }
        formatted.push_str(&tail);

        let encoding = self
            .tokenizer
            .encode(formatted, true)
            .map_err(|e| format!("Tokenization error: {e}"))?;
        let mut prompt_tokens = encoding.get_ids().to_vec();
        if prompt_tokens.is_empty() {
            return Err("Empty prompt tokens".to_string());
        }
        if prompt_tokens.len() > MAX_PROMPT_TOKENS {
            prompt_tokens = prompt_tokens[prompt_tokens.len() - MAX_PROMPT_TOKENS..].to_vec();
        }

        self.weights.clear_kv_cache();
        let device = Device::Cpu;
        let prompt_len = prompt_tokens.len();

        let input = Tensor::new(prompt_tokens.as_slice(), &device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| format!("Tensor error: {e}"))?;
        let logits = self
            .weights
            .forward(&input, 0)
            .map_err(|e| format!("Forward pass error: {e}"))?;
        let mut logits = logits
            .squeeze(0)
            .map_err(|e| format!("Logits squeeze error: {e}"))?;

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ 0x5A17_9C21)
            .unwrap_or(42);
        let safe_temp = temperature.clamp(0.05, 1.5);
        let mut logits_proc = LogitsProcessor::new(seed, Some(safe_temp), Some(0.9));

        let mut generated: Vec<u32> = Vec::new();
        let mut next_token = logits_proc
            .sample(&logits)
            .map_err(|e| format!("Sampling error: {e}"))?;

        if !self.eos_tokens.contains(&next_token) {
            generated.push(next_token);

            for step in 0..max_tokens.saturating_sub(1) {
                let tok_tensor = Tensor::new(&[next_token], &device)
                    .and_then(|t| t.unsqueeze(0))
                    .map_err(|e| format!("Token tensor error: {e}"))?;
                logits = self
                    .weights
                    .forward(&tok_tensor, prompt_len + step)
                    .and_then(|l| l.squeeze(0))
                    .map_err(|e| format!("Decode step error: {e}"))?;

                let ctx_start = generated.len().saturating_sub(64);
                let penalized = apply_repeat_penalty(&logits, 1.12, &generated[ctx_start..])
                    .map_err(|e| format!("Repeat penalty error: {e}"))?;

                next_token = logits_proc
                    .sample(&penalized)
                    .map_err(|e| format!("Sample step error: {e}"))?;

                if self.eos_tokens.contains(&next_token) {
                    break;
                }
                generated.push(next_token);
            }
        }

        let decoded = self
            .tokenizer
            .decode(&generated, true)
            .map_err(|e| format!("Decode UTF-8 error: {e}"))?;
        let cleaned = decoded
            .replace("<|im_end|>", "")
            .replace("<|im_start|>", "")
            .replace("<|endoftext|>", "");
        Ok(cleaned.trim().to_string())
    }
}

pub struct AiManager {
    models_dir: PathBuf,
    loaded_model: Mutex<Option<LoadedCandleModel>>,
    downloading_id: Mutex<Option<String>>,
    cancel_download: Arc<AtomicBool>,
    manual_unload: AtomicBool,
}

impl AiManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            loaded_model: Mutex::new(None),
            downloading_id: Mutex::new(None),
            cancel_download: Arc::new(AtomicBool::new(false)),
            manual_unload: AtomicBool::new(false),
        }
    }

    pub fn model_dir(&self, model_id: &str) -> PathBuf {
        self.models_dir.join(model_id)
    }

    pub fn ensure_loaded_if_available(&self) {
        if self.manual_unload.load(Ordering::Relaxed) {
            return;
        }
        let Ok(mut guard) = self.loaded_model.lock() else {
            return;
        };
        if guard.is_some() {
            return;
        }
        let Some(spec) = MODEL_CATALOG.first() else {
            return;
        };
        let dir = self.model_dir(spec.id);
        let gguf_path = dir.join("model.gguf");
        let tok_path = dir.join("tokenizer.json");
        if gguf_path.exists() && tok_path.exists() {
            match LoadedCandleModel::load_from_dir(spec, &dir) {
                Ok(loaded) => {
                    *guard = Some(loaded);
                }
                Err(err) => {
                    eprintln!("Luair Candle auto-load error: {err}");
                }
            }
        }
    }

    pub fn status(&self) -> AiEngineStatus {
        let loaded_id = self
            .loaded_model
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|m| m.model_id.clone()));
        let loaded_name = self
            .loaded_model
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|m| m.model_name.clone()));
        let downloading_id = self
            .downloading_id
            .lock()
            .ok()
            .and_then(|g| g.clone());

        let models: Vec<AiModelInfo> = MODEL_CATALOG
            .iter()
            .map(|spec| {
                let dir = self.model_dir(spec.id);
                let gguf_path = dir.join("model.gguf");
                let tok_path = dir.join("tokenizer.json");
                let downloaded = gguf_path.exists() && tok_path.exists();
                let local_size_bytes = fs::metadata(&gguf_path).map(|m| m.len()).unwrap_or(0)
                    + fs::metadata(&tok_path).map(|m| m.len()).unwrap_or(0);
                AiModelInfo {
                    id: spec.id.to_string(),
                    name: spec.name.to_string(),
                    architecture: spec.arch.as_str().to_string(),
                    quantization: spec.quantization.to_string(),
                    approx_size_mb: spec.approx_size_mb,
                    description: spec.description.to_string(),
                    downloaded,
                    loaded: loaded_id.as_deref() == Some(spec.id),
                    local_size_bytes,
                }
            })
            .collect();

        let primary_downloaded = models.first().is_some_and(|m| m.downloaded);
        let active_backend = match loaded_name {
            Some(name) => format!("{name} (Local CPU · Active)"),
            None if primary_downloaded => {
                "Luair Candle (Downloaded · Click Load or Chat)".to_string()
            }
            None => "Offline Intent Engine (Download Luair Candle for full AI chat)".to_string(),
        };

        AiEngineStatus {
            models,
            loaded_model_id: loaded_id,
            downloading_model_id: downloading_id,
            active_backend,
        }
    }

    pub fn try_generate(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        user_prompt: &str,
        max_tokens: usize,
        temperature: f64,
    ) -> Option<(String, String)> {
        self.ensure_loaded_if_available();
        let mut guard = self.loaded_model.lock().ok()?;
        let model = guard.as_mut()?;
        let model_label = model.model_name.clone();
        match model.generate(system_prompt, history, user_prompt, max_tokens, temperature) {
            Ok(text) if !text.is_empty() => Some((text, model_label)),
            Ok(_) => None,
            Err(err) => {
                eprintln!("Luair Candle generation error: {err}");
                None
            }
        }
    }
}

fn find_model_spec(model_id: &str) -> Result<&'static ModelSpec, String> {
    MODEL_CATALOG
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model id '{model_id}'"))
}

struct DownloadSlotGuard(Arc<AiManager>);

impl Drop for DownloadSlotGuard {
    fn drop(&mut self) {
        if let Ok(mut dl) = self.0.downloading_id.lock() {
            *dl = None;
        }
    }
}

fn emit_progress(app: Option<&AppHandle>, progress: AiDownloadProgress) {
    if let Some(handle) = app {
        let _ = handle.emit("luair://ai-download-progress", progress);
    }
}

const MAX_DOWNLOAD_ATTEMPTS: usize = 8;

pub fn parse_content_range(header: &str) -> Option<(u64, Option<u64>)> {
    let trimmed = header.trim();
    if trimmed.len() < 6 || !trimmed[..6].eq_ignore_ascii_case("bytes ") {
        return None;
    }
    let rest = trimmed[6..].trim();
    let (range_part, total_part) = rest.split_once('/')?;
    let (start_str, end_str) = range_part.split_once('-')?;
    let start = start_str.trim().parse::<u64>().ok()?;
    let end = end_str.trim().parse::<u64>().ok()?;
    if end < start {
        return None;
    }
    let total = match total_part.trim() {
        "*" => None,
        s => Some(s.parse::<u64>().ok()?),
    };
    Some((start, total))
}

fn sleep_with_cancel(cancel_flag: &AtomicBool, total: Duration) -> bool {
    let step = Duration::from_millis(250);
    let mut elapsed = Duration::ZERO;
    while elapsed < total {
        if cancel_flag.load(Ordering::Relaxed) {
            return false;
        }
        let chunk = (total - elapsed).min(step);
        std::thread::sleep(chunk);
        elapsed += chunk;
    }
    !cancel_flag.load(Ordering::Relaxed)
}

pub fn download_file_to_path(
    app: Option<&AppHandle>,
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    model_id: &str,
    stage: &str,
    stage_label: &str,
    allow_range: bool,
    cancel_flag: &AtomicBool,
) -> Result<u64, String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed creating model folder: {e}"))?;
    }
    let part_path = dest.with_extension("part");

    emit_progress(
        app,
        AiDownloadProgress {
            model_id: model_id.to_string(),
            stage: stage.to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            percent: 0.0,
            message: stage_label.to_string(),
        },
    );

    let mut last_err = String::new();
    for attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = fs::remove_file(&part_path);
            return Err("Download canceled by user.".to_string());
        }

        if !allow_range && part_path.exists() {
            let _ = fs::remove_file(&part_path);
        }

        let existing_len = if allow_range {
            fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let call_res = if existing_len > 0 {
            agent
                .get(url)
                .header("Range", &format!("bytes={existing_len}-"))
                .call()
        } else {
            agent.get(url).call()
        };

        let mut resp = match call_res {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(416)) if allow_range && existing_len > 0 => {
                let _ = fs::remove_file(&part_path);
                last_err = format!("Range mismatch for {stage}; restarting from byte 0.");
                continue;
            }
            Err(e) => {
                last_err = format!("Network request failed ({stage}): {e}");
                if attempt < MAX_DOWNLOAD_ATTEMPTS {
                    emit_progress(
                        app,
                        AiDownloadProgress {
                            model_id: model_id.to_string(),
                            stage: stage.to_string(),
                            downloaded_bytes: existing_len,
                            total_bytes: None,
                            percent: 0.0,
                            message: "Reconnecting to resume download...".to_string(),
                        },
                    );
                    let wait = Duration::from_secs((attempt as u64).min(5));
                    if !sleep_with_cancel(cancel_flag, wait) {
                        let _ = fs::remove_file(&part_path);
                        return Err("Download canceled by user.".to_string());
                    }
                    continue;
                }
                if !allow_range {
                    let _ = fs::remove_file(&part_path);
                }
                return Err(last_err);
            }
        };

        let status = resp.status().as_u16();
        let body_len = resp.body().content_length();
        let content_range = resp
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_range);

        let resuming = allow_range
            && existing_len > 0
            && status == 206
            && content_range.is_some_and(|(start, _)| start == existing_len);

        let (mut out, mut downloaded_bytes, total_bytes) = if resuming {
            let total = content_range
                .and_then(|(_, tot)| tot)
                .or_else(|| body_len.map(|rem| existing_len + rem));
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part_path)
                .map_err(|e| format!("Failed opening temp file for resume: {e}"))?;
            (file, existing_len, total)
        } else {
            let file =
                File::create(&part_path).map_err(|e| format!("Failed creating temp file: {e}"))?;
            (file, 0u64, body_len)
        };

        let mut reader = resp.body_mut().as_reader();
        let mut buf = [0u8; 64 * 1024];
        let mut last_emit = Instant::now() - Duration::from_millis(250);
        let mut stream_err: Option<String> = None;

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = out.flush();
                drop(out);
                let _ = fs::remove_file(&part_path);
                return Err("Download canceled by user.".to_string());
            }

            let n = match reader.read(&mut buf) {
                Ok(count) => count,
                Err(e) => {
                    stream_err = Some(format!("Download interrupted ({stage}): {e}"));
                    break;
                }
            };
            if n == 0 {
                break;
            }
            if let Err(e) = out.write_all(&buf[..n]) {
                drop(out);
                let _ = fs::remove_file(&part_path);
                return Err(format!("Failed writing {stage} file: {e}"));
            }
            downloaded_bytes += n as u64;

            let now = Instant::now();
            if now.duration_since(last_emit) >= Duration::from_millis(120) {
                last_emit = now;
                let percent = total_bytes
                    .filter(|&t| t > 0)
                    .map(|t| ((downloaded_bytes as f64 / t as f64) * 100.0).min(99.0) as f32)
                    .unwrap_or(5.0);
                emit_progress(
                    app,
                    AiDownloadProgress {
                        model_id: model_id.to_string(),
                        stage: stage.to_string(),
                        downloaded_bytes,
                        total_bytes,
                        percent,
                        message: stage_label.to_string(),
                    },
                );
            }
        }

        if let Err(e) = out.flush() {
            drop(out);
            if !allow_range {
                let _ = fs::remove_file(&part_path);
            }
            return Err(format!("Failed flushing {stage} file: {e}"));
        }
        drop(out);

        if let Some(err_msg) = stream_err {
            last_err = err_msg;
            if !allow_range {
                let _ = fs::remove_file(&part_path);
            }
            if attempt < MAX_DOWNLOAD_ATTEMPTS {
                emit_progress(
                    app,
                    AiDownloadProgress {
                        model_id: model_id.to_string(),
                        stage: stage.to_string(),
                        downloaded_bytes,
                        total_bytes,
                        percent: total_bytes
                            .filter(|&t| t > 0)
                            .map(|t| ((downloaded_bytes as f64 / t as f64) * 100.0).min(99.0) as f32)
                            .unwrap_or(0.0),
                        message: "Reconnecting to resume download...".to_string(),
                    },
                );
                let wait = Duration::from_secs((attempt as u64).min(5));
                if !sleep_with_cancel(cancel_flag, wait) {
                    let _ = fs::remove_file(&part_path);
                    return Err("Download canceled by user.".to_string());
                }
                continue;
            }
            return Err(last_err);
        }

        if downloaded_bytes == 0 {
            let _ = fs::remove_file(&part_path);
            last_err = format!("Downloaded empty {stage} file.");
            if attempt < MAX_DOWNLOAD_ATTEMPTS {
                let wait = Duration::from_secs((attempt as u64).min(5));
                if !sleep_with_cancel(cancel_flag, wait) {
                    let _ = fs::remove_file(&part_path);
                    return Err("Download canceled by user.".to_string());
                }
                continue;
            }
            return Err(last_err);
        }

        if let Some(expected) = total_bytes.filter(|&t| t > 0) {
            if downloaded_bytes < expected {
                last_err = format!(
                    "Incomplete {stage} download ({downloaded_bytes} of {expected} bytes)."
                );
                if attempt < MAX_DOWNLOAD_ATTEMPTS {
                    emit_progress(
                        app,
                        AiDownloadProgress {
                            model_id: model_id.to_string(),
                            stage: stage.to_string(),
                            downloaded_bytes,
                            total_bytes,
                            percent: ((downloaded_bytes as f64 / expected as f64) * 100.0)
                                .min(99.0) as f32,
                            message: "Reconnecting to resume download...".to_string(),
                        },
                    );
                    let wait = Duration::from_secs((attempt as u64).min(5));
                    if !sleep_with_cancel(cancel_flag, wait) {
                        let _ = fs::remove_file(&part_path);
                        return Err("Download canceled by user.".to_string());
                    }
                    continue;
                }
                let _ = fs::remove_file(&part_path);
                return Err(last_err);
            }
            if downloaded_bytes > expected {
                let _ = fs::remove_file(&part_path);
                return Err(format!(
                    "Corrupt {stage} download ({downloaded_bytes} bytes exceeded expected {expected})."
                ));
            }
        }

        if dest.exists() {
            let _ = fs::remove_file(dest);
        }
        fs::rename(&part_path, dest).map_err(|e| {
            let _ = fs::remove_file(&part_path);
            format!("Failed finalizing {stage} file: {e}")
        })?;

        emit_progress(
            app,
            AiDownloadProgress {
                model_id: model_id.to_string(),
                stage: stage.to_string(),
                downloaded_bytes,
                total_bytes: total_bytes.or(Some(downloaded_bytes)),
                percent: 100.0,
                message: stage_label.to_string(),
            },
        );

        return Ok(downloaded_bytes);
    }

    Err(last_err)
}

pub fn extract_json_object(raw: &str) -> Option<serde_json::Value> {
    let start = raw.find('{')?;
    let bytes = raw.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;

    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let slice = &raw[start..=i];
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(slice) {
                        return Some(val);
                    }
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_hex_color(lower: &str) -> Option<String> {
    let bytes = lower.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'#' && i + 7 <= bytes.len() {
            let candidate = &lower[i..i + 7];
            if candidate[1..].bytes().all(|c| c.is_ascii_hexdigit()) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn strip_outline_color_phrases(lower: &str) -> String {
    let mut cleaned = lower.to_string();
    for phrase in [
        "black outline",
        "black border",
        "white outline",
        "white border",
        "dark outline",
        "dark border",
        "thin border",
        "thin outline",
    ] {
        cleaned = cleaned.replace(phrase, " ");
    }
    cleaned
}

fn detect_named_color(lower: &str) -> Option<(&'static str, &'static str)> {
    let color_text = strip_outline_color_phrases(lower);
    if color_text.contains("lime") {
        Some(("#84cc16", "Lime Green"))
    } else if color_text.contains("emerald") || color_text.contains("matrix") {
        Some(("#10b981", "Emerald"))
    } else if color_text.contains("green") {
        Some(("#22c55e", "Green"))
    } else if color_text.contains("turquoise") || color_text.contains("teal") {
        Some(("#14b8a6", "Teal"))
    } else if color_text.contains("cyan") || color_text.contains("aqua") || color_text.contains("ice") {
        Some(("#22d3ee", "Cyan"))
    } else if color_text.contains("sky") {
        Some(("#38bdf8", "Sky Blue"))
    } else if color_text.contains("indigo") {
        Some(("#818cf8", "Indigo"))
    } else if color_text.contains("blue") || color_text.contains("cobalt") || color_text.contains("ocean") {
        Some(("#38bdf8", "Blue"))
    } else if color_text.contains("purple") || color_text.contains("violet") {
        Some(("#a855f7", "Purple"))
    } else if color_text.contains("magenta") || color_text.contains("pink") {
        Some(("#ec4899", "Pink"))
    } else if color_text.contains("crimson") || color_text.contains("red") {
        Some(("#f43f5e", "Crimson"))
    } else if color_text.contains("gold") || color_text.contains("yellow") || color_text.contains("amber") {
        Some(("#f59e0b", "Gold"))
    } else if color_text.contains("orange") {
        Some(("#fb923c", "Orange"))
    } else if color_text.contains("white") {
        Some(("#ffffff", "White"))
    } else {
        None
    }
}

fn detect_requested_hotkey(prompt: &str) -> Option<String> {
    let lower = prompt.to_ascii_lowercase();
    for token in [
        "mouse4",
        "mouse5",
        "mousemiddle",
        "mouseright",
        "mouseleft",
        "xbutton1",
        "xbutton2",
    ] {
        if lower.contains(token) {
            let normalized = match token {
                "xbutton1" => "Mouse4",
                "xbutton2" => "Mouse5",
                "mouse4" => "Mouse4",
                "mouse5" => "Mouse5",
                "mousemiddle" => "MouseMiddle",
                "mouseright" => "MouseRight",
                "mouseleft" => "MouseLeft",
                _ => token,
            };
            return Some(normalized.to_string());
        }
    }

    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    for (idx, &w) in words.iter().enumerate() {
        if matches!(w, "key" | "hotkey" | "bind" | "bound" | "button") {
            if let Some(&next) = words.get(idx + 1) {
                if matches!(next, "to" | "on" | "as" | "is") {
                    if let Some(&target) = words.get(idx + 2) {
                        if hotkeys::parse_hotkey(target).is_some() {
                            return Some(target.to_ascii_uppercase());
                        }
                    }
                } else if hotkeys::parse_hotkey(next).is_some() {
                    return Some(next.to_ascii_uppercase());
                }
            }
        }
        if matches!(w, "slot" | "preset") {
            continue;
        }
        if w == "on" && idx > 0 && !matches!(words[idx - 1], "preset" | "profile" | "slot") {
            if let Some(&next) = words.get(idx + 1) {
                if next.len() == 1
                    && next.chars().all(|c| c.is_ascii_digit())
                    && hotkeys::parse_hotkey(next).is_some()
                {
                    return Some(next.to_ascii_uppercase());
                }
            }
        }
    }
    None
}

fn apply_json_to_crosshair(cfg: &mut CrosshairConfig, val: &serde_json::Value) {
    let Some(obj) = val.as_object() else { return };
    if let Some(s) = obj.get("color").and_then(|v| v.as_str()) {
        if s.starts_with('#') && s.len() >= 4 {
            cfg.color = s.to_string();
        }
    }
    if let Some(b) = obj.get("outline").and_then(|v| v.as_bool()) {
        cfg.outline = b;
    }
    if let Some(s) = obj.get("outlineColor").and_then(|v| v.as_str()) {
        if s.starts_with('#') && s.len() >= 4 {
            cfg.outline_color = s.to_string();
        }
    }
    if let Some(n) = obj.get("outlineThickness").and_then(|v| v.as_u64()) {
        cfg.outline_thickness = (n as u32).clamp(0, 8);
    }
    if let Some(n) = obj.get("length").and_then(|v| v.as_u64()) {
        cfg.length = (n as u32).clamp(0, 60);
    }
    if let Some(n) = obj.get("thickness").and_then(|v| v.as_u64()) {
        cfg.thickness = (n as u32).clamp(1, 24);
    }
    if let Some(n) = obj.get("gap").and_then(|v| v.as_u64()) {
        cfg.gap = (n as u32).clamp(0, 60);
    }
    if let Some(n) = obj.get("rotation").and_then(|v| v.as_u64()) {
        cfg.rotation = (n as u32).clamp(0, 360);
    }
    if let Some(b) = obj.get("dot").and_then(|v| v.as_bool()) {
        cfg.dot = b;
    }
    if let Some(n) = obj.get("dotSize").and_then(|v| v.as_u64()) {
        cfg.dot_size = (n as u32).clamp(1, 20);
    }
    if let Some(s) = obj.get("dotShape").and_then(|v| v.as_str()) {
        if s == "circle" || s == "square" {
            cfg.dot_shape = s.to_string();
        }
    }
    if let Some(b) = obj.get("tShape").and_then(|v| v.as_bool()) {
        cfg.t_shape = b;
    }
    if let Some(b) = obj.get("hideBars").and_then(|v| v.as_bool()) {
        cfg.hide_bars = b;
    }
    if let Some(b) = obj.get("hasCircle").and_then(|v| v.as_bool()) {
        cfg.has_circle = b;
    }
    if let Some(n) = obj.get("circleRadius").and_then(|v| v.as_u64()) {
        cfg.circle_radius = (n as u32).clamp(1, 200);
    }
    if let Some(f) = obj.get("opacity").and_then(|v| v.as_f64()) {
        cfg.opacity = (f as f32).clamp(0.1, 1.0);
    }
    if let Some(n) = obj.get("blur").and_then(|v| v.as_u64()) {
        cfg.blur = (n as u32).clamp(0, 20);
    }
    if let Some(s) = obj.get("colorMode").and_then(|v| v.as_str()) {
        if matches!(s, "solid" | "gradient" | "rainbow") {
            cfg.color_mode = s.to_string();
            cfg.rainbow = s == "rainbow";
        }
    }
    if let Some(s) = obj.get("gradientColor").and_then(|v| v.as_str()) {
        if s.starts_with('#') && s.len() >= 4 {
            cfg.gradient_color = s.to_string();
        }
    }
    if let Some(b) = obj.get("dynamicSpread").and_then(|v| v.as_bool()) {
        cfg.dynamic_spread = b;
    }
    if let Some(b) = obj.get("recoilColorEnabled").and_then(|v| v.as_bool()) {
        cfg.recoil_color_enabled = b;
    }
    if let Some(s) = obj.get("recoilColor").and_then(|v| v.as_str()) {
        if s.starts_with('#') && s.len() >= 4 {
            cfg.recoil_color = s.to_string();
        }
    }
    if let Some(b) = obj.get("animate").and_then(|v| v.as_bool()) {
        cfg.animate = b;
    }
}

pub fn synthesize_crosshair_from_prompt(
    base: &CrosshairConfig,
    prompt: &str,
    llm_json: Option<&serde_json::Value>,
) -> (CrosshairConfig, Option<String>, String) {
    let mut cfg = base.clone();
    if let Some(val) = llm_json {
        apply_json_to_crosshair(&mut cfg, val);
    }

    let lower = prompt.to_ascii_lowercase();

    if let Some(hex) = extract_hex_color(&lower) {
        cfg.color = hex;
    } else if let Some((hex, _)) = detect_named_color(&lower) {
        cfg.color = hex.to_string();
    }

    if lower.contains("gradient") {
        cfg.color_mode = "gradient".to_string();
        cfg.rainbow = false;
        if lower.contains("pink") {
            cfg.gradient_color = "#f472b6".to_string();
        } else if lower.contains("purple") || lower.contains("violet") {
            cfg.gradient_color = "#a855f7".to_string();
        } else if lower.contains("orange") || lower.contains("gold") {
            cfg.gradient_color = "#fb923c".to_string();
        }
    } else if lower.contains("rainbow") || lower.contains("rgb") {
        cfg.color_mode = "rainbow".to_string();
        cfg.rainbow = true;
    } else if lower.contains("solid") {
        cfg.color_mode = "solid".to_string();
        cfg.rainbow = false;
    }

    let explicit_bars = lower.contains("t-shape")
        || lower.contains("t shape")
        || lower.contains("plus")
        || lower.contains("bars")
        || lower.contains("x-shape")
        || lower.contains("diagonal")
        || lower.contains("valorant")
        || lower.contains("rifle")
        || lower.contains("vandal")
        || lower.contains("phantom");

    let wants_dot_only = (lower.contains("dot")
        && (lower.contains("only")
            || lower.contains("cs2")
            || lower.contains("clean")
            || lower.contains("precision")
            || lower.contains("pistol")
            || lower.contains("sheriff")
            || lower.contains("deagle")
            || lower.contains("sniper")
            || lower.contains("awp")
            || !explicit_bars))
        || (lower.contains("sniper") && !lower.contains("cross"))
        || (lower.contains("awp") && !lower.contains("cross"));

    if wants_dot_only {
        cfg.dot = true;
        cfg.hide_bars = true;
        cfg.t_shape = false;
        cfg.dot_size = if lower.contains("tiny") || lower.contains("micro") || lower.contains("small") {
            3
        } else if lower.contains("large") || lower.contains("big") {
            7
        } else {
            4
        };
        cfg.dot_shape = if lower.contains("square") {
            "square".to_string()
        } else {
            "circle".to_string()
        };
    } else {
        if lower.contains("dot") {
            cfg.dot = true;
        }
        if lower.contains("no dot") || lower.contains("without dot") {
            cfg.dot = false;
        }
        if explicit_bars || lower.contains("crosshair") || lower.contains("cross") || lower.contains("reticle") {
            cfg.hide_bars = false;
        }
        if lower.contains("t-shape") || lower.contains("t shape") {
            cfg.t_shape = true;
            cfg.hide_bars = false;
        }
    }

    if lower.contains("valorant") {
        cfg.hide_bars = false;
        cfg.length = 6;
        cfg.thickness = 2;
        cfg.gap = 3;
        cfg.outline = true;
        cfg.outline_thickness = 1;
    } else if lower.contains("rifle") || lower.contains("vandal") || lower.contains("phantom") {
        cfg.hide_bars = false;
        cfg.length = 10;
        cfg.thickness = 2;
        cfg.gap = 4;
        cfg.outline = true;
    }

    if lower.contains("x-shape") || lower.contains("diagonal") {
        cfg.rotation = 45;
        cfg.hide_bars = false;
    }
    if lower.contains("square") {
        cfg.dot = true;
        cfg.dot_shape = "square".to_string();
    }

    if !wants_dot_only {
        if lower.contains("tiny") || lower.contains("small") || lower.contains("compact") {
            cfg.length = 6;
            cfg.thickness = 2;
        } else if lower.contains("large") || lower.contains("big") || lower.contains("thick") {
            cfg.length = 16;
            cfg.thickness = 3;
        }
    }

    if lower.contains("no gap") || lower.contains("zero gap") || lower.contains("tight gap") {
        cfg.gap = 1;
    } else if lower.contains("wide gap") {
        cfg.gap = 9;
    }

    if lower.contains("circle") || lower.contains("ring") || lower.contains("shotgun") {
        cfg.has_circle = true;
        cfg.dot = true;
        if lower.contains("shotgun") || lower.contains("large") || lower.contains("wide") {
            cfg.circle_radius = 28;
        }
    }
    if lower.contains("shotgun") && !lower.contains("bars") {
        cfg.hide_bars = true;
    }

    if lower.contains("thin border") || lower.contains("thin outline") {
        cfg.outline = true;
        cfg.outline_thickness = 1;
        cfg.outline_opacity = 0.85;
    } else if lower.contains("no border") || lower.contains("no outline") {
        cfg.outline = false;
    } else if lower.contains("thick border") || lower.contains("bold outline") {
        cfg.outline = true;
        cfg.outline_thickness = 2;
    }

    if lower.contains("glow") || lower.contains("blur") || lower.contains("soft") {
        cfg.blur = 2;
    }
    if lower.contains("dynamic") || lower.contains("spread") || lower.contains("smg") {
        cfg.dynamic_spread = true;
        cfg.spread_amount = 16;
    }
    if lower.contains("recoil") {
        cfg.recoil_color_enabled = true;
    }
    if lower.contains("spin") {
        cfg.animate = true;
    }

    let bound_hotkey = llm_json
        .and_then(|v| v.get("hotkey").and_then(|h| h.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && hotkeys::parse_hotkey(s).is_some())
        .or_else(|| detect_requested_hotkey(prompt));

    let style_label = if cfg.hide_bars {
        if cfg.has_circle {
            "circle + dot"
        } else {
            "precision dot"
        }
    } else if cfg.t_shape {
        "T-shape"
    } else {
        "crosshair"
    };

    let summary = match &bound_hotkey {
        Some(hk) => format!(
            "Generated {style_label} ({}) and bound it to [{hk}].",
            cfg.color
        ),
        None => format!("Generated {style_label} ({}).", cfg.color),
    };

    (cfg, bound_hotkey, summary)
}

fn apply_json_to_theme(theme: &mut ThemeConfig, val: &serde_json::Value) {
    let Some(obj) = val.as_object() else { return };
    if let Some(s) = obj.get("name").and_then(|v| v.as_str()) {
        if !s.trim().is_empty() {
            theme.name = s.trim().to_string();
        }
    }
    if let Some(s) = obj.get("surfaceMode").and_then(|v| v.as_str()) {
        if s == "liquid-glass" || s == "solid-minimal" {
            theme.surface_mode = s.to_string();
        }
    }
    for (key, target) in [
        ("background", &mut theme.background),
        ("surface", &mut theme.surface),
        ("text", &mut theme.text),
        ("mutedText", &mut theme.muted_text),
        ("accent", &mut theme.accent),
        ("highlight", &mut theme.highlight),
    ] {
        if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
            if s.starts_with('#') && s.len() >= 4 {
                *target = s.to_string();
            }
        }
    }
    if let Some(n) = obj.get("blurPx").and_then(|v| v.as_u64()) {
        theme.blur_px = (n as u32).clamp(0, 36);
    }
    if let Some(f) = obj.get("glassOpacity").and_then(|v| v.as_f64()) {
        theme.glass_opacity = (f as f32).clamp(0.35, 1.0);
    }
    if let Some(n) = obj.get("borderRefraction").and_then(|v| v.as_u64()) {
        theme.border_refraction = (n as u32).clamp(0, 100);
    }
    if let Some(b) = obj.get("dynamicLighting").and_then(|v| v.as_bool()) {
        theme.dynamic_lighting = b;
    }
    if let Some(n) = obj.get("cornerRadius").and_then(|v| v.as_u64()) {
        theme.corner_radius = (n as u32).clamp(0, 24);
    }
}

pub fn synthesize_theme_from_prompt(
    base: &ThemeConfig,
    prompt: &str,
    llm_json: Option<&serde_json::Value>,
) -> (ThemeConfig, String) {
    let mut theme = base.clone();
    theme.id = "ai-custom".to_string();
    theme.name = "AI Studio Theme".to_string();

    if let Some(val) = llm_json {
        apply_json_to_theme(&mut theme, val);
    }

    let lower = prompt.to_ascii_lowercase();

    if lower.contains("solid") || lower.contains("minimal") || lower.contains("matte") {
        theme.surface_mode = "solid-minimal".to_string();
        theme.blur_px = 0;
        theme.glass_opacity = 1.0;
        theme.dynamic_lighting = false;
    } else if lower.contains("liquid") || lower.contains("glass") || lower.contains("blur") {
        theme.surface_mode = "liquid-glass".to_string();
        theme.blur_px = if lower.contains("heavy") { 28 } else { 22 };
        theme.glass_opacity = 0.76;
        theme.border_refraction = 86;
        theme.dynamic_lighting = true;
    }

    if lower.contains("light") || lower.contains("frost") || lower.contains("arctic") || lower.contains("white") {
        theme.background = "#e6f2fc".to_string();
        theme.surface = "#f8fbff".to_string();
        theme.text = "#0b1d33".to_string();
        theme.muted_text = "#4a6482".to_string();
    } else if lower.contains("purple") || lower.contains("violet") {
        theme.background = "#0d061a".to_string();
        theme.surface = "#190c30".to_string();
        theme.text = "#f5efff".to_string();
        theme.muted_text = "#b39ddb".to_string();
        theme.accent = "#a855f7".to_string();
        theme.highlight = "#f472b6".to_string();
    } else if lower.contains("crimson") || lower.contains("red") {
        theme.background = "#130508".to_string();
        theme.surface = "#210a10".to_string();
        theme.text = "#fff1f2".to_string();
        theme.muted_text = "#fda4af".to_string();
        theme.accent = "#f43f5e".to_string();
        theme.highlight = "#fb923c".to_string();
    } else if lower.contains("gold") || lower.contains("solar") || lower.contains("amber") {
        theme.background = "#120d06".to_string();
        theme.surface = "#1f170b".to_string();
        theme.text = "#fef7e0".to_string();
        theme.muted_text = "#c9b07a".to_string();
        theme.accent = "#f59e0b".to_string();
        theme.highlight = "#fde047".to_string();
    } else if lower.contains("emerald") || lower.contains("green") || lower.contains("matrix") {
        theme.background = "#040b07".to_string();
        theme.surface = "#0a1710".to_string();
        theme.text = "#dcfce7".to_string();
        theme.muted_text = "#6ee7b7".to_string();
        theme.accent = "#10b981".to_string();
        theme.highlight = "#34d399".to_string();
    } else if lower.contains("cyberpunk") || lower.contains("neon") {
        theme.background = "#090412".to_string();
        theme.surface = "#160a26".to_string();
        theme.text = "#f0f9ff".to_string();
        theme.muted_text = "#b89fd6".to_string();
        theme.accent = "#00f0ff".to_string();
        theme.highlight = "#ff00aa".to_string();
    } else if let Some(hex) = extract_hex_color(&lower) {
        theme.accent = hex;
    }

    let mode_label = if theme.surface_mode == "liquid-glass" {
        "Liquid Glass"
    } else {
        "Solid Minimalist"
    };
    let summary = format!("Applied AI {mode_label} theme (accent {}).", theme.accent);
    (theme, summary)
}

fn local_help_reply(prompt: &str) -> String {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("obs") || lower.contains("discord") || lower.contains("cloak") || lower.contains("stream") {
        "Luair's OBS Shield uses Windows `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` (`0x11`) so the crosshair stays visible to you while hidden from OBS, Discord screenshares, and screenshots. Toggle it in the OBS Shield tab or system tray.".to_string()
    } else if lower.contains("hotkey") || lower.contains("bind") || lower.contains("mouse") || lower.contains("key") {
        "Luair uses a non-blocking pass-through input hook (`rdev`), so pressing `1`, `2`, `3`, or `Mouse4` switches crosshair slots without delaying game inputs. Click any slot's key button in Profiles to bind any key or mouse button, or set the Menu Toggle hotkey (default `F10`) in Settings.".to_string()
    } else if lower.contains("support") || lower.contains("email") || lower.contains("contact") || lower.contains("help") {
        format!(
            "Need help or found a bug? Reach out anytime at {DEFAULT_SUPPORT_EMAIL} or open an issue on GitHub (https://github.com/confusionszn/luair/issues)."
        )
    } else if lower.contains("vault") || lower.contains("account") || lower.contains("privacy") || lower.contains("offline") {
        "Everything in Luair runs 100% locally with zero API keys or cloud servers. Your local account is protected in embedded SQLite with Argon2id password hashing and AES-256-GCM encryption.".to_string()
    } else {
        "I'm ready locally! Ask me to create a new preset (e.g. \"Create a preset called Sniper with a crimson dot on key 5\"), put a crosshair on an existing preset (\"Put a cyan T-shape crosshair on preset 2\"), generate a multi-preset pack, design a theme, or toggle OBS Shield.".to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackPresetSpec {
    name: String,
    style_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PresetAction {
    SwitchTo {
        index_1based: Option<usize>,
        name_query: Option<String>,
        hotkey_query: Option<String>,
    },
    RenameSlot {
        index_1based: Option<usize>,
        name_query: Option<String>,
        new_name: String,
    },
    DeleteSlot {
        index_1based: Option<usize>,
        name_query: Option<String>,
    },
    CreatePack {
        items: Vec<PackPresetSpec>,
        update_existing: bool,
    },
    CreateNew {
        name: Option<String>,
        hotkey: Option<String>,
        use_current_crosshair: bool,
    },
    UpdateSlotIndex {
        index_1based: usize,
        new_name: Option<String>,
        hotkey: Option<String>,
    },
    UpdateSlotByName {
        slot_id: String,
        new_name: Option<String>,
        hotkey: Option<String>,
    },
    DefaultTarget,
}

fn clean_extracted_name(raw: &str) -> Option<String> {
    let mut s = raw.trim().trim_matches(|c| matches!(c, '"' | '\'' | '`'));
    for prefix in ["a ", "an ", "the ", "my ", "new ", "preset ", "profile ", "slot "] {
        if s.len() > prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
            s = s[prefix.len()..].trim();
        }
    }
    let lower = s.to_ascii_lowercase();
    let stop_markers = [
        " with ",
        " using ",
        " having ",
        " on key",
        " on slot",
        " on preset",
        " on profile",
        " on 1",
        " on 2",
        " on 3",
        " on 4",
        " on 5",
        " on 6",
        " on 7",
        " on 8",
        " on 9",
        " on mouse",
        " bound ",
        " bind ",
        " hotkey ",
        " that ",
        " and ",
        ",",
        ".",
        "!",
        "?",
        "\n",
    ];
    let mut cutoff = s.len();
    for marker in stop_markers {
        if let Some(pos) = lower.find(marker) {
            if pos < cutoff {
                cutoff = pos;
            }
        }
    }
    let candidate = s[..cutoff]
        .trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '`' | ':' | '-'))
        .trim();
    if candidate.is_empty() || candidate.len() > 36 {
        return None;
    }
    let lower_cand = candidate.to_ascii_lowercase();
    if matches!(
        lower_cand.as_str(),
        "preset" | "profile" | "slot" | "crosshair" | "new preset" | "new profile" | "new slot"
    ) {
        return None;
    }
    Some(candidate.to_string())
}

pub fn extract_preset_name(prompt: &str) -> Option<String> {
    for quote in ['"', '\''] {
        if let Some(start) = prompt.find(quote) {
            let rest = &prompt[start + 1..];
            if let Some(end) = rest.find(quote) {
                let inner = rest[..end].trim();
                if !inner.is_empty() && inner.len() <= 36 {
                    return Some(inner.to_string());
                }
            }
        }
    }

    let lower = prompt.to_ascii_lowercase();
    for marker in [
        "called ",
        "named ",
        "titled ",
        "preset for ",
        "profile for ",
        "slot for ",
    ] {
        if let Some(pos) = lower.find(marker) {
            let after = &prompt[pos + marker.len()..];
            if let Some(cleaned) = clean_extracted_name(after) {
                return Some(cleaned);
            }
        }
    }

    if let Some(pos) = lower.find("as ") {
        let after = &prompt[pos + 3..];
        let trimmed_lower = after.trim().to_ascii_lowercase();
        if !trimmed_lower.starts_with("a new preset")
            && !trimmed_lower.starts_with("new preset")
            && !trimmed_lower.starts_with("preset")
        {
            if let Some(cleaned) = clean_extracted_name(after) {
                return Some(cleaned);
            }
        }
    }

    None
}

fn find_slot_index_in_prompt(prompt: &str) -> Option<usize> {
    let lower = prompt.to_ascii_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();

    for (idx, &tok) in tokens.iter().enumerate() {
        if matches!(tok, "preset" | "profile" | "slot") {
            if let Some(&next) = tokens.get(idx + 1) {
                let mut parsed = next.parse::<usize>().ok();
                if parsed.is_none() && next == "number" {
                    parsed = tokens.get(idx + 2).and_then(|t| t.parse::<usize>().ok());
                }
                if let Some(n) = parsed.filter(|&n| (1..=10).contains(&n)) {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn find_matching_profile_index(profiles: &[CrosshairProfile], prompt: &str) -> Option<usize> {
    let lower = prompt.to_ascii_lowercase();
    for (idx, profile) in profiles.iter().enumerate() {
        let full_name = profile.name.trim().to_ascii_lowercase();
        if !full_name.is_empty() && lower.contains(&full_name) {
            return Some(idx);
        }
        let words: Vec<&str> = full_name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| w.len() >= 4)
            .collect();
        for word in words {
            if matches!(
                word,
                "precision" | "dynamic" | "tactical" | "quarters" | "close" | "custom" | "preset"
            ) {
                continue;
            }
            if lower.contains(&format!("on {word}"))
                || lower.contains(&format!("on my {word}"))
                || lower.contains(&format!("on the {word}"))
                || lower.contains(&format!("for {word}"))
                || lower.contains(&format!("to {word}"))
                || lower.contains(&format!("{word} preset"))
                || lower.contains(&format!("{word} slot"))
            {
                return Some(idx);
            }
        }
    }
    None
}

fn build_pack_items(prompt: &str) -> Vec<PackPresetSpec> {
    let lower = prompt.to_ascii_lowercase();
    let base_color = detect_named_color(&lower).map(|(_, lbl)| lbl);

    let catalog: &[(&[&str], &str, &str)] = &[
        (
            &["rifle", "vandal", "phantom", "ak"],
            "Rifle",
            "cyan rifle crosshair with thin border",
        ),
        (
            &["pistol", "sheriff", "deagle", "sidearm"],
            "Pistol",
            "lime green precision pistol dot with thin border",
        ),
        (
            &["shotgun", "pump", "close quarters"],
            "Shotgun",
            "amber gold shotgun circle ring with center dot and dynamic spread",
        ),
        (
            &["sniper", "awp", "operator", "scout"],
            "Sniper",
            "crimson sniper precision dot with thin border",
        ),
        (
            &["smg", "submachine"],
            "SMG",
            "electric blue dynamic spread crosshair with dot",
        ),
        (
            &["valorant"],
            "Valorant",
            "cyan valorant tactical crosshair with thin border",
        ),
        (
            &["cs2", "counter"],
            "CS2",
            "lime green cs2 dot with thin border",
        ),
        (
            &["t-shape", "t shape", "tactical"],
            "Tactical T-Shape",
            "cyan gradient t-shape crosshair",
        ),
    ];

    let mut items = Vec::new();
    for (keywords, title, default_style) in catalog {
        if keywords.iter().any(|kw| lower.contains(kw)) {
            let style_prompt = match base_color {
                Some(color_name) => format!("{color_name} {default_style}"),
                None => default_style.to_string(),
            };
            items.push(PackPresetSpec {
                name: (*title).to_string(),
                style_prompt,
            });
        }
    }

    if items.is_empty() {
        let color_prefix = base_color.unwrap_or("Lime Green");
        items = vec![
            PackPresetSpec {
                name: "Rifle".to_string(),
                style_prompt: format!("{color_prefix} rifle crosshair with thin border"),
            },
            PackPresetSpec {
                name: "Precision Dot".to_string(),
                style_prompt: "Cyan precision dot with thin border".to_string(),
            },
            PackPresetSpec {
                name: "Shotgun Ring".to_string(),
                style_prompt: "Gold shotgun circle ring with dot and dynamic spread".to_string(),
            },
        ];
    }

    items
}

fn parse_preset_action(prompt: &str, profiles: &[CrosshairProfile]) -> PresetAction {
    let lower = prompt.to_ascii_lowercase();
    let idx_1based = find_slot_index_in_prompt(prompt);
    let matched_profile_idx = find_matching_profile_index(profiles, prompt);

    if lower.contains("rename ") && (lower.contains("preset") || lower.contains("profile") || lower.contains("slot") || matched_profile_idx.is_some()) {
        if let Some(to_pos) = lower.rfind(" to ") {
            let after_to = prompt[to_pos + 4..]
                .trim()
                .trim_matches(|c| matches!(c, '"' | '\'' | '.' | '!' | '?'))
                .trim();
            if !after_to.is_empty() && after_to.len() <= 36 {
                return PresetAction::RenameSlot {
                    index_1based: idx_1based,
                    name_query: matched_profile_idx.map(|i| profiles[i].id.clone()),
                    new_name: after_to.to_string(),
                };
            }
        }
    }

    if (lower.starts_with("delete ")
        || lower.starts_with("remove ")
        || lower.contains("delete preset")
        || lower.contains("remove preset")
        || lower.contains("delete slot")
        || lower.contains("remove slot"))
        && (idx_1based.is_some() || matched_profile_idx.is_some())
    {
        return PresetAction::DeleteSlot {
            index_1based: idx_1based,
            name_query: matched_profile_idx.map(|i| profiles[i].id.clone()),
        };
    }

    let has_style_words = lower.contains("with ")
        || lower.contains("dot")
        || lower.contains("crosshair")
        || lower.contains("reticle")
        || lower.contains("t-shape")
        || lower.contains("circle")
        || lower.contains("make ")
        || lower.contains("create ")
        || lower.contains("put ");

    let is_switch_intent = !has_style_words
        && (lower.starts_with("switch ")
            || lower.starts_with("load ")
            || lower.starts_with("equip ")
            || lower.starts_with("select ")
            || lower.contains("switch to preset")
            || lower.contains("switch to slot")
            || lower.contains("load preset")
            || lower.contains("equip preset"));

    if is_switch_intent && (idx_1based.is_some() || matched_profile_idx.is_some()) {
        return PresetAction::SwitchTo {
            index_1based: idx_1based,
            name_query: matched_profile_idx.map(|i| profiles[i].id.clone()),
            hotkey_query: detect_requested_hotkey(prompt),
        };
    }

    let mentions_plural_presets = lower.contains("presets")
        || lower.contains("profiles")
        || lower.contains("slots");

    if mentions_plural_presets
        && (lower.contains("make")
            || lower.contains("create")
            || lower.contains("add")
            || lower.contains("build")
            || lower.contains("generate")
            || lower.contains("put")
            || lower.contains("populate")
            || lower.contains("fill")
            || lower.contains("give"))
    {
        let update_existing = (lower.contains("all my presets")
            || lower.contains("my presets")
            || lower.contains("existing presets")
            || lower.contains("all presets"))
            && !lower.contains("new ");
        return PresetAction::CreatePack {
            items: build_pack_items(prompt),
            update_existing,
        };
    }

    if lower.contains("save")
        && (lower.contains("current") || lower.contains("this"))
        && (lower.contains("preset") || lower.contains("profile") || lower.contains("slot"))
    {
        return PresetAction::CreateNew {
            name: extract_preset_name(prompt),
            hotkey: detect_requested_hotkey(prompt),
            use_current_crosshair: true,
        };
    }

    if let Some(n) = idx_1based {
        return PresetAction::UpdateSlotIndex {
            index_1based: n,
            new_name: extract_preset_name(prompt),
            hotkey: detect_requested_hotkey(prompt),
        };
    }

    if !lower.contains("new preset") && !lower.contains("new profile") && !lower.contains("new slot") {
        if let Some(idx) = matched_profile_idx {
            let targets_existing = lower.contains(&format!("on {}", profiles[idx].name.to_ascii_lowercase()))
                || lower.contains("on ")
                || lower.contains("update ")
                || lower.contains("for ");
            if targets_existing {
                return PresetAction::UpdateSlotByName {
                    slot_id: profiles[idx].id.clone(),
                    new_name: extract_preset_name(prompt),
                    hotkey: detect_requested_hotkey(prompt),
                };
            }
        }
    }

    let wants_new_preset = lower.contains("new preset")
        || lower.contains("new profile")
        || lower.contains("new slot")
        || ((lower.contains("make")
            || lower.contains("create")
            || lower.contains("add")
            || lower.contains("build")
            || lower.contains("generate")
            || lower.contains("save"))
            && (lower.contains("preset") || lower.contains("profile") || lower.contains("slot")));

    if wants_new_preset {
        return PresetAction::CreateNew {
            name: extract_preset_name(prompt),
            hotkey: detect_requested_hotkey(prompt),
            use_current_crosshair: false,
        };
    }

    PresetAction::DefaultTarget
}

fn smart_preset_name(prompt: &str, cfg: &CrosshairConfig, explicit: Option<String>) -> String {
    if let Some(name) = explicit {
        return name;
    }
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("sniper") || lower.contains("awp") {
        return "Sniper".to_string();
    }
    if lower.contains("shotgun") {
        return "Shotgun".to_string();
    }
    if lower.contains("valorant") {
        return "Valorant".to_string();
    }
    if lower.contains("pistol") || lower.contains("sheriff") {
        return "Pistol".to_string();
    }
    let color_label = detect_named_color(&lower).map(|(_, n)| n).unwrap_or("Custom");
    let shape_label = if cfg.hide_bars {
        if cfg.has_circle {
            "Circle"
        } else {
            "Dot"
        }
    } else if cfg.t_shape {
        "T-Shape"
    } else {
        "Crosshair"
    };
    format!("{color_label} {shape_label}")
}

fn next_available_digit_hotkey(profiles: &[CrosshairProfile]) -> Option<String> {
    for digit in 1..=9 {
        let key = digit.to_string();
        let taken = profiles
            .iter()
            .any(|p| p.hotkey.as_deref().map(|k| k.eq_ignore_ascii_case(&key)) == Some(true));
        if !taken {
            return Some(key);
        }
    }
    None
}

fn next_unique_ai_slot_id(profiles: &[CrosshairProfile]) -> String {
    let mut n = profiles.len() + 1;
    loop {
        let candidate = format!("ai-slot-{n}");
        if !profiles.iter().any(|p| p.id == candidate) {
            return candidate;
        }
        n += 1;
    }
}

pub fn apply_preset_or_crosshair_to_state(
    state: &mut LuairState,
    prompt: &str,
    llm_json: Option<&serde_json::Value>,
    target_slot_id: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let action = parse_preset_action(prompt, &state.profiles);

    match action {
        PresetAction::SwitchTo {
            index_1based,
            name_query,
            hotkey_query,
        } => {
            let found_idx = index_1based
                .and_then(|n| n.checked_sub(1))
                .filter(|&i| i < state.profiles.len())
                .or_else(|| {
                    name_query.and_then(|id| state.profiles.iter().position(|p| p.id == id))
                })
                .or_else(|| {
                    hotkey_query.and_then(|hk| {
                        state.profiles.iter().position(|p| {
                            p.hotkey.as_deref().is_some_and(|k| k.eq_ignore_ascii_case(&hk))
                        })
                    })
                });
            let Some(idx) = found_idx else {
                return Err("Could not find that preset slot.".to_string());
            };
            let chosen = state.profiles[idx].clone();
            state.crosshair = chosen.config.clone();
            state.active_profile_id = Some(chosen.id.clone());
            let hk_note = chosen
                .hotkey
                .as_deref()
                .map(|k| format!(" [{k}]"))
                .unwrap_or_default();
            Ok((
                format!("Switched to preset #{} \"{}\"{hk_note}.", idx + 1, chosen.name),
                chosen.hotkey,
            ))
        }

        PresetAction::RenameSlot {
            index_1based,
            name_query,
            new_name,
        } => {
            let found_idx = index_1based
                .and_then(|n| n.checked_sub(1))
                .filter(|&i| i < state.profiles.len())
                .or_else(|| {
                    name_query.and_then(|id| state.profiles.iter().position(|p| p.id == id))
                })
                .or_else(|| {
                    state.active_profile_id.as_deref().and_then(|id| {
                        state.profiles.iter().position(|p| p.id == id)
                    })
                });
            let Some(idx) = found_idx else {
                return Err("Could not find that preset slot to rename.".to_string());
            };
            let old_name = state.profiles[idx].name.clone();
            state.profiles[idx].name = new_name.clone();
            state.active_profile_id = Some(state.profiles[idx].id.clone());
            let hk = state.profiles[idx].hotkey.clone();
            Ok((
                format!("Renamed preset #{} \"{old_name}\" to \"{new_name}\".", idx + 1),
                hk,
            ))
        }

        PresetAction::DeleteSlot {
            index_1based,
            name_query,
        } => {
            if state.profiles.len() <= 1 {
                return Err("Cannot delete the last remaining preset slot.".to_string());
            }
            let found_idx = index_1based
                .and_then(|n| n.checked_sub(1))
                .filter(|&i| i < state.profiles.len())
                .or_else(|| {
                    name_query.and_then(|id| state.profiles.iter().position(|p| p.id == id))
                });
            let Some(idx) = found_idx else {
                return Err("Could not find that preset slot to delete.".to_string());
            };
            let removed = state.profiles.remove(idx);
            if state.active_profile_id.as_deref() == Some(removed.id.as_str()) {
                if let Some(first) = state.profiles.first() {
                    state.active_profile_id = Some(first.id.clone());
                    state.crosshair = first.config.clone();
                }
            }
            Ok((
                format!("Deleted preset #{} \"{}\".", idx + 1, removed.name),
                None,
            ))
        }

        PresetAction::CreatePack {
            items,
            update_existing,
        } => {
            let mut summary_parts = Vec::new();
            if update_existing && !state.profiles.is_empty() {
                for (idx, profile) in state.profiles.iter_mut().enumerate() {
                    let style = &items[idx % items.len()].style_prompt;
                    let (cfg, _, _) =
                        synthesize_crosshair_from_prompt(&CrosshairConfig::default(), style, None);
                    profile.config = cfg;
                    summary_parts.push(format!("#{} \"{}\"", idx + 1, profile.name));
                }
                if let Some(first) = state.profiles.first() {
                    state.crosshair = first.config.clone();
                    state.active_profile_id = Some(first.id.clone());
                }
                return Ok((
                    format!(
                        "Updated crosshairs on {} presets ({}).",
                        summary_parts.len(),
                        summary_parts.join(", ")
                    ),
                    None,
                ));
            }

            if state.profiles.len() >= 10 {
                return Err(
                    "All 10 preset slots are full — delete a slot or ask me to put a crosshair on preset 1..10."
                        .to_string(),
                );
            }
            let mut first_new: Option<(String, CrosshairConfig, Option<String>)> = None;
            for item in items {
                if state.profiles.len() >= 10 {
                    break;
                }
                let (cfg, _, _) = synthesize_crosshair_from_prompt(
                    &CrosshairConfig::default(),
                    &item.style_prompt,
                    llm_json,
                );
                let id = next_unique_ai_slot_id(&state.profiles);
                let hk = next_available_digit_hotkey(&state.profiles);
                let slot_num = state.profiles.len() + 1;
                state.profiles.push(CrosshairProfile {
                    id: id.clone(),
                    name: item.name.clone(),
                    hotkey: hk.clone(),
                    config: cfg.clone(),
                });
                let hk_text = hk.as_deref().map(|k| format!(" [{k}]")).unwrap_or_default();
                summary_parts.push(format!("#{slot_num} \"{}\"{hk_text}", item.name));
                if first_new.is_none() {
                    first_new = Some((id, cfg, hk));
                }
            }
            if let Some((id, cfg, hk)) = first_new {
                state.crosshair = cfg;
                state.active_profile_id = Some(id);
                return Ok((
                    format!(
                        "Created {} presets ({}).",
                        summary_parts.len(),
                        summary_parts.join(", ")
                    ),
                    hk,
                ));
            }
            Err("No preset slots were created.".to_string())
        }

        PresetAction::CreateNew {
            name,
            hotkey,
            use_current_crosshair,
        } => {
            if state.profiles.len() >= 10 {
                return Err(
                    "All 10 preset slots are full — delete a slot or ask me to put a crosshair on preset 1..10."
                        .to_string(),
                );
            }
            let (cfg, extracted_hk) = if use_current_crosshair {
                (state.crosshair.clone(), hotkey.or_else(|| detect_requested_hotkey(prompt)))
            } else {
                let (cfg, hk, _) =
                    synthesize_crosshair_from_prompt(&CrosshairConfig::default(), prompt, llm_json);
                (cfg, hotkey.or(hk))
            };
            let final_name = smart_preset_name(prompt, &cfg, name);
            let final_hotkey = extracted_hk.or_else(|| next_available_digit_hotkey(&state.profiles));
            let id = next_unique_ai_slot_id(&state.profiles);
            let slot_num = state.profiles.len() + 1;
            state.profiles.push(CrosshairProfile {
                id: id.clone(),
                name: final_name.clone(),
                hotkey: final_hotkey.clone(),
                config: cfg.clone(),
            });
            state.crosshair = cfg.clone();
            state.active_profile_id = Some(id);
            let hk_text = final_hotkey
                .as_deref()
                .map(|k| format!(" on [{k}]"))
                .unwrap_or_default();
            Ok((
                format!(
                    "Created preset #{slot_num} \"{final_name}\"{hk_text} ({}).",
                    cfg.color
                ),
                final_hotkey,
            ))
        }

        PresetAction::UpdateSlotIndex {
            index_1based,
            new_name,
            hotkey,
        } => {
            let idx = index_1based.saturating_sub(1);
            if idx >= state.profiles.len() {
                return Err(format!(
                    "Preset #{index_1based} does not exist yet (you have {} presets). Ask me to create a new preset instead!",
                    state.profiles.len()
                ));
            }
            let (cfg, extracted_hk, _) =
                synthesize_crosshair_from_prompt(&CrosshairConfig::default(), prompt, llm_json);
            let final_hk = hotkey.or(extracted_hk);
            let slot = &mut state.profiles[idx];
            slot.config = cfg.clone();
            if let Some(name) = new_name {
                slot.name = name;
            }
            if let Some(ref hk) = final_hk {
                slot.hotkey = Some(hk.clone());
            }
            let slot_name = slot.name.clone();
            let slot_hk = slot.hotkey.clone();
            let slot_id = slot.id.clone();
            state.crosshair = cfg.clone();
            state.active_profile_id = Some(slot_id);
            let hk_text = slot_hk
                .as_deref()
                .map(|k| format!(" [{k}]"))
                .unwrap_or_default();
            Ok((
                format!(
                    "Updated preset #{index_1based} \"{slot_name}\"{hk_text} with {} crosshair.",
                    cfg.color
                ),
                slot_hk,
            ))
        }

        PresetAction::UpdateSlotByName {
            slot_id,
            new_name,
            hotkey,
        } => {
            let Some(idx) = state.profiles.iter().position(|p| p.id == slot_id) else {
                return Err("Could not find that preset slot.".to_string());
            };
            let (cfg, extracted_hk, _) =
                synthesize_crosshair_from_prompt(&CrosshairConfig::default(), prompt, llm_json);
            let final_hk = hotkey.or(extracted_hk);
            let slot = &mut state.profiles[idx];
            slot.config = cfg.clone();
            if let Some(name) = new_name {
                slot.name = name;
            }
            if let Some(ref hk) = final_hk {
                slot.hotkey = Some(hk.clone());
            }
            let slot_name = slot.name.clone();
            let slot_hk = slot.hotkey.clone();
            state.crosshair = cfg.clone();
            state.active_profile_id = Some(slot.id.clone());
            Ok((
                format!(
                    "Updated preset #{} \"{slot_name}\" with {} crosshair.",
                    idx + 1,
                    cfg.color
                ),
                slot_hk,
            ))
        }

        PresetAction::DefaultTarget => {
            let (new_cfg, bound_hotkey, summary) =
                synthesize_crosshair_from_prompt(&state.crosshair, prompt, llm_json);
            state.crosshair = new_cfg.clone();

            if let Some(slot_id) = target_slot_id {
                if let Some(p) = state.profiles.iter_mut().find(|p| p.id == slot_id) {
                    p.config = new_cfg.clone();
                    if let Some(ref hk) = bound_hotkey {
                        p.hotkey = Some(hk.clone());
                    }
                    state.active_profile_id = Some(p.id.clone());
                }
            } else if let Some(ref hk) = bound_hotkey {
                let hk_upper = hk.to_ascii_uppercase();
                if let Some(existing) = state.profiles.iter_mut().find(|p| {
                    p.hotkey.as_deref().map(|k| k.to_ascii_uppercase()) == Some(hk_upper.clone())
                }) {
                    existing.config = new_cfg.clone();
                    state.active_profile_id = Some(existing.id.clone());
                } else if let Some(slot_idx) = hk
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n >= 1 && n <= state.profiles.len())
                {
                    let p = &mut state.profiles[slot_idx - 1];
                    p.config = new_cfg.clone();
                    p.hotkey = Some(hk.clone());
                    state.active_profile_id = Some(p.id.clone());
                } else if state.profiles.len() < 10 {
                    let id = next_unique_ai_slot_id(&state.profiles);
                    state.profiles.push(CrosshairProfile {
                        id: id.clone(),
                        name: smart_preset_name(prompt, &new_cfg, extract_preset_name(prompt)),
                        hotkey: Some(hk.clone()),
                        config: new_cfg,
                    });
                    state.active_profile_id = Some(id);
                } else if let Some(active_id) = state.active_profile_id.clone() {
                    if let Some(p) = state.profiles.iter_mut().find(|p| p.id == active_id) {
                        p.config = new_cfg;
                        p.hotkey = Some(hk.clone());
                    }
                }
            } else if let Some(active_id) = state.active_profile_id.clone() {
                if let Some(p) = state.profiles.iter_mut().find(|p| p.id == active_id) {
                    p.config = new_cfg;
                }
            }

            Ok((summary, bound_hotkey))
        }
    }
}

fn apply_generated_preset_or_crosshair(
    app: &AppHandle,
    app_state: &AppState,
    prompt: &str,
    llm_json: Option<&serde_json::Value>,
    target_slot_id: Option<&str>,
) -> Result<(String, Option<String>, LuairState), String> {
    let (next_state, summary, bound_hotkey) = {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        let (summary, bound_hotkey) =
            apply_preset_or_crosshair_to_state(&mut guard, prompt, llm_json, target_slot_id)?;
        (guard.clone(), summary, bound_hotkey)
    };

    app_state.sync_hotkeys(&next_state);
    storage::save_to_disk(app, &next_state)?;
    let _ = app.emit("luair://state-updated", &next_state);
    Ok((summary, bound_hotkey, next_state))
}

#[tauri::command]
pub fn get_ai_status(app_state: State<'_, AppState>) -> Result<AiEngineStatus, String> {
    Ok(app_state.ai.status())
}

#[tauri::command]
pub async fn download_ai_model(
    app: AppHandle,
    app_state: State<'_, AppState>,
    model_id: String,
) -> Result<AiEngineStatus, String> {
    let spec = find_model_spec(&model_id)?;
    let ai = app_state.ai.clone();

    {
        let mut dl = ai.downloading_id.lock().map_err(|e| e.to_string())?;
        if let Some(existing) = dl.as_ref() {
            return Err(format!("Already downloading '{existing}'."));
        }
        *dl = Some(model_id.clone());
    }
    ai.cancel_download.store(false, Ordering::Relaxed);
    ai.manual_unload.store(false, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        let _guard = DownloadSlotGuard(ai.clone());
        let dir = ai.model_dir(spec.id);
        let agent = build_http_agent();

        download_file_to_path(
            Some(&app),
            &agent,
            spec.tokenizer_url,
            &dir.join("tokenizer.json"),
            spec.id,
            "tokenizer",
            "Downloading Luair Candle tokenizer (1/2)...",
            false,
            &ai.cancel_download,
        )?;

        download_file_to_path(
            Some(&app),
            &agent,
            spec.gguf_url,
            &dir.join("model.gguf"),
            spec.id,
            "model",
            "Downloading Luair Candle weights (2/2)...",
            true,
            &ai.cancel_download,
        )?;

        emit_progress(
            Some(&app),
            AiDownloadProgress {
                model_id: spec.id.to_string(),
                stage: "loading".to_string(),
                downloaded_bytes: 0,
                total_bytes: None,
                percent: 99.0,
                message: "Loading Luair Candle...".to_string(),
            },
        );

        let loaded = LoadedCandleModel::load_from_dir(spec, &dir)?;
        {
            let mut guard = ai.loaded_model.lock().map_err(|e| e.to_string())?;
            *guard = Some(loaded);
        }

        emit_progress(
            Some(&app),
            AiDownloadProgress {
                model_id: spec.id.to_string(),
                stage: "ready".to_string(),
                downloaded_bytes: 0,
                total_bytes: None,
                percent: 100.0,
                message: "Luair Candle ready.".to_string(),
            },
        );

        Ok(ai.status())
    })
    .await
    .map_err(|e| format!("Download task error: {e}"))?
}

#[tauri::command]
pub fn cancel_ai_download(app_state: State<'_, AppState>) -> Result<(), String> {
    app_state.ai.cancel_download.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn load_ai_model(
    app_state: State<'_, AppState>,
    model_id: String,
) -> Result<AiEngineStatus, String> {
    let spec = find_model_spec(&model_id)?;
    let ai = app_state.ai.clone();
    ai.manual_unload.store(false, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        let dir = ai.model_dir(spec.id);
        let loaded = LoadedCandleModel::load_from_dir(spec, &dir)?;
        {
            let mut guard = ai.loaded_model.lock().map_err(|e| e.to_string())?;
            *guard = Some(loaded);
        }
        Ok(ai.status())
    })
    .await
    .map_err(|e| format!("Load task error: {e}"))?
}

#[tauri::command]
pub fn unload_ai_model(app_state: State<'_, AppState>) -> Result<AiEngineStatus, String> {
    app_state.ai.manual_unload.store(true, Ordering::Relaxed);
    {
        let mut guard = app_state.ai.loaded_model.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
    Ok(app_state.ai.status())
}

#[tauri::command]
pub fn delete_ai_model(
    app_state: State<'_, AppState>,
    model_id: String,
) -> Result<AiEngineStatus, String> {
    let _ = find_model_spec(&model_id)?;
    {
        let mut guard = app_state.ai.loaded_model.lock().map_err(|e| e.to_string())?;
        if guard.as_ref().map(|m| m.model_id.as_str()) == Some(model_id.as_str()) {
            *guard = None;
        }
    }
    let dir = app_state.ai.model_dir(&model_id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(app_state.ai.status())
}

const CROSSHAIR_JSON_SYSTEM_PROMPT: &str = "You convert user crosshair requests into ONE valid JSON object with any relevant keys from: color (hex), outline (bool), outlineThickness (0-8), outlineColor (hex), length (0-60), thickness (1-24), gap (0-60), dot (bool), dotSize (1-20), dotShape (circle|square), hideBars (bool), tShape (bool), hasCircle (bool), circleRadius (1-200), opacity (0.1-1.0), blur (0-20), colorMode (solid|gradient|rainbow), gradientColor (hex), dynamicSpread (bool), recoilColorEnabled (bool), recoilColor (hex), hotkey (string). Output JSON only.";
const THEME_JSON_SYSTEM_PROMPT: &str = "You convert user UI theme requests into ONE valid JSON object with keys: name, surfaceMode (liquid-glass|solid-minimal), background (hex), surface (hex), text (hex), mutedText (hex), accent (hex), highlight (hex), blurPx (0-36), glassOpacity (0.35-1.0), borderRefraction (0-100), dynamicLighting (bool), cornerRadius (0-24). Output JSON only.";

#[tauri::command]
pub async fn ai_make_crosshair(
    app: AppHandle,
    app_state: State<'_, AppState>,
    prompt: String,
    target_slot_id: Option<String>,
) -> Result<AiApplyResult, String> {
    let ai = app_state.ai.clone();
    let prompt_clone = prompt.clone();

    let (raw_llm, engine_used) = tauri::async_runtime::spawn_blocking(move || {
        match ai.try_generate(CROSSHAIR_JSON_SYSTEM_PROMPT, &[], &prompt_clone, 180, 0.2) {
            Some((text, engine)) => (Some(text), engine),
            None => (None, "Offline Intent Engine".to_string()),
        }
    })
    .await
    .map_err(|e| format!("AI generation task error: {e}"))?;

    let llm_json = raw_llm.as_deref().and_then(extract_json_object);
    let (summary, bound_hotkey, next_state) = apply_generated_preset_or_crosshair(
        &app,
        &app_state,
        &prompt,
        llm_json.as_ref(),
        target_slot_id.as_deref(),
    )?;

    Ok(AiApplyResult {
        summary,
        engine_used,
        bound_hotkey,
        state: next_state,
    })
}

#[tauri::command]
pub async fn ai_make_theme(
    app: AppHandle,
    app_state: State<'_, AppState>,
    prompt: String,
) -> Result<AiApplyResult, String> {
    let base_theme = app_state.snapshot().ui.theme;
    let ai = app_state.ai.clone();
    let prompt_clone = prompt.clone();

    let (raw_llm, engine_used) = tauri::async_runtime::spawn_blocking(move || {
        match ai.try_generate(THEME_JSON_SYSTEM_PROMPT, &[], &prompt_clone, 180, 0.25) {
            Some((text, engine)) => (Some(text), engine),
            None => (None, "Offline Intent Engine".to_string()),
        }
    })
    .await
    .map_err(|e| format!("AI theme task error: {e}"))?;

    let llm_json = raw_llm.as_deref().and_then(extract_json_object);
    let (new_theme, summary) =
        synthesize_theme_from_prompt(&base_theme, &prompt, llm_json.as_ref());

    let next_state = {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        guard.ui.ui_theme = new_theme.name.clone();
        guard.ui.ui_accent = new_theme.accent.clone();
        guard.ui.theme = new_theme;
        guard.clone()
    };

    storage::save_to_disk(&app, &next_state)?;
    let _ = app.emit("luair://state-updated", &next_state);

    Ok(AiApplyResult {
        summary,
        engine_used,
        bound_hotkey: None,
        state: next_state,
    })
}

#[tauri::command]
pub async fn ai_chat(
    app: AppHandle,
    app_state: State<'_, AppState>,
    messages: Vec<ChatMessage>,
    prompt: String,
) -> Result<AiChatResponse, String> {
    let lower = prompt.to_ascii_lowercase();
    let snapshot = app_state.snapshot();

    let wants_shield_on = (lower.contains("enable") || lower.contains("turn on"))
        && (lower.contains("shield") || lower.contains("cloak") || lower.contains("obs"));
    let wants_shield_off = (lower.contains("disable") || lower.contains("turn off"))
        && (lower.contains("shield") || lower.contains("cloak") || lower.contains("obs"));
    let wants_theme = (lower.contains("make")
        || lower.contains("create")
        || lower.contains("change")
        || lower.contains("set")
        || lower.contains("apply"))
        && (lower.contains("theme") || lower.contains("liquid glass") || lower.contains("glassmorphism"));
    let wants_preset_or_crosshair = (lower.contains("make")
        || lower.contains("create")
        || lower.contains("add")
        || lower.contains("save")
        || lower.contains("put")
        || lower.contains("build")
        || lower.contains("generate")
        || lower.contains("set")
        || lower.contains("apply")
        || lower.contains("change")
        || lower.contains("update")
        || lower.contains("give")
        || lower.contains("fill")
        || lower.contains("populate")
        || lower.contains("switch")
        || lower.contains("load")
        || lower.contains("equip")
        || lower.contains("select")
        || lower.contains("rename")
        || lower.contains("delete")
        || lower.contains("remove"))
        && (lower.contains("preset")
            || lower.contains("profile")
            || lower.contains("slot")
            || lower.contains("crosshair")
            || lower.contains("dot")
            || lower.contains("reticle")
            || lower.contains("t-shape")
            || lower.contains("circle")
            || lower.contains("ring"));

    let ai = app_state.ai.clone();
    let prompt_clone = prompt.clone();

    if wants_shield_on || wants_shield_off {
        let target = wants_shield_on;
        let next_state = {
            let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
            guard.ui.streamproof = target;
            guard.clone()
        };
        let _ = cloak::apply_cloak_to_app(&app, target, next_state.ui.cloak_menu);
        storage::save_to_disk(&app, &next_state)?;
        let _ = app.emit("luair://state-updated", &next_state);
        let action = if target {
            "Enabled OBS Shield (WDA_EXCLUDEFROMCAPTURE)".to_string()
        } else {
            "Disabled OBS Shield".to_string()
        };
        return Ok(AiChatResponse {
            reply: format!("Done — {action}."),
            engine_used: "Luair Candle Action Engine".to_string(),
            applied_action: Some(action),
            updated_state: Some(next_state),
        });
    }

    if wants_theme {
        let (raw_llm, engine_used) = tauri::async_runtime::spawn_blocking(move || {
            match ai.try_generate(THEME_JSON_SYSTEM_PROMPT, &[], &prompt_clone, 160, 0.25) {
                Some((text, engine)) => (Some(text), engine),
                None => (None, "Offline Intent Engine".to_string()),
            }
        })
        .await
        .map_err(|e| format!("AI theme task error: {e}"))?;

        let llm_json = raw_llm.as_deref().and_then(extract_json_object);
        let (new_theme, summary) =
            synthesize_theme_from_prompt(&snapshot.ui.theme, &prompt, llm_json.as_ref());
        let next_state = {
            let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
            guard.ui.ui_theme = new_theme.name.clone();
            guard.ui.ui_accent = new_theme.accent.clone();
            guard.ui.theme = new_theme;
            guard.clone()
        };
        storage::save_to_disk(&app, &next_state)?;
        let _ = app.emit("luair://state-updated", &next_state);
        return Ok(AiChatResponse {
            reply: format!("Done — {summary}"),
            engine_used,
            applied_action: Some(summary),
            updated_state: Some(next_state),
        });
    }

    if wants_preset_or_crosshair {
        let (raw_llm, engine_used) = tauri::async_runtime::spawn_blocking(move || {
            match ai.try_generate(CROSSHAIR_JSON_SYSTEM_PROMPT, &[], &prompt_clone, 160, 0.2) {
                Some((text, engine)) => (Some(text), engine),
                None => (None, "Offline Intent Engine".to_string()),
            }
        })
        .await
        .map_err(|e| format!("AI preset/crosshair task error: {e}"))?;

        let llm_json = raw_llm.as_deref().and_then(extract_json_object);
        let (summary, _bound_hotkey, next_state) = apply_generated_preset_or_crosshair(
            &app,
            &app_state,
            &prompt,
            llm_json.as_ref(),
            None,
        )?;
        return Ok(AiChatResponse {
            reply: format!("Done — {summary}"),
            engine_used,
            applied_action: Some(summary),
            updated_state: Some(next_state),
        });
    }

    let (llm_reply, engine_used) = tauri::async_runtime::spawn_blocking(move || {
        let sys = "You are Luair Candle, the built-in offline AI assistant inside Luair, a Rust + Tauri desktop crosshair overlay. Be concise (2-4 short sentences), accurate, and friendly. You can create/edit/switch/rename/delete crosshair presets, generate custom crosshairs and Liquid Glass/Solid themes, and toggle OBS Shield (WDA_EXCLUDEFROMCAPTURE). Support email: luairsupport@gmail.com.";
        match ai.try_generate(sys, &messages, &prompt_clone, 140, 0.45) {
            Some((text, engine)) => (Some(text), engine),
            None => (None, "Offline Intent Engine".to_string()),
        }
    })
    .await
    .map_err(|e| format!("AI chat task error: {e}"))?;

    Ok(AiChatResponse {
        reply: llm_reply.unwrap_or_else(|| local_help_reply(&prompt)),
        engine_used,
        applied_action: None,
        updated_state: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_single_luair_candle_entry() {
        assert_eq!(MODEL_CATALOG.len(), 1);
        let entry = &MODEL_CATALOG[0];
        assert_eq!(entry.id, "luair-candle");
        assert_eq!(entry.name, "Luair Candle");
        assert_eq!(entry.arch, ModelArch::Qwen2);
        assert_eq!(entry.quantization, "Q4_K_M");
        assert_eq!(entry.approx_size_mb, 475);
        assert!(!entry.name.to_ascii_lowercase().contains("qwen"));
        assert!(!entry.name.to_ascii_lowercase().contains("smollm"));
        assert!(!entry.description.to_ascii_lowercase().contains("qwen"));
        assert!(!entry.description.to_ascii_lowercase().contains("smollm"));
    }

    #[test]
    fn deduplicates_trailing_user_message_in_history() {
        let history = vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Earlier question".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Earlier answer".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "  Enable OBS Shield ".to_string(),
            },
        ];
        let prompt = build_chatml_prompt("System", &history, "Enable OBS Shield");
        assert_eq!(prompt.matches("Enable OBS Shield").count(), 1);
        assert!(prompt.contains("Earlier question"));
        assert!(prompt.contains("Earlier answer"));
    }

    #[test]
    #[ignore]
    fn live_download_load_and_chat_luair_candle() {
        let spec = &MODEL_CATALOG[0];
        let temp_dir = std::env::temp_dir().join("luair_candle_live_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        fs::create_dir_all(&temp_dir).expect("create live test temp dir");

        let agent = build_http_agent();
        let cancel = AtomicBool::new(false);

        let tok_path = temp_dir.join("tokenizer.json");
        let gguf_path = temp_dir.join("model.gguf");

        let tok_bytes = download_file_to_path(
            None,
            &agent,
            spec.tokenizer_url,
            &tok_path,
            spec.id,
            "tokenizer",
            "Downloading Luair Candle tokenizer (1/2)...",
            false,
            &cancel,
        )
        .expect("download tokenizer.json");
        assert_eq!(tok_bytes, 7_031_645);

        let gguf_bytes = download_file_to_path(
            None,
            &agent,
            spec.gguf_url,
            &gguf_path,
            spec.id,
            "model",
            "Downloading Luair Candle weights (2/2)...",
            true,
            &cancel,
        )
        .expect("download model.gguf");
        assert_eq!(gguf_bytes, 491_400_032);

        let combined_mib = ((tok_bytes + gguf_bytes) as f64 / (1024.0 * 1024.0)).round() as u32;
        assert_eq!(combined_mib, spec.approx_size_mb);

        let mut model = LoadedCandleModel::load_from_dir(spec, &temp_dir)
            .expect("load downloaded Luair Candle model");
        let reply = model
            .generate(
                "You are Luair Candle, an offline desktop crosshair assistant. Reply concisely.",
                &[],
                "Say hello as Luair Candle in one short sentence.",
                48,
                0.3,
            )
            .expect("generate reply from Luair Candle");

        println!("Luair Candle live reply: {reply}");
        assert!(!reply.is_empty());
        assert!(reply.chars().any(|c| c.is_ascii_alphabetic()));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn parses_http_content_range_headers() {
        assert_eq!(
            parse_content_range("bytes 100-491400031/491400032"),
            Some((100, Some(491_400_032)))
        );
        assert_eq!(parse_content_range("bytes 0-99/*"), Some((0, None)));
        assert_eq!(parse_content_range("bytes 20-10/100"), None);
        assert_eq!(parse_content_range("items 0-10/100"), None);
        assert_eq!(parse_content_range("invalid"), None);
    }

    #[test]
    fn extracts_json_from_fenced_and_prose_llm_output() {
        let raw = "Sure! Here is your crosshair:\n```json\n{\"color\": \"#84cc16\", \"dot\": true, \"gap\": 3}\n```\nHope you like it.";
        let parsed = extract_json_object(raw).expect("extract balanced json");
        assert_eq!(parsed["color"], "#84cc16");
        assert_eq!(parsed["dot"], true);
        assert_eq!(parsed["gap"], 3);
    }

    #[test]
    fn generates_lime_green_cs2_dot_with_thin_border_and_key_2_bind() {
        let base = CrosshairConfig::default();
        let (cfg, hotkey, summary) = synthesize_crosshair_from_prompt(
            &base,
            "Make a clean lime green CS2 dot with a thin border and bind it to key 2",
            None,
        );
        assert_eq!(cfg.color, "#84cc16");
        assert!(cfg.dot);
        assert!(cfg.hide_bars);
        assert!(cfg.outline);
        assert_eq!(cfg.outline_thickness, 1);
        assert_eq!(hotkey.as_deref(), Some("2"));
        assert!(summary.contains("[2]"));
    }

    #[test]
    fn generates_neon_purple_liquid_glass_theme() {
        let base = ThemeConfig::default();
        let (theme, summary) = synthesize_theme_from_prompt(
            &base,
            "Make a neon purple liquid glass theme",
            None,
        );
        assert_eq!(theme.surface_mode, "liquid-glass");
        assert_eq!(theme.accent, "#a855f7");
        assert!(theme.blur_px >= 20);
        assert!(theme.dynamic_lighting);
        assert!(summary.contains("Liquid Glass"));
    }

    #[test]
    fn merges_llm_json_fields_over_base_config() {
        let base = CrosshairConfig::default();
        let llm = serde_json::json!({
            "color": "#ff4488",
            "length": 14,
            "gap": 6,
            "tShape": true,
            "dynamicSpread": true,
            "hotkey": "Mouse5"
        });
        let (cfg, hotkey, _) = synthesize_crosshair_from_prompt(
            &base,
            "Custom tactical reticle",
            Some(&llm),
        );
        assert_eq!(cfg.color, "#ff4488");
        assert_eq!(cfg.length, 14);
        assert_eq!(cfg.gap, 6);
        assert!(cfg.t_shape);
        assert!(cfg.dynamic_spread);
        assert_eq!(hotkey.as_deref(), Some("Mouse5"));
    }

    #[test]
    fn creates_new_named_preset_even_when_target_slot_id_is_present() {
        let mut state = LuairState::default();
        assert_eq!(state.profiles.len(), 4);

        let (summary, hk) = apply_preset_or_crosshair_to_state(
            &mut state,
            "Make a new preset called Sniper with a crimson dot and thin border on key 5",
            None,
            Some("slot-1"),
        )
        .expect("create Sniper preset");

        assert_eq!(state.profiles.len(), 5);
        let created = &state.profiles[4];
        assert_eq!(created.name, "Sniper");
        assert_eq!(created.hotkey.as_deref(), Some("5"));
        assert_eq!(hk.as_deref(), Some("5"));
        assert_eq!(created.config.color, "#f43f5e");
        assert!(created.config.dot);
        assert!(created.config.hide_bars);
        assert!(created.config.outline);
        assert_eq!(state.active_profile_id.as_deref(), Some(created.id.as_str()));
        assert_eq!(state.profiles[0].name, "Primary (Rifle)");
        assert!(summary.contains("Sniper"));
    }

    #[test]
    fn puts_crosshair_onto_existing_preset_by_index_and_name() {
        let mut state = LuairState::default();

        let (summary1, _) = apply_preset_or_crosshair_to_state(
            &mut state,
            "Put a cyan T-shape crosshair on preset 2",
            None,
            None,
        )
        .expect("put crosshair on preset 2");
        assert_eq!(state.profiles[1].config.color, "#22d3ee");
        assert!(state.profiles[1].config.t_shape);
        assert!(!state.profiles[1].config.hide_bars);
        assert_eq!(state.active_profile_id.as_deref(), Some("slot-2"));
        assert!(summary1.contains("#2"));

        let (summary2, _) = apply_preset_or_crosshair_to_state(
            &mut state,
            "Put a gold circle crosshair on Primary",
            None,
            None,
        )
        .expect("put crosshair on Primary");
        assert_eq!(state.profiles[0].config.color, "#f59e0b");
        assert!(state.profiles[0].config.has_circle);
        assert_eq!(state.active_profile_id.as_deref(), Some("slot-1"));
        assert!(summary2.contains("Primary"));
    }

    #[test]
    fn saves_current_crosshair_and_supports_switch_rename_delete() {
        let mut state = LuairState::default();
        state.crosshair.color = "#ec4899".to_string();
        state.crosshair.length = 18;

        apply_preset_or_crosshair_to_state(
            &mut state,
            "Save my current crosshair as a new preset called Ranked",
            None,
            None,
        )
        .expect("save current crosshair");
        assert_eq!(state.profiles.len(), 5);
        assert_eq!(state.profiles[4].name, "Ranked");
        assert_eq!(state.profiles[4].config.color, "#ec4899");
        assert_eq!(state.profiles[4].config.length, 18);

        apply_preset_or_crosshair_to_state(&mut state, "Switch to preset 3", None, None)
            .expect("switch to preset 3");
        assert_eq!(state.active_profile_id.as_deref(), Some("slot-3"));

        apply_preset_or_crosshair_to_state(&mut state, "Rename preset 2 to Sheriff", None, None)
            .expect("rename preset 2");
        assert_eq!(state.profiles[1].name, "Sheriff");

        apply_preset_or_crosshair_to_state(&mut state, "Delete preset 5", None, None)
            .expect("delete preset 5");
        assert_eq!(state.profiles.len(), 4);
    }

    #[test]
    fn creates_multi_preset_pack_and_puts_crosshairs_on_them() {
        let mut state = LuairState::default();

        let (summary, _) = apply_preset_or_crosshair_to_state(
            &mut state,
            "Create presets for Rifle, Pistol, and Shotgun",
            None,
            None,
        )
        .expect("create pack");

        assert_eq!(state.profiles.len(), 7);
        assert_eq!(state.profiles[4].name, "Rifle");
        assert!(!state.profiles[4].config.hide_bars);
        assert_eq!(state.profiles[5].name, "Pistol");
        assert!(state.profiles[5].config.dot);
        assert!(state.profiles[5].config.hide_bars);
        assert_eq!(state.profiles[6].name, "Shotgun");
        assert!(state.profiles[6].config.has_circle);
        assert!(summary.contains("Created 3 presets"));
    }
}
