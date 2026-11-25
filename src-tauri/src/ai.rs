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
        .user_agent("Luair/2.0.3")
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

fn detect_named_color(lower: &str) -> Option<(&'static str, &'static str)> {
    if lower.contains("lime") {
        Some(("#84cc16", "lime green"))
    } else if lower.contains("emerald") || lower.contains("matrix") {
        Some(("#10b981", "emerald green"))
    } else if lower.contains("green") {
        Some(("#22c55e", "green"))
    } else if lower.contains("cyan") || lower.contains("aqua") || lower.contains("ice") {
        Some(("#22d3ee", "cyan"))
    } else if lower.contains("blue") || lower.contains("cobalt") || lower.contains("ocean") {
        Some(("#38bdf8", "electric blue"))
    } else if lower.contains("purple") || lower.contains("violet") {
        Some(("#a855f7", "neon purple"))
    } else if lower.contains("magenta") || lower.contains("pink") {
        Some(("#ec4899", "magenta pink"))
    } else if lower.contains("crimson") || lower.contains("red") {
        Some(("#f43f5e", "crimson red"))
    } else if lower.contains("gold") || lower.contains("yellow") || lower.contains("amber") {
        Some(("#f59e0b", "solar gold"))
    } else if lower.contains("orange") {
        Some(("#fb923c", "orange"))
    } else if lower.contains("white") {
        Some(("#ffffff", "crisp white"))
    } else {
        None
    }
}

