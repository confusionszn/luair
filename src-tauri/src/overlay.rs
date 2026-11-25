use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

/// Fits a window to the primary monitor's physical bounds.
/// On Windows, inset by 1px on each side so DWM does not treat the borderless
/// transparent overlay as an opaque fullscreen-optimization (FSO) surface.
pub fn fit_to_primary_monitor(window: &WebviewWindow) -> Result<(), String> {
    if let Some(monitor) = window.primary_monitor().map_err(|e| e.to_string())? {
        let position = *monitor.position();
        let size = *monitor.size();

        #[cfg(target_os = "windows")]
        let (target_pos, target_size) = if size.width > 2 && size.height > 2 {
            (
                tauri::PhysicalPosition::new(position.x + 1, position.y + 1),
                tauri::PhysicalSize::new(size.width - 2, size.height - 2),
            )
        } else {
            (position, size)
        };

        #[cfg(not(target_os = "windows"))]
        let (target_pos, target_size) = (position, size);

        window.set_position(target_pos).map_err(|e| e.to_string())?;
        window.set_size(target_size).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn force_windows_foreground(hwnd: *mut core::ffi::c_void) {
    use windows_sys::Win32::{
        System::Threading::{AttachThreadInput, GetCurrentThreadId},
        UI::{
            Input::KeyboardAndMouse::SetFocus,
            WindowsAndMessaging::{
                AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow,
                GetWindowThreadProcessId, IsIconic, SetForegroundWindow, SetWindowPos, ShowWindow,
                ASFW_ANY, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_RESTORE,
                SW_SHOW,
            },
        },
    };

    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        if IsIconic(hwnd) != 0 {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );

        let fg = GetForegroundWindow();
        let fg_tid = if fg.is_null() {
            0
        } else {
            GetWindowThreadProcessId(fg, std::ptr::null_mut())
        };
        let cur_tid = GetCurrentThreadId();

        if fg_tid != 0 && fg_tid != cur_tid {
            let attached = AttachThreadInput(cur_tid, fg_tid, 1) != 0;
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(hwnd);
            if attached {
                let _ = AttachThreadInput(cur_tid, fg_tid, 0);
            }
        } else {
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(hwnd);
        }
    }
}

#[cfg(target_os = "windows")]
fn hide_tauri_drag_resize_borders(hwnd: *mut core::ffi::c_void) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, ShowWindow, SW_HIDE};

    let class_w = to_wide_null("TAURI_DRAG_RESIZE_BORDERS");
    let title_w = to_wide_null("TAURI_DRAG_RESIZE_WINDOW");
    unsafe {
        let child = FindWindowExW(
            hwnd,
            std::ptr::null_mut(),
            class_w.as_ptr(),
            title_w.as_ptr(),
        );
        if !child.is_null() {
            let _ = ShowWindow(child, SW_HIDE);
        }
    }
}

#[cfg(target_os = "windows")]
fn apply_windows_menu_icon(hwnd: *mut core::ffi::c_void) {
    use windows_sys::Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            GetSystemMetrics, LoadImageW, SendMessageW, SetClassLongPtrW, GCLP_HICON, GCLP_HICONSM,
            ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, SM_CYICON,
            SM_CYSMICON, WM_SETICON,
        },
    };

    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        if hinst.is_null() {
            return;
        }
        let res_id = 32512usize as *const u16;
        let big_w = GetSystemMetrics(SM_CXICON).max(32);
        let big_h = GetSystemMetrics(SM_CYICON).max(32);
        let sm_w = GetSystemMetrics(SM_CXSMICON).max(16);
        let sm_h = GetSystemMetrics(SM_CYSMICON).max(16);

        let hicon_big = LoadImageW(hinst, res_id, IMAGE_ICON, big_w, big_h, LR_DEFAULTCOLOR);
        if !hicon_big.is_null() {
            let _ = SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, hicon_big as isize);
            let _ = SetClassLongPtrW(hwnd, GCLP_HICON, hicon_big as isize);
        }

        let hicon_small = LoadImageW(hinst, res_id, IMAGE_ICON, sm_w, sm_h, LR_DEFAULTCOLOR);
        if !hicon_small.is_null() {
            let _ = SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, hicon_small as isize);
            let _ = SetClassLongPtrW(hwnd, GCLP_HICONSM, hicon_small as isize);
        }
    }
}

