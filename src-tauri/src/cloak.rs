use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::{state::AppState, storage};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloakStatus {
    pub enabled: bool,
    pub cloak_menu: bool,
    pub platform: String,
    pub mechanism: String,
    pub supported: bool,
    pub overlay_verified: bool,
}

#[cfg(target_os = "windows")]
pub fn apply_window_cloak(window: &WebviewWindow, enabled: bool) -> Result<bool, String> {
    use windows_sys::Win32::{
        Foundation::GetLastError,
        UI::WindowsAndMessaging::{
            GetWindowDisplayAffinity, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
        },
    };

    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0;
    let target_affinity = if enabled {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };

    let ok = unsafe { SetWindowDisplayAffinity(hwnd, target_affinity) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("SetWindowDisplayAffinity failed (Win32 error {err})"));
    }

    let mut actual: u32 = 0;
    let queried = unsafe { GetWindowDisplayAffinity(hwnd, &mut actual) };
    Ok(queried != 0 && actual == target_affinity)
}

#[cfg(target_os = "macos")]
pub fn apply_window_cloak(window: &WebviewWindow, enabled: bool) -> Result<bool, String> {
    window
        .set_content_protected(enabled)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(target_os = "linux")]
pub fn apply_window_cloak(window: &WebviewWindow, enabled: bool) -> Result<bool, String> {
    window
        .set_content_protected(enabled)
        .map_err(|e| e.to_string())?;
    Ok(false)
}

#[cfg(target_os = "windows")]
pub fn query_window_cloak_verified(window: &WebviewWindow, expected_enabled: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    };

    let Ok(hwnd) = window.hwnd() else {
        return false;
    };
    let expected = if expected_enabled {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    let mut actual: u32 = 0;
    let ok = unsafe { GetWindowDisplayAffinity(hwnd.0, &mut actual) };
    ok != 0 && actual == expected
}

#[cfg(target_os = "macos")]
pub fn query_window_cloak_verified(_window: &WebviewWindow, _expected_enabled: bool) -> bool {
    true
}

#[cfg(target_os = "linux")]
pub fn query_window_cloak_verified(_window: &WebviewWindow, _expected_enabled: bool) -> bool {
    false
}

fn platform_metadata() -> (&'static str, &'static str, bool) {
    #[cfg(target_os = "windows")]
    {
        ("windows", "WDA_EXCLUDEFROMCAPTURE (0x11)", true)
    }
    #[cfg(target_os = "macos")]
    {
        ("macos", "NSWindowSharingNone", true)
    }
    #[cfg(target_os = "linux")]
    {
        let is_wayland = std::env::var("WAYLAND_DISPLAY")
            .is_ok_and(|v| !v.trim().is_empty())
            || std::env::var("XDG_SESSION_TYPE")
                .is_ok_and(|v| v.eq_ignore_ascii_case("wayland"));
        if is_wayland {
            ("linux", "Wayland Portal / Compositor Best-Effort", false)
        } else {
            ("linux", "X11 Window Property Best-Effort", false)
        }
    }
}

pub fn apply_cloak_to_app(
    app: &AppHandle,
    enabled: bool,
    cloak_menu: bool,
) -> Result<CloakStatus, String> {
    let mut overlay_verified = false;

    if let Some(overlay_win) = app.get_webview_window("overlay") {
        overlay_verified = apply_window_cloak(&overlay_win, enabled)?;
    }

    if let Some(menu_win) = app.get_webview_window("menu") {
        let _ = apply_window_cloak(&menu_win, enabled && cloak_menu);
    }

    let (platform, mechanism, supported) = platform_metadata();
    Ok(CloakStatus {
        enabled,
        cloak_menu,
        platform: platform.to_string(),
        mechanism: mechanism.to_string(),
        supported,
        overlay_verified,
    })
}

#[tauri::command]
pub fn set_capture_cloak(
    app: AppHandle,
    app_state: State<'_, AppState>,
    enabled: bool,
    cloak_menu: Option<bool>,
) -> Result<CloakStatus, String> {
    let next_state = {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        guard.ui.streamproof = enabled;
        if let Some(cm) = cloak_menu {
            guard.ui.cloak_menu = cm;
        }
        guard.clone()
    };

    let status = apply_cloak_to_app(&app, next_state.ui.streamproof, next_state.ui.cloak_menu)?;
    storage::save_to_disk(&app, &next_state)?;
    let _ = app.emit("luair://state-updated", &next_state);
    Ok(status)
}

#[tauri::command]
pub fn get_capture_cloak_status(
    app: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<CloakStatus, String> {
    let snapshot = app_state.snapshot();
    let overlay_verified = app
        .get_webview_window("overlay")
        .map(|w| query_window_cloak_verified(&w, snapshot.ui.streamproof))
        .unwrap_or(false);

    let (platform, mechanism, supported) = platform_metadata();
    Ok(CloakStatus {
        enabled: snapshot.ui.streamproof,
        cloak_menu: snapshot.ui.cloak_menu,
        platform: platform.to_string(),
        mechanism: mechanism.to_string(),
        supported,
        overlay_verified,
    })
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "windows")]
    #[test]
    fn win32_capture_constants_match_msdn() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{WDA_EXCLUDEFROMCAPTURE, WDA_NONE};
        assert_eq!(WDA_EXCLUDEFROMCAPTURE, 0x0000_0011);
        assert_eq!(WDA_NONE, 0x0000_0000);
    }
}
