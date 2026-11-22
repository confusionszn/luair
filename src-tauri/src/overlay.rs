use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

/// Fits a window to the primary monitor's physical bounds.
pub fn fit_to_primary_monitor(window: &WebviewWindow) -> Result<(), String> {
    if let Some(monitor) = window.primary_monitor().map_err(|e| e.to_string())? {
        let position = *monitor.position();
        let size = *monitor.size();
        window.set_position(position).map_err(|e| e.to_string())?;
        window.set_size(size).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Configures the transparent, click-through, always-on-top crosshair overlay window.
pub fn configure_overlay_window(window: &WebviewWindow) -> Result<(), String> {
    fit_to_primary_monitor(window)?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::{
            Graphics::Dwm::{
                DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
                DWMWA_TRANSITIONS_FORCEDISABLED,
            },
            UI::{
                Controls::MARGINS,
                WindowsAndMessaging::{
                    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
                    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_LAYERED,
                    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
                },
            },
        };

        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0;
        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let overlay_flags =
                (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | overlay_flags);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );

            // Extend DWM sheet of glass across the full borderless window for clean DirectX/GDI alpha compositing.
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

            let disable_transitions: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                &disable_transitions as *const i32 as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = window.set_visible_on_all_workspaces(true);
        let _ = window.set_skip_taskbar(true);
    }

    Ok(())
}

pub fn toggle_menu_window(app: &AppHandle) -> Result<bool, String> {
    let menu = app
        .get_webview_window("menu")
        .ok_or_else(|| "Menu window not found".to_string())?;

    let is_visible = menu.is_visible().map_err(|e| e.to_string())?;
    if is_visible {
        menu.hide().map_err(|e| e.to_string())?;
        let _ = app.emit("luair://menu-visibility", false);
        Ok(false)
    } else {
        menu.show().map_err(|e| e.to_string())?;
        let _ = menu.unminimize();
        let _ = menu.set_focus();
        let _ = app.emit("luair://menu-visibility", true);
        Ok(true)
    }
}

pub fn hide_menu_window(app: &AppHandle) -> Result<(), String> {
    if let Some(menu) = app.get_webview_window("menu") {
        menu.hide().map_err(|e| e.to_string())?;
        let _ = app.emit("luair://menu-visibility", false);
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_menu(app: AppHandle) -> Result<bool, String> {
    toggle_menu_window(&app)
}

#[tauri::command]
pub fn hide_menu(app: AppHandle) -> Result<(), String> {
    hide_menu_window(&app)
}

#[tauri::command]
pub fn minimize_menu(app: AppHandle) -> Result<(), String> {
    if let Some(menu) = app.get_webview_window("menu") {
        menu.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn reset_overlay_bounds(app: AppHandle) -> Result<(), String> {
    if let Some(overlay) = app.get_webview_window("overlay") {
        configure_overlay_window(&overlay)?;
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
