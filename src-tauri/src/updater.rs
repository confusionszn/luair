use std::{cmp::Ordering, sync::Mutex, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const GITHUB_RELEASES_URL: &str = "https://github.com/confusionszn/luair/releases";
pub const UPDATER_ENDPOINT: &str =
    "https://github.com/confusionszn/luair/releases/latest/download/latest.json";

#[derive(Default)]
pub struct PendingUpdateState(pub Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_notes: Option<String>,
    pub release_date: Option<String>,
    pub release_url: String,
    pub endpoint: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgressEvent {
    pub stage: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformBuildInfo {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub target_triple: String,
    pub display_server: String,
    pub overlay_backend: String,
    pub packaging_targets: Vec<String>,
    pub updater_endpoint: String,
}

fn parse_semver_core(raw: &str) -> ([u64; 3], bool) {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    let (core, has_prerelease) = match trimmed.split_once('-') {
        Some((head, pre)) => (head, !pre.trim().is_empty()),
        None => (trimmed, false),
    };
    let core = core.split('+').next().unwrap_or(core);
    let mut parts = core.split('.');
    let major = parts
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    ([major, minor, patch], has_prerelease)
}

pub fn compare_semver(a: &str, b: &str) -> Ordering {
    let (va, pre_a) = parse_semver_core(a);
    let (vb, pre_b) = parse_semver_core(b);
    match va.cmp(&vb) {
        Ordering::Equal => match (pre_a, pre_b) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => Ordering::Equal,
        },
        other => other,
    }
}

pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    compare_semver(candidate, current) == Ordering::Greater
}

pub fn format_release_notes(raw_body: Option<&str>, version: &str) -> String {
    let body = raw_body.unwrap_or("").trim();
    if body.is_empty() {
        format!(
            "Luair v{version} includes stability, performance, and cross-platform overlay improvements."
        )
    } else {
        body.to_string()
    }
}

pub fn detect_linux_display_server(
    wayland_display: Option<&str>,
    x11_display: Option<&str>,
    xdg_session_type: Option<&str>,
) -> String {
    let has_wayland = wayland_display.is_some_and(|v| !v.trim().is_empty())
        || xdg_session_type.is_some_and(|v| v.eq_ignore_ascii_case("wayland"));
    let has_x11 = x11_display.is_some_and(|v| !v.trim().is_empty());

    match (has_wayland, has_x11) {
        (true, true) => "Wayland + XWayland (X11 global hotkeys active)".to_string(),
        (true, false) => "Wayland Native".to_string(),
        (false, true) => "X11 (Xorg)".to_string(),
        (false, false) => "Linux Desktop Display".to_string(),
    }
}

pub fn format_updater_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("404") || lower.contains("status: 404") || lower.contains("not found") {
        "No published release manifest was found at the GitHub Releases endpoint yet.".to_string()
    } else if lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("timed out")
        || lower.contains("network")
    {
        "Unable to reach GitHub Releases. Verify your internet connection and retry.".to_string()
    } else {
        format!("Update check could not complete right now ({raw}).")
    }
}

fn current_target_triple() -> String {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc".to_string()
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        "aarch64-pc-windows-msvc".to_string()
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu".to_string()
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu".to_string()
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin (universal-apple-darwin)".to_string()
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin (universal-apple-darwin)".to_string()
    } else {
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    }
}

pub fn build_platform_info() -> PlatformBuildInfo {
    let display_server = if cfg!(target_os = "windows") {
        "DirectX / GDI (DWM Borderless Composition)".to_string()
    } else if cfg!(target_os = "macos") {
        "Quartz / Metal (Universal Intel + Apple Silicon)".to_string()
    } else {
        let wl = std::env::var("WAYLAND_DISPLAY").ok();
        let x11 = std::env::var("DISPLAY").ok();
        let xdg = std::env::var("XDG_SESSION_TYPE").ok();
        detect_linux_display_server(wl.as_deref(), x11.as_deref(), xdg.as_deref())
    };

    let overlay_backend = if cfg!(target_os = "windows") {
        "WebView2 + WS_EX_LAYERED / WDA_EXCLUDEFROMCAPTURE".to_string()
    } else if cfg!(target_os = "macos") {
        "WKWebView + NSWindowSharingNone".to_string()
    } else {
        "WebKitGTK 4.1 + GTK3 Overlay (X11 & Wayland)".to_string()
    };

    PlatformBuildInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        target_triple: current_target_triple(),
        display_server,
        overlay_backend,
        packaging_targets: vec![
            "Windows MSVC x86_64 (NSIS & MSI)".to_string(),
            "Linux Debian (.deb)".to_string(),
            "Linux Arch (PKGBUILD)".to_string(),
            "Linux Fedora (.rpm)".to_string(),
            "Linux Flatpak & AppImage (X11/Wayland)".to_string(),
            "macOS Universal Binary (x86_64 + arm64 DMG)".to_string(),
        ],
        updater_endpoint: UPDATER_ENDPOINT.to_string(),
    }
}

