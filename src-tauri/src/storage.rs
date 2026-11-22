use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{cloak, schema::LuairState, state::AppState};

const CONFIG_FILENAME: &str = "luair-config.json";

fn config_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(CONFIG_FILENAME))
}

pub fn load_from_disk(app: &AppHandle) -> LuairState {
    let Ok(path) = config_file_path(app) else {
        return LuairState::default();
    };
    if !path.exists() {
        return LuairState::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<LuairState>(&raw).unwrap_or_default(),
        Err(_) => LuairState::default(),
    }
}

pub fn save_to_disk(app: &AppHandle, state: &LuairState) -> Result<(), String> {
    let path = config_file_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let serialized = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(&path, serialized).map_err(|e| e.to_string())?;

    if let Some(app_state) = app.try_state::<AppState>() {
        let _ = app_state.persist_active_account_state(state);
    }

    Ok(())
}

#[tauri::command]
pub fn get_state(app_state: State<'_, AppState>) -> Result<LuairState, String> {
    Ok(app_state.snapshot())
}

#[tauri::command]
pub fn update_state(
    app: AppHandle,
    app_state: State<'_, AppState>,
    state: LuairState,
) -> Result<LuairState, String> {
    let (prev_streamproof, prev_cloak_menu) = {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        let prev_sp = guard.ui.streamproof;
        let prev_cm = guard.ui.cloak_menu;
        *guard = state.clone();
        (prev_sp, prev_cm)
    };

    app_state.sync_hotkeys(&state);

    if state.ui.streamproof != prev_streamproof || state.ui.cloak_menu != prev_cloak_menu {
        let _ = cloak::apply_cloak_to_app(&app, state.ui.streamproof, state.ui.cloak_menu);
    }

    save_to_disk(&app, &state)?;
    let _ = app.emit("luair://state-updated", &state);
    Ok(state)
}

#[tauri::command]
pub fn select_profile(
    app: AppHandle,
    app_state: State<'_, AppState>,
    profile_id: String,
) -> Result<LuairState, String> {
    let next_state = {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        let profile = guard
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .cloned()
            .ok_or_else(|| format!("Profile '{profile_id}' not found"))?;
        guard.crosshair = profile.config;
        guard.active_profile_id = Some(profile.id);
        guard.clone()
    };

    app_state.sync_hotkeys(&next_state);
    save_to_disk(&app, &next_state)?;
    let _ = app.emit("luair://state-updated", &next_state);
    Ok(next_state)
}
