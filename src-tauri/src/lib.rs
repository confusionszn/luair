mod ai;
mod cloak;
mod hotkeys;
mod overlay;
mod schema;
mod state;
mod storage;
mod updater;
mod vault;

use std::sync::Arc;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WindowEvent,
};

use crate::{ai::AiManager, state::AppState, vault::LocalVault};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            app.manage(updater::PendingUpdateState::default());

            let config_dir = app.path().app_config_dir().ok();
            let vault = match config_dir.as_ref() {
                Some(dir) => LocalVault::open_at_dir(dir).unwrap_or_else(|_| {
                    LocalVault::open_in_memory([0x17u8; 32]).expect("memory vault")
                }),
                None => LocalVault::open_in_memory([0x17u8; 32]).expect("memory vault"),
            };

            let models_dir = config_dir
                .unwrap_or_else(std::env::temp_dir)
                .join("models");
            let ai_manager = Arc::new(AiManager::new(models_dir));

            let (active_account, remember_me, initial_state) =
                match vault.restore_remembered_session() {
                    Some((acct, acct_state)) => (Some(acct), true, acct_state),
                    None => (None, false, storage::load_from_disk(app.handle())),
                };

            let initial_streamproof = initial_state.ui.streamproof;
            let initial_cloak_menu = initial_state.ui.cloak_menu;

            let app_state =
                AppState::new(initial_state, vault, active_account, remember_me, ai_manager);
            let hotkey_table = app_state.hotkey_table.clone();
            let recording_hotkey = app_state.recording_hotkey.clone();
            app.manage(app_state);

            if let Some(overlay_win) = app.get_webview_window("overlay") {
                let _ = overlay::configure_overlay_window(&overlay_win);
            }

            if initial_streamproof {
                let _ = cloak::apply_cloak_to_app(
                    app.handle(),
                    initial_streamproof,
                    initial_cloak_menu,
                );
            }

            hotkeys::spawn_global_listener(app.handle().clone(), hotkey_table, recording_hotkey);

            let toggle_item =
                MenuItem::with_id(app, "toggle_menu", "Toggle Menu", true, None::<&str>)?;
            let cloak_item =
                MenuItem::with_id(app, "toggle_cloak", "Toggle OBS Shield", true, None::<&str>)?;
            let reset_item = MenuItem::with_id(
                app,
                "reset_bounds",
                "Reset Overlay Position",
                true,
                None::<&str>,
            )?;
            let check_updates_item = MenuItem::with_id(
                app,
                "check_updates",
                "Check for Updates",
                true,
                None::<&str>,
            )?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Luair", true, None::<&str>)?;
            let tray_menu = Menu::with_items(
                app,
                &[
                    &toggle_item,
                    &cloak_item,
                    &reset_item,
                    &check_updates_item,
                    &quit_item,
                ],
            )?;

            let mut tray_builder = TrayIconBuilder::new()
                .tooltip("Luair Crosshair Overlay")
                .menu(&tray_menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle_menu" => {
                        let _ = overlay::toggle_menu_window(app);
                    }
                    "toggle_cloak" => {
                        let app_state = app.state::<AppState>();
                        let next_state = {
                            let mut guard = app_state
                                .state
                                .lock()
                                .expect("AppState mutex poisoned");
                            guard.ui.streamproof = !guard.ui.streamproof;
                            guard.clone()
                        };
                        let _ = cloak::apply_cloak_to_app(
                            app,
                            next_state.ui.streamproof,
                            next_state.ui.cloak_menu,
                        );
                        let _ = storage::save_to_disk(app, &next_state);
                        let _ = app.emit("luair://state-updated", &next_state);
                    }
                    "reset_bounds" => {
                        if let Some(overlay_win) = app.get_webview_window("overlay") {
                            let _ = overlay::configure_overlay_window(&overlay_win);
                        }
                    }
                    "check_updates" => {
                        if let Some(menu_win) = app.get_webview_window("menu") {
                            let _ = menu_win.show();
                            let _ = menu_win.unminimize();
                            let _ = menu_win.set_focus();
                        }
                        let _ = app.emit("luair://trigger-update-check", ());
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                });

            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "menu" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            storage::get_state,
            storage::update_state,
            storage::select_profile,
            cloak::set_capture_cloak,
            cloak::get_capture_cloak_status,
            overlay::toggle_menu,
            overlay::hide_menu,
            overlay::minimize_menu,
            overlay::reset_overlay_bounds,
            overlay::quit_app,
            hotkeys::set_hotkey_recording,
            vault::get_auth_session,
            vault::sign_up_local,
            vault::sign_in_local,
            vault::sign_out_local,
            vault::complete_onboarding,
            vault::reopen_onboarding,
            ai::get_ai_status,
            ai::download_ai_model,
            ai::cancel_ai_download,
            ai::load_ai_model,
            ai::unload_ai_model,
            ai::delete_ai_model,
            ai::ai_make_crosshair,
            ai::ai_make_theme,
            ai::ai_chat,
            updater::check_for_updates,
            updater::download_and_install_update,
            updater::get_platform_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Luair overlay");
}