#[tauri::command]
pub fn get_platform_info() -> PlatformBuildInfo {
    build_platform_info()
}

#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    pending: State<'_, PendingUpdateState>,
) -> Result<UpdateCheckResult, String> {
    let current_version = app.package_info().version.to_string();
    let updater = app.updater().map_err(|e| e.to_string())?;

    match updater.check().await {
        Ok(Some(update)) => {
            let latest_version = update.version.clone();
            let current = update.current_version.clone();
            let release_notes = Some(format_release_notes(
                update.body.as_deref(),
                &latest_version,
            ));
            let release_date = update.date.map(|d| d.to_string());
            let is_newer = is_newer_version(&current, &latest_version);

            if let Ok(mut guard) = pending.0.lock() {
                *guard = if is_newer { Some(update) } else { None };
            }

            if is_newer {
                Ok(UpdateCheckResult {
                    available: true,
                    current_version: current,
                    latest_version: Some(latest_version.clone()),
                    release_notes,
                    release_date,
                    release_url: format!("{GITHUB_RELEASES_URL}/tag/v{latest_version}"),
                    endpoint: UPDATER_ENDPOINT.to_string(),
                    status: "update-available".to_string(),
                    message: format!("Luair v{latest_version} is ready to install."),
                })
            } else {
                Ok(UpdateCheckResult {
                    available: false,
                    current_version: current.clone(),
                    latest_version: Some(latest_version),
                    release_notes: None,
                    release_date,
                    release_url: GITHUB_RELEASES_URL.to_string(),
                    endpoint: UPDATER_ENDPOINT.to_string(),
                    status: "up-to-date".to_string(),
                    message: format!("Luair v{current} is up to date."),
                })
            }
        }
        Ok(None) => {
            if let Ok(mut guard) = pending.0.lock() {
                *guard = None;
            }
            Ok(UpdateCheckResult {
                available: false,
                current_version: current_version.clone(),
                latest_version: Some(current_version.clone()),
                release_notes: None,
                release_date: None,
                release_url: GITHUB_RELEASES_URL.to_string(),
                endpoint: UPDATER_ENDPOINT.to_string(),
                status: "up-to-date".to_string(),
                message: format!("Luair v{current_version} is up to date."),
            })
        }
        Err(err) => {
            if let Ok(mut guard) = pending.0.lock() {
                *guard = None;
            }
            Ok(UpdateCheckResult {
                available: false,
                current_version,
                latest_version: None,
                release_notes: None,
                release_date: None,
                release_url: GITHUB_RELEASES_URL.to_string(),
                endpoint: UPDATER_ENDPOINT.to_string(),
                status: "offline-or-unreleased".to_string(),
                message: format_updater_error(&err.to_string()),
            })
        }
    }
}

