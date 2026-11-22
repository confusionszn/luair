use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use candle_core::{quantized::gguf_file, Device, Tensor};
use candle_transformers::{
    generation::LogitsProcessor,
    models::{quantized_llama, quantized_qwen2},
    utils::apply_repeat_penalty,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArch {
    Llama,
    Qwen2,
}

impl ModelArch {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelArch::Llama => "llama",
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

pub const MODEL_CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "smollm2-360m",
        name: "SmolLM2-360M-Instruct",
        arch: ModelArch::Llama,
        quantization: "Q4_K_M",
        approx_size_mb: 258,
        gguf_url: "https://huggingface.co/bartowski/SmolLM2-360M-Instruct-GGUF/resolve/main/SmolLM2-360M-Instruct-Q4_K_M.gguf",
        tokenizer_url: "https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct/resolve/main/tokenizer.json",
        description: "Ultra-lightweight 360M Llama-family model for fast CPU chat & JSON synthesis.",
    },
    ModelSpec {
        id: "qwen2.5-0.5b",
        name: "Qwen2.5-0.5B-Instruct",
        arch: ModelArch::Qwen2,
        quantization: "Q4_K_M",
        approx_size_mb: 469,
        gguf_url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf",
        tokenizer_url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/main/tokenizer.json",
        description: "High-accuracy 0.5B Qwen2.5 instruction model with strong structured JSON output.",
    },
];

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

enum QuantizedWeights {
    Llama(quantized_llama::ModelWeights),
    Qwen2(quantized_qwen2::ModelWeights),
}

impl QuantizedWeights {
    fn clear_kv_cache(&mut self) {
        match self {
            QuantizedWeights::Llama(m) => m.clear_kv_cache(),
            QuantizedWeights::Qwen2(m) => m.clear_kv_cache(),
        }
    }

    fn forward(&mut self, x: &Tensor, index_pos: usize) -> candle_core::Result<Tensor> {
        match self {
            QuantizedWeights::Llama(m) => m.forward(x, index_pos),
            QuantizedWeights::Qwen2(m) => m.forward(x, index_pos),
        }
    }
}

pub struct LoadedCandleModel {
    pub model_id: String,
    pub model_name: String,
    weights: QuantizedWeights,
    tokenizer: Tokenizer,
    eos_tokens: Vec<u32>,
}

impl LoadedCandleModel {
    pub fn load_from_dir(spec: &ModelSpec, dir: &Path) -> Result<Self, String> {
        let gguf_path = dir.join("model.gguf");
        let tok_path = dir.join("tokenizer.json");

        if !gguf_path.exists() || !tok_path.exists() {
            return Err(format!(
                "Model '{}' is incomplete on disk. Please download it first.",
                spec.name
            ));
        }

        let mut file = File::open(&gguf_path).map_err(|e| format!("Failed opening GGUF: {e}"))?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| format!("Failed parsing GGUF header: {e}"))?;

        let device = Device::Cpu;
        let weights = match spec.arch {
            ModelArch::Llama => {
                let m = quantized_llama::ModelWeights::from_gguf(content, &mut file, &device)
                    .map_err(|e| format!("Failed loading Llama GGUF weights: {e}"))?;
                QuantizedWeights::Llama(m)
            }
            ModelArch::Qwen2 => {
                let m = quantized_qwen2::ModelWeights::from_gguf(content, &mut file, &device)
                    .map_err(|e| format!("Failed loading Qwen2 GGUF weights: {e}"))?;
                QuantizedWeights::Qwen2(m)
            }
        };

        let tokenizer = Tokenizer::from_file(&tok_path)
            .map_err(|e| format!("Failed loading tokenizer.json: {e}"))?;

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
        let mut formatted = format!("<|im_start|>system\n{system_prompt}<|im_end|>\n");
        let recent = if history.len() > 6 {
            &history[history.len() - 6..]
        } else {
            history
        };
        for msg in recent {
            let role = if msg.role == "assistant" {
                "assistant"
            } else {
                "user"
            };
            formatted.push_str(&format!("<|im_start|>{role}\n{}<|im_end|>\n", msg.content));
        }
        formatted.push_str(&format!("<|im_start|>user\n{user_prompt}<|im_end|>\n<|im_start|>assistant\n"));

        let encoding = self
            .tokenizer
            .encode(formatted, true)
            .map_err(|e| format!("Tokenization error: {e}"))?;
        let mut prompt_tokens = encoding.get_ids().to_vec();
        if prompt_tokens.is_empty() {
            return Err("Empty prompt tokens".to_string());
        }
        if prompt_tokens.len() > 1400 {
            prompt_tokens = prompt_tokens[prompt_tokens.len() - 1400..].to_vec();
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
        let mut logits_proc = LogitsProcessor::new(seed, Some(temperature), Some(0.9));

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
        Ok(decoded.trim().to_string())
    }
}

pub struct AiManager {
    models_dir: PathBuf,
    loaded_model: Mutex<Option<LoadedCandleModel>>,
    downloading_id: Mutex<Option<String>>,
    cancel_download: Arc<AtomicBool>,
}