/// Configures the menu window on Windows (taskbar/titlebar icon + hides unpainted native resize frame).
pub fn configure_menu_window(window: &WebviewWindow) -> Result<(), String> {
    if let Some(icon) = window.app_handle().default_window_icon().cloned() {
        let _ = window.set_icon(icon);
    }

    #[cfg(target_os = "windows")]
    {
        let win = window.clone();
        let _ = window.run_on_main_thread(move || {
            if let Ok(hwnd) = win.hwnd() {
                apply_windows_menu_icon(hwnd.0);
                hide_tauri_drag_resize_borders(hwnd.0);
            }
        });
    }

    Ok(())
}

/// Configures the transparent, click-through, always-on-top crosshair overlay window.
pub fn configure_overlay_window(window: &WebviewWindow) -> Result<(), String> {
    if let Some(icon) = window.app_handle().default_window_icon().cloned() {
        let _ = window.set_icon(icon);
    }
    fit_to_primary_monitor(window)?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::{
            Graphics::Dwm::{
                DwmEnableBlurBehindWindow, DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
                DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND,
            },
            Graphics::Gdi::{CreateRectRgn, DeleteObject},
            UI::WindowsAndMessaging::{
                GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
                SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_APPWINDOW,
                WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            },
        };

        let win = window.clone();
        let _ = window.run_on_main_thread(move || {
            let Ok(hwnd_handle) = win.hwnd() else {
                return;
            };
            let hwnd = hwnd_handle.0;
            apply_windows_menu_icon(hwnd);
            unsafe {
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let overlay_flags = (WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TRANSPARENT
                    | WS_EX_LAYERED) as isize;
                let next_ex_style = (ex_style | overlay_flags) & !(WS_EX_APPWINDOW as isize);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_ex_style);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );

                // Use DwmEnableBlurBehindWindow with an empty region so DWM composites
                // per-pixel WebView2 alpha without painting an opaque Windows 10 frame.
                let region = CreateRectRgn(0, 0, -1, -1);
                let bb = DWM_BLURBEHIND {
                    dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                    fEnable: 1,
                    hRgnBlur: region,
                    fTransitionOnMaximized: 0,
                };
                let _ = DwmEnableBlurBehindWindow(hwnd, &bb);
                let _ = DeleteObject(region);

                let disable_transitions: i32 = 1;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                    &disable_transitions as *const i32 as *const core::ffi::c_void,
                    std::mem::size_of::<i32>() as u32,
                );
            }
        });
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = window.set_visible_on_all_workspaces(true);
        let _ = window.set_skip_taskbar(true);
    }

    Ok(())
}

pub fn show_and_focus_menu_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window("menu").is_none() {
        return Err("Menu window not found".to_string());
    }
    let app_handle = app.clone();
    app.run_on_main_thread(move || {
        if let Some(menu) = app_handle.get_webview_window("menu") {
            let _ = menu.unminimize();
            let _ = menu.show();
            let _ = menu.set_always_on_top(true);

            #[cfg(target_os = "windows")]
            if let Ok(hwnd) = menu.hwnd() {
                apply_windows_menu_icon(hwnd.0);
                hide_tauri_drag_resize_borders(hwnd.0);
                force_windows_foreground(hwnd.0);
            }

            let _ = menu.set_focus();
            let _ = app_handle.emit("luair://menu-visibility", true);
        }
    })
    .map_err(|e| e.to_string())
}

pub fn toggle_menu_window(app: &AppHandle) -> Result<bool, String> {
    let menu = app
        .get_webview_window("menu")
        .ok_or_else(|| "Menu window not found".to_string())?;

    let is_visible = menu.is_visible().map_err(|e| e.to_string())?;
    let is_minimized = menu.is_minimized().unwrap_or(false);
    let should_show = !is_visible || is_minimized;

    if should_show {
        show_and_focus_menu_window(app)?;
        Ok(true)
    } else {
        hide_menu_window(app)?;
        Ok(false)
    }
}

pub fn hide_menu_window(app: &AppHandle) -> Result<(), String> {
    let app_handle = app.clone();
    app.run_on_main_thread(move || {
        if let Some(menu) = app_handle.get_webview_window("menu") {
            let _ = menu.hide();
            let _ = app_handle.emit("luair://menu-visibility", false);
        }
        if let Some(overlay) = app_handle.get_webview_window("overlay") {
            let _ = overlay.set_always_on_top(true);
        }
    })
    .map_err(|e| e.to_string())
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