#[tauri::command]
pub async fn download_and_install_update(
    app: AppHandle,
    pending: State<'_, PendingUpdateState>,
) -> Result<(), String> {
    let cached_update = pending.0.lock().ok().and_then(|mut guard| guard.take());

    let update = match cached_update {
        Some(u) => u,
        None => {
            let updater = app.updater().map_err(|e| e.to_string())?;
            updater
                .check()
                .await
                .map_err(|e| format_updater_error(&e.to_string()))?
                .ok_or_else(|| "No update is currently available.".to_string())?
        }
    };

    let target_version = update.version.clone();
    let _ = app.emit(
        "luair://update-progress",
        UpdateProgressEvent {
            stage: "downloading".to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            percent: 0.0,
            message: format!("Downloading Luair v{target_version}..."),
        },
    );

    let mut downloaded: u64 = 0;
    let app_for_chunks = app.clone();
    let app_for_finish = app.clone();

    let result = update
        .download_and_install(
            move |chunk_len, content_len| {
                downloaded = downloaded.saturating_add(chunk_len as u64);
                let percent = content_len
                    .filter(|&total| total > 0)
                    .map(|total| ((downloaded as f64 / total as f64) * 100.0).min(100.0))
                    .unwrap_or(0.0);
                let _ = app_for_chunks.emit(
                    "luair://update-progress",
                    UpdateProgressEvent {
                        stage: "downloading".to_string(),
                        downloaded_bytes: downloaded,
                        total_bytes: content_len,
                        percent,
                        message: format!("Downloading Luair v{target_version}..."),
                    },
                );
            },
            move || {
                let _ = app_for_finish.emit(
                    "luair://update-progress",
                    UpdateProgressEvent {
                        stage: "installing".to_string(),
                        downloaded_bytes: 0,
                        total_bytes: None,
                        percent: 100.0,
                        message: "Verifying Minisign signature and installing update...".to_string(),
                    },
                );
            },
        )
        .await;

    if let Err(err) = result {
        let msg = format!("Failed to install update: {err}");
        let _ = app.emit(
            "luair://update-progress",
            UpdateProgressEvent {
                stage: "error".to_string(),
                downloaded_bytes: 0,
                total_bytes: None,
                percent: 0.0,
                message: msg.clone(),
            },
        );
        return Err(msg);
    }

    let _ = app.emit(
        "luair://update-progress",
        UpdateProgressEvent {
            stage: "restarting".to_string(),
            downloaded_bytes: downloaded,
            total_bytes: Some(downloaded),
            percent: 100.0,
            message: "Update installed. Restarting Luair...".to_string(),
        },
    );

    let app_restart = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        app_restart.restart();
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_base64_str(input: &str) -> Result<String, String> {
        const TABLE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        if clean.len() % 4 != 0 {
            return Err("invalid base64 length".to_string());
        }
        let mut out = Vec::with_capacity((clean.len() / 4) * 3);
        let bytes = clean.as_bytes();
        for chunk in bytes.chunks_exact(4) {
            let mut vals = [0u8; 4];
            let mut pad = 0usize;
            for (idx, &ch) in chunk.iter().enumerate() {
                if ch == b'=' {
                    vals[idx] = 0;
                    pad += 1;
                } else {
                    let pos = TABLE
                        .as_bytes()
                        .iter()
                        .position(|&b| b == ch)
                        .ok_or_else(|| format!("invalid base64 char: {}", ch as char))?;
                    vals[idx] = pos as u8;
                }
            }
            out.push((vals[0] << 2) | (vals[1] >> 4));
            if pad < 2 {
                out.push((vals[1] << 4) | (vals[2] >> 2));
            }
            if pad < 1 {
                out.push((vals[2] << 6) | vals[3]);
            }
        }
        String::from_utf8(out).map_err(|e| e.to_string())
    }

    #[test]
    fn semver_comparison_handles_prefixes_and_prereleases() {
        assert!(is_newer_version("2.0.0", "2.0.1"));
        assert!(is_newer_version("v2.0.0", "v2.1.0"));
        assert!(is_newer_version("2.0.0-rc.1", "2.0.0"));
        assert!(!is_newer_version("2.1.0", "2.0.9"));
        assert!(!is_newer_version("2.0.0", "2.0.0"));
    }

    #[test]
    fn release_notes_formatting_provides_fallback() {
        assert!(format_release_notes(None, "2.1.0").contains("Luair v2.1.0"));
        assert!(format_release_notes(Some("   "), "2.1.0").contains("Luair v2.1.0"));
        assert_eq!(
            format_release_notes(Some("- Added Metal compositing"), "2.1.0"),
            "- Added Metal compositing"
        );
    }

    #[test]
    fn linux_display_server_detection_covers_wayland_xwayland_and_x11() {
        assert_eq!(
            detect_linux_display_server(Some("wayland-0"), Some(":0"), Some("wayland")),
            "Wayland + XWayland (X11 global hotkeys active)"
        );
        assert_eq!(
            detect_linux_display_server(Some("wayland-0"), None, Some("wayland")),
            "Wayland Native"
        );
        assert_eq!(
            detect_linux_display_server(None, Some(":0"), Some("x11")),
            "X11 (Xorg)"
        );
    }

    #[test]
    fn tauri_conf_updater_schema_and_pubkey_are_valid() {
        let raw = include_str!("../tauri.conf.json");
        let parsed: serde_json::Value = serde_json::from_str(raw).expect("valid tauri.conf.json");

        assert_eq!(parsed["bundle"]["createUpdaterArtifacts"], true);
        assert_eq!(parsed["plugins"]["updater"]["windows"]["installMode"], "passive");

        let endpoint = parsed["plugins"]["updater"]["endpoints"][0]
            .as_str()
            .expect("updater endpoint string");
        assert!(endpoint.contains("github.com"));
        assert!(endpoint.ends_with("latest.json"));

        let pubkey_b64 = parsed["plugins"]["updater"]["pubkey"]
            .as_str()
            .expect("updater pubkey string");
        let decoded = decode_base64_str(pubkey_b64).expect("valid base64 pubkey");
        assert!(decoded.contains("untrusted comment: minisign public key:"));
        assert!(decoded.contains("\nRW") || decoded.starts_with("RW"));
    }

    #[test]
    fn platform_info_reports_valid_runtime_metadata() {
        let info = build_platform_info();
        assert_eq!(info.app_version, env!("CARGO_PKG_VERSION"));
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(!info.target_triple.is_empty());
        assert!(info.packaging_targets.len() >= 6);
    }
}