impl AiManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            loaded_model: Mutex::new(None),
            downloading_id: Mutex::new(None),
            cancel_download: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn model_dir(&self, model_id: &str) -> PathBuf {
        self.models_dir.join(model_id)
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

        let models = MODEL_CATALOG
            .iter()
            .map(|spec| {
                let dir = self.model_dir(spec.id);
                let gguf_path = dir.join("model.gguf");
                let tok_path = dir.join("tokenizer.json");
                let downloaded = gguf_path.exists() && tok_path.exists();
                let local_size_bytes = fs::metadata(&gguf_path).map(|m| m.len()).unwrap_or(0);
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

        let active_backend = match loaded_name {
            Some(name) => format!("Candle GGUF (CPU · {name})"),
            None => "Offline Intent Engine (Ready · Load a GGUF model for full LLM)".to_string(),
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
        let mut guard = self.loaded_model.lock().ok()?;
        let model = guard.as_mut()?;
        let model_label = format!("Candle GGUF ({})", model.model_name);
        match model.generate(system_prompt, history, user_prompt, max_tokens, temperature) {
            Ok(text) if !text.is_empty() => Some((text, model_label)),
            _ => None,
        }
    }
}

fn find_model_spec(model_id: &str) -> Result<&'static ModelSpec, String> {
    MODEL_CATALOG
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model id '{model_id}'"))
}

fn download_file_streaming(
    app: &AppHandle,
    agent: &ureq::Agent,
    url: &str,
    dest: &Path,
    model_id: &str,
    stage: &str,
    cancel_flag: &AtomicBool,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let part_path = dest.with_extension("part");
    let mut resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("HTTP download failed ({stage}): {e}"))?;

    let total_bytes = resp.body().content_length();
    let mut reader = resp.body_mut().as_reader();
    let mut out = File::create(&part_path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded_bytes: u64 = 0;
    let mut last_emit = Instant::now() - Duration::from_millis(250);

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            drop(out);
            let _ = fs::remove_file(&part_path);
            return Err("Download canceled by user.".to_string());
        }

        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Stream read error: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded_bytes += n as u64;

        let now = Instant::now();
        if now.duration_since(last_emit) >= Duration::from_millis(120) {
            last_emit = now;
            let percent = total_bytes
                .filter(|&t| t > 0)
                .map(|t| ((downloaded_bytes as f64 / t as f64) * 100.0).min(100.0) as f32)
                .unwrap_or(0.0);
            let _ = app.emit(
                "luair://ai-download-progress",
                AiDownloadProgress {
                    model_id: model_id.to_string(),
                    stage: stage.to_string(),
                    downloaded_bytes,
                    total_bytes,
                    percent,
                    message: format!("Downloading {stage}..."),
                },
            );
        }
    }

    out.flush().map_err(|e| e.to_string())?;
    drop(out);
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(&part_path, dest).map_err(|e| e.to_string())?;

    let _ = app.emit(
        "luair://ai-download-progress",
        AiDownloadProgress {
            model_id: model_id.to_string(),
            stage: stage.to_string(),
            downloaded_bytes,
            total_bytes: total_bytes.or(Some(downloaded_bytes)),
            percent: 100.0,
            message: format!("Finished {stage}."),
        },
    );

    Ok(())
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
    } else if lower.contains("vault") || lower.contains("account") || lower.contains("privacy") || lower.contains("offline") {
        "Everything in Luair runs 100% locally with zero API keys or cloud servers. Your local account is protected in embedded SQLite with Argon2id password hashing and AES-256-GCM encryption.".to_string()
    } else {
        "I'm ready locally! Download and load `SmolLM2-360M` or `Qwen2.5-0.5B` above for full Candle GGUF conversational reasoning, or ask me right now to create a crosshair (e.g. \"Make a clean lime green CS2 dot with a thin border and bind it to key 2\"), design a theme, or toggle OBS Shield.".to_string()
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

    let result = tauri::async_runtime::spawn_blocking(move || {
        let dir = ai.model_dir(spec.id);
        let config = ureq::Agent::config_builder()
            .timeout_global(None)
            .timeout_connect(Some(Duration::from_secs(20)))
            .build();
        let agent: ureq::Agent = config.into();

        let tok_res = download_file_streaming(
            &app,
            &agent,
            spec.tokenizer_url,
            &dir.join("tokenizer.json"),
            spec.id,
            "tokenizer",
            &ai.cancel_download,
        );
        if let Err(e) = tok_res {
            if let Ok(mut dl) = ai.downloading_id.lock() {
                *dl = None;
            }
            return Err(e);
        }

        let gguf_res = download_file_streaming(
            &app,
            &agent,
            spec.gguf_url,
            &dir.join("model.gguf"),
            spec.id,
            "model",
            &ai.cancel_download,
        );
        if let Ok(mut dl) = ai.downloading_id.lock() {
            *dl = None;
        }
        gguf_res?;
        Ok(ai.status())
    })
    .await
    .map_err(|e| format!("Download task error: {e}"))?;

    result
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
        let sys = "You are Luair AI, the built-in local assistant for Luair, an open-source, lightweight Rust + Tauri desktop crosshair overlay. Be concise and helpful. Mention OBS Shield (WDA_EXCLUDEFROMCAPTURE), non-blocking rdev keybinds (keyboard + mouse buttons), Liquid Glass / Solid themes, and the Crosshair Studio accurately.";
        match ai.try_generate(sys, &messages, &prompt_clone, 220, 0.5) {
            Some((text, engine)) => (Some(text), engine),
            None => (None, "Offline Rule Engine".to_string()),
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
    fn catalog_contains_qwen_and_smollm2_gguf_entries() {
        assert!(MODEL_CATALOG.iter().any(|m| m.id == "smollm2-360m" && m.arch == ModelArch::Llama));
        assert!(MODEL_CATALOG.iter().any(|m| m.id == "qwen2.5-0.5b" && m.arch == ModelArch::Qwen2));
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