fn detect_requested_hotkey(prompt: &str) -> Option<String> {
    let lower = prompt.to_ascii_lowercase();
    for token in ["mouse4", "mouse5", "mousemiddle", "mouseright", "mouseleft", "xbutton1", "xbutton2"] {
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
        if matches!(w, "key" | "slot" | "hotkey" | "button" | "bind") {
            if let Some(&next) = words.get(idx + 1) {
                if next == "to" || next == "on" || next == "as" {
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
        }
    } else if lower.contains("rainbow") || lower.contains("rgb") {
        cfg.color_mode = "rainbow".to_string();
        cfg.rainbow = true;
    } else if lower.contains("solid") {
        cfg.color_mode = "solid".to_string();
        cfg.rainbow = false;
    }

    let wants_dot_only = lower.contains("dot")
        && (lower.contains("only")
            || lower.contains("cs2")
            || lower.contains("clean")
            || (!lower.contains("cross") && !lower.contains("t-shape") && !lower.contains("bars")));

    if wants_dot_only {
        cfg.dot = true;
        cfg.hide_bars = true;
        cfg.t_shape = false;
        cfg.dot_size = if lower.contains("tiny") || lower.contains("small") {
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
        if lower.contains("cross") || lower.contains("bars") {
            cfg.hide_bars = false;
        }
        if lower.contains("t-shape") || lower.contains("t shape") {
            cfg.t_shape = true;
            cfg.hide_bars = false;
        }
    }

    if lower.contains("circle") || lower.contains("ring") || lower.contains("shotgun") {
        cfg.has_circle = true;
        if lower.contains("large") || lower.contains("wide") {
            cfg.circle_radius = 28;
        }
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
    if lower.contains("dynamic") || lower.contains("spread") {
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
        "I'm ready locally! Download Luair Candle (475 MB) above for full local conversational reasoning, or ask me right now to create a crosshair (e.g. \"Make a clean lime green CS2 dot with a thin border and bind it to key 2\"), design a theme, or toggle OBS Shield.".to_string()
    }
}

fn apply_generated_crosshair(
    app: &AppHandle,
    app_state: &AppState,
    new_cfg: CrosshairConfig,
    bound_hotkey: Option<String>,
    target_slot_id: Option<String>,
) -> Result<LuairState, String> {
    let next_state = {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        guard.crosshair = new_cfg.clone();

        if let Some(slot_id) = target_slot_id {
            if let Some(p) = guard.profiles.iter_mut().find(|p| p.id == slot_id) {
                p.config = new_cfg.clone();
                if let Some(ref hk) = bound_hotkey {
                    p.hotkey = Some(hk.clone());
                }
                guard.active_profile_id = Some(p.id.clone());
            }
        } else if let Some(ref hk) = bound_hotkey {
            let hk_upper = hk.to_ascii_uppercase();
            if let Some(existing) = guard
                .profiles
                .iter_mut()
                .find(|p| p.hotkey.as_deref().map(|k| k.to_ascii_uppercase()) == Some(hk_upper.clone()))
            {
                existing.config = new_cfg.clone();
                guard.active_profile_id = Some(existing.id.clone());
            } else if let Some(slot_idx) = hk.parse::<usize>().ok().filter(|&n| n >= 1 && n <= guard.profiles.len()) {
                let p = &mut guard.profiles[slot_idx - 1];
                p.config = new_cfg.clone();
                p.hotkey = Some(hk.clone());
                guard.active_profile_id = Some(p.id.clone());
            } else if guard.profiles.len() < 10 {
                let id = format!("ai-slot-{}", guard.profiles.len() + 1);
                guard.profiles.push(CrosshairProfile {
                    id: id.clone(),
                    name: format!("AI ({hk})"),
                    hotkey: Some(hk.clone()),
                    config: new_cfg.clone(),
                });
                guard.active_profile_id = Some(id);
            } else if let Some(active_id) = guard.active_profile_id.clone() {
                if let Some(p) = guard.profiles.iter_mut().find(|p| p.id == active_id) {
                    p.config = new_cfg.clone();
                    p.hotkey = Some(hk.clone());
                }
            }
        } else if let Some(active_id) = guard.active_profile_id.clone() {
            if let Some(p) = guard.profiles.iter_mut().find(|p| p.id == active_id) {
                p.config = new_cfg.clone();
            }
        }

        guard.clone()
    };

    app_state.sync_hotkeys(&next_state);
    storage::save_to_disk(app, &next_state)?;
    let _ = app.emit("luair://state-updated", &next_state);
    Ok(next_state)
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

#[tauri::command]
pub async fn ai_make_crosshair(
    app: AppHandle,
    app_state: State<'_, AppState>,
    prompt: String,
    target_slot_id: Option<String>,
) -> Result<AiApplyResult, String> {
    let base_cfg = app_state.snapshot().crosshair;
    let ai = app_state.ai.clone();
    let prompt_clone = prompt.clone();

    let (raw_llm, engine_used) = tauri::async_runtime::spawn_blocking(move || {
        let sys = "You convert user crosshair requests into ONE valid JSON object with any relevant keys from: color (hex), outline (bool), outlineThickness (0-8), outlineColor (hex), length (0-60), thickness (1-24), gap (0-60), dot (bool), dotSize (1-20), dotShape (circle|square), hideBars (bool), tShape (bool), hasCircle (bool), circleRadius (1-200), opacity (0.1-1.0), blur (0-20), colorMode (solid|gradient|rainbow), gradientColor (hex), dynamicSpread (bool), recoilColorEnabled (bool), recoilColor (hex), hotkey (string). Output JSON only.";
        match ai.try_generate(sys, &[], &prompt_clone, 180, 0.2) {
            Some((text, engine)) => (Some(text), engine),
            None => (None, "Offline Intent Engine".to_string()),
        }
    })
    .await
    .map_err(|e| format!("AI generation task error: {e}"))?;

    let llm_json = raw_llm.as_deref().and_then(extract_json_object);
    let (new_cfg, bound_hotkey, summary) =
        synthesize_crosshair_from_prompt(&base_cfg, &prompt, llm_json.as_ref());

    let next_state =
        apply_generated_crosshair(&app, &app_state, new_cfg, bound_hotkey.clone(), target_slot_id)?;

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
        let sys = "You convert user UI theme requests into ONE valid JSON object with keys: name, surfaceMode (liquid-glass|solid-minimal), background (hex), surface (hex), text (hex), mutedText (hex), accent (hex), highlight (hex), blurPx (0-36), glassOpacity (0.35-1.0), borderRefraction (0-100), dynamicLighting (bool), cornerRadius (0-24). Output JSON only.";
        match ai.try_generate(sys, &[], &prompt_clone, 180, 0.25) {
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

    // Check if the user asked chat to directly change Luair state.
    let wants_shield_on = (lower.contains("enable") || lower.contains("turn on"))
        && (lower.contains("shield") || lower.contains("cloak") || lower.contains("obs"));
    let wants_shield_off = (lower.contains("disable") || lower.contains("turn off"))
        && (lower.contains("shield") || lower.contains("cloak") || lower.contains("obs"));
    let wants_crosshair = (lower.contains("make") || lower.contains("create") || lower.contains("set") || lower.contains("switch") || lower.contains("give me"))
        && (lower.contains("crosshair") || lower.contains("dot") || lower.contains("reticle") || lower.contains("t-shape"));
    let wants_theme = (lower.contains("make") || lower.contains("create") || lower.contains("change") || lower.contains("set"))
        && (lower.contains("theme") || lower.contains("liquid glass") || lower.contains("glassmorphism"));

    let ai = app_state.ai.clone();
    let prompt_clone = prompt.clone();

    let (llm_reply, engine_used) = tauri::async_runtime::spawn_blocking(move || {
        let sys = "You are Luair Candle, the built-in offline AI assistant inside Luair, a lightweight Rust + Tauri desktop crosshair overlay. Be concise (2-4 short sentences), accurate, and friendly. Mention OBS Shield (WDA_EXCLUDEFROMCAPTURE), non-blocking global hotkeys, Liquid Glass / Solid themes, and Crosshair Studio accurately.";
        match ai.try_generate(sys, &messages, &prompt_clone, 140, 0.45) {
            Some((text, engine)) => (Some(text), engine),
            None => (None, "Offline Intent Engine".to_string()),
        }
    })
    .await
    .map_err(|e| format!("AI chat task error: {e}"))?;

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
            reply: llm_reply.unwrap_or_else(|| format!("Done — {action}.")),
            engine_used,
            applied_action: Some(action),
            updated_state: Some(next_state),
        });
    }

    if wants_theme {
        let llm_json = llm_reply.as_deref().and_then(extract_json_object);
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
            reply: llm_reply.unwrap_or_else(|| summary.clone()),
            engine_used,
            applied_action: Some(summary),
            updated_state: Some(next_state),
        });
    }

    if wants_crosshair {
        let llm_json = llm_reply.as_deref().and_then(extract_json_object);
        let (new_cfg, bound_hotkey, summary) =
            synthesize_crosshair_from_prompt(&snapshot.crosshair, &prompt, llm_json.as_ref());
        let next_state = apply_generated_crosshair(&app, &app_state, new_cfg, bound_hotkey, None)?;
        return Ok(AiChatResponse {
            reply: llm_reply.unwrap_or_else(|| summary.clone()),
            engine_used,
            applied_action: Some(summary),
            updated_state: Some(next_state),
        });
    }

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
}
