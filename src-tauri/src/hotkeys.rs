use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, RwLock,
};
use std::thread;
use std::time::{Duration, Instant};

use rdev::{listen, Button, Event, EventType, Key};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{overlay, schema::LuairState, state::AppState, storage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputTrigger {
    Key(Key),
    Mouse(Button),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBinding {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    pub trigger: InputTrigger,
}

impl InputBinding {
    pub fn has_modifiers(&self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModifierState {
    pub ctrl_left: bool,
    pub ctrl_right: bool,
    pub shift_left: bool,
    pub shift_right: bool,
    pub alt_left: bool,
    pub alt_right: bool,
    pub meta_left: bool,
    pub meta_right: bool,
}

impl ModifierState {
    pub fn ctrl(&self) -> bool {
        self.ctrl_left || self.ctrl_right
    }

    pub fn shift(&self) -> bool {
        self.shift_left || self.shift_right
    }

    pub fn alt(&self) -> bool {
        self.alt_left || self.alt_right
    }

    pub fn meta(&self) -> bool {
        self.meta_left || self.meta_right
    }

    pub fn update(&mut self, key: Key, pressed: bool) -> bool {
        match key {
            Key::ControlLeft => {
                self.ctrl_left = pressed;
                true
            }
            Key::ControlRight => {
                self.ctrl_right = pressed;
                true
            }
            Key::ShiftLeft => {
                self.shift_left = pressed;
                true
            }
            Key::ShiftRight => {
                self.shift_right = pressed;
                true
            }
            Key::Alt => {
                self.alt_left = pressed;
                true
            }
            Key::AltGr => {
                self.alt_right = pressed;
                true
            }
            Key::MetaLeft => {
                self.meta_left = pressed;
                true
            }
            Key::MetaRight => {
                self.meta_right = pressed;
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HotkeyTable {
    pub menu_toggle: Option<InputBinding>,
    pub ads_toggle: Option<InputBinding>,
    pub enable_profile_hotkeys: bool,
    pub live_recoil_tracking: bool,
    pub profiles: Vec<(String, InputBinding)>,
}

impl HotkeyTable {
    pub fn from_state(state: &LuairState) -> Self {
        let menu_toggle = parse_hotkey(&state.ui.hotkey);
        let ads_toggle = state.ui.ads_hotkey.as_deref().and_then(parse_hotkey);
        let profiles = state
            .profiles
            .iter()
            .filter_map(|p| {
                let spec = p.hotkey.as_deref()?;
                let binding = parse_hotkey(spec)?;
                Some((p.id.clone(), binding))
            })
            .collect();

        Self {
            menu_toggle,
            ads_toggle,
            enable_profile_hotkeys: state.ui.enable_profile_hotkeys,
            live_recoil_tracking: state.crosshair.dynamic_spread
                || state.crosshair.recoil_color_enabled,
            profiles,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleMenu,
    ToggleAds,
    SwitchProfile(String),
}

fn parse_mouse_token(token: &str) -> Option<Button> {
    let upper = token.trim().to_ascii_uppercase();
    match upper.as_str() {
        "MOUSELEFT" | "MOUSE1" | "LMB" => Some(Button::Left),
        "MOUSERIGHT" | "MOUSE2" | "RMB" => Some(Button::Right),
        "MOUSEMIDDLE" | "MOUSE3" | "MMB" => Some(Button::Middle),
        "MOUSE4" | "XBUTTON1" | "MOUSEBACK" => Some(Button::Unknown(1)),
        "MOUSE5" | "XBUTTON2" | "MOUSEFORWARD" => Some(Button::Unknown(2)),
        "MOUSE6" => Some(Button::Unknown(3)),
        "MOUSE7" => Some(Button::Unknown(4)),
        "MOUSE8" => Some(Button::Unknown(5)),
        "MOUSE9" => Some(Button::Unknown(6)),
        _ => None,
    }
}

fn parse_key_token(token: &str) -> Option<Key> {
    let upper = token.trim().to_ascii_uppercase();
    match upper.as_str() {
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "0" | "DIGIT0" | "NUM0" => Some(Key::Num0),
        "1" | "DIGIT1" | "NUM1" => Some(Key::Num1),
        "2" | "DIGIT2" | "NUM2" => Some(Key::Num2),
        "3" | "DIGIT3" | "NUM3" => Some(Key::Num3),
        "4" | "DIGIT4" | "NUM4" => Some(Key::Num4),
        "5" | "DIGIT5" | "NUM5" => Some(Key::Num5),
        "6" | "DIGIT6" | "NUM6" => Some(Key::Num6),
        "7" | "DIGIT7" | "NUM7" => Some(Key::Num7),
        "8" | "DIGIT8" | "NUM8" => Some(Key::Num8),
        "9" | "DIGIT9" | "NUM9" => Some(Key::Num9),
        "KP0" | "NUMPAD0" => Some(Key::Kp0),
        "KP1" | "NUMPAD1" => Some(Key::Kp1),
        "KP2" | "NUMPAD2" => Some(Key::Kp2),
        "KP3" | "NUMPAD3" => Some(Key::Kp3),
        "KP4" | "NUMPAD4" => Some(Key::Kp4),
        "KP5" | "NUMPAD5" => Some(Key::Kp5),
        "KP6" | "NUMPAD6" => Some(Key::Kp6),
        "KP7" | "NUMPAD7" => Some(Key::Kp7),
        "KP8" | "NUMPAD8" => Some(Key::Kp8),
        "KP9" | "NUMPAD9" => Some(Key::Kp9),
        "A" | "KEYA" => Some(Key::KeyA),
        "B" | "KEYB" => Some(Key::KeyB),
        "C" | "KEYC" => Some(Key::KeyC),
        "D" | "KEYD" => Some(Key::KeyD),
        "E" | "KEYE" => Some(Key::KeyE),
        "F" | "KEYF" => Some(Key::KeyF),
        "G" | "KEYG" => Some(Key::KeyG),
        "H" | "KEYH" => Some(Key::KeyH),
        "I" | "KEYI" => Some(Key::KeyI),
        "J" | "KEYJ" => Some(Key::KeyJ),
        "K" | "KEYK" => Some(Key::KeyK),
        "L" | "KEYL" => Some(Key::KeyL),
        "M" | "KEYM" => Some(Key::KeyM),
        "N" | "KEYN" => Some(Key::KeyN),
        "O" | "KEYO" => Some(Key::KeyO),
        "P" | "KEYP" => Some(Key::KeyP),
        "Q" | "KEYQ" => Some(Key::KeyQ),
        "R" | "KEYR" => Some(Key::KeyR),
        "S" | "KEYS" => Some(Key::KeyS),
        "T" | "KEYT" => Some(Key::KeyT),
        "U" | "KEYU" => Some(Key::KeyU),
        "V" | "KEYV" => Some(Key::KeyV),
        "W" | "KEYW" => Some(Key::KeyW),
        "X" | "KEYX" => Some(Key::KeyX),
        "Y" | "KEYY" => Some(Key::KeyY),
        "Z" | "KEYZ" => Some(Key::KeyZ),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "INSERT" | "INS" => Some(Key::Insert),
        "DELETE" | "DEL" => Some(Key::Delete),
        "PAGEUP" | "PGUP" => Some(Key::PageUp),
        "PAGEDOWN" | "PGDN" => Some(Key::PageDown),
        "ESCAPE" | "ESC" => Some(Key::Escape),
        "TAB" => Some(Key::Tab),
        "SPACE" | "SPACEBAR" => Some(Key::Space),
        "CAPSLOCK" => Some(Key::CapsLock),
        "`" | "~" | "BACKQUOTE" | "TILDE" => Some(Key::BackQuote),
        "-" | "MINUS" => Some(Key::Minus),
        "=" | "EQUAL" => Some(Key::Equal),
        "[" | "LEFTBRACKET" => Some(Key::LeftBracket),
        "]" | "RIGHTBRACKET" => Some(Key::RightBracket),
        ";" | "SEMICOLON" => Some(Key::SemiColon),
        "'" | "QUOTE" => Some(Key::Quote),
        "\\" | "BACKSLASH" => Some(Key::BackSlash),
        "," | "COMMA" => Some(Key::Comma),
        "." | "DOT" | "PERIOD" => Some(Key::Dot),
        "/" | "SLASH" => Some(Key::Slash),
        "ARROWUP" | "UP" => Some(Key::UpArrow),
        "ARROWDOWN" | "DOWN" => Some(Key::DownArrow),
        "ARROWLEFT" | "LEFT" => Some(Key::LeftArrow),
        "ARROWRIGHT" | "RIGHT" => Some(Key::RightArrow),
        "ENTER" | "RETURN" => Some(Key::Return),
        "BACKSPACE" => Some(Key::Backspace),
        _ => None,
    }
}

pub fn format_mouse_binding(mods: &ModifierState, button: Button) -> String {
    let mut parts: Vec<String> = Vec::new();
    if mods.ctrl() {
        parts.push("Ctrl".to_string());
    }
    if mods.shift() {
        parts.push("Shift".to_string());
    }
    if mods.alt() {
        parts.push("Alt".to_string());
    }
    if mods.meta() {
        parts.push("Meta".to_string());
    }
    let btn_name = match button {
        Button::Left => "MouseLeft".to_string(),
        Button::Right => "MouseRight".to_string(),
        Button::Middle => "MouseMiddle".to_string(),
        Button::Unknown(1) => "Mouse4".to_string(),
        Button::Unknown(2) => "Mouse5".to_string(),
        Button::Unknown(code) => format!("Mouse{}", code.saturating_add(3)),
    };
    parts.push(btn_name);
    parts.join("+")
}

pub fn parse_hotkey(spec: &str) -> Option<InputBinding> {
    let trimmed = spec.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("None")
        || trimmed.eq_ignore_ascii_case("BIND")
    {
        return None;
    }

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut meta = false;
    let mut trigger: Option<InputTrigger> = None;

    for part in trimmed.split('+') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        match token.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => ctrl = true,
            "SHIFT" => shift = true,
            "ALT" => alt = true,
            "META" | "WIN" | "SUPER" | "CMD" | "COMMAND" => meta = true,
            _ => {
                if trigger.is_some() {
                    return None;
                }
                if let Some(btn) = parse_mouse_token(token) {
                    trigger = Some(InputTrigger::Mouse(btn));
                } else {
                    trigger = parse_key_token(token).map(InputTrigger::Key);
                }
                trigger?;
            }
        }
    }

    trigger.map(|t| InputBinding {
        ctrl,
        shift,
        alt,
        meta,
        trigger: t,
    })
}

fn exact_modifiers_match(binding: &InputBinding, mods: &ModifierState) -> bool {
    binding.ctrl == mods.ctrl()
        && binding.shift == mods.shift()
        && binding.alt == mods.alt()
        && binding.meta == mods.meta()
}

pub fn match_action(
    table: &HotkeyTable,
    mods: &ModifierState,
    trigger: InputTrigger,
    menu_focused: bool,
) -> Option<HotkeyAction> {
    // 1. Menu toggle always takes precedence and works even when the menu is focused.
    if let Some(menu_bind) = &table.menu_toggle {
        if menu_bind.trigger == trigger && exact_modifiers_match(menu_bind, mods) {
            return Some(HotkeyAction::ToggleMenu);
        }
    }

    // 2. ADS toggle (skip bare trigger when the menu has focus so typing/clicking isn't interrupted).
    if let Some(ads_bind) = &table.ads_toggle {
        if ads_bind.trigger == trigger
            && exact_modifiers_match(ads_bind, mods)
            && (!menu_focused || ads_bind.has_modifiers())
        {
            return Some(HotkeyAction::ToggleAds);
        }
    }

    if !table.enable_profile_hotkeys {
        return None;
    }

    // 3. Modified profile binds take precedence over bare profile binds.
    for (id, bind) in &table.profiles {
        if bind.has_modifiers() && bind.trigger == trigger && exact_modifiers_match(bind, mods) {
            return Some(HotkeyAction::SwitchProfile(id.clone()));
        }
    }

    // 4. Bare profile binds (only when menu is unfocused; permits Shift/Ctrl held for FPS movement).
    if !menu_focused && !mods.alt() && !mods.meta() {
        for (id, bind) in &table.profiles {
            if !bind.has_modifiers() && bind.trigger == trigger {
                return Some(HotkeyAction::SwitchProfile(id.clone()));
            }
        }
    }

    None
}

fn dispatch_action(
    app: &AppHandle,
    action: HotkeyAction,
    last_menu_toggle: &mut Instant,
) {
    match action {
        HotkeyAction::ToggleMenu => {
            let now = Instant::now();
            if now.duration_since(*last_menu_toggle) >= Duration::from_millis(200) {
                *last_menu_toggle = now;
                let _ = overlay::toggle_menu_window(app);
            }
        }
        HotkeyAction::ToggleAds => {
            let app_state = app.state::<AppState>();
            let next_state = {
                let mut guard = app_state.state.lock().expect("AppState mutex poisoned");
                guard.crosshair.hide_crosshair = !guard.crosshair.hide_crosshair;
                guard.clone()
            };
            let _ = app.emit("luair://state-updated", &next_state);
            let _ = storage::save_to_disk(app, &next_state);
        }
        HotkeyAction::SwitchProfile(profile_id) => {
            let app_state = app.state::<AppState>();
            let updated = {
                let mut guard = app_state.state.lock().expect("AppState mutex poisoned");
                if let Some(profile) = guard.profiles.iter().find(|p| p.id == profile_id).cloned()
                {
                    guard.crosshair = profile.config;
                    guard.active_profile_id = Some(profile.id);
                    Some(guard.clone())
                } else {
                    None
                }
            };

            if let Some(next_state) = updated {
                app_state.sync_hotkeys(&next_state);
                let _ = app.emit("luair://state-updated", &next_state);
                let _ = app.emit("luair://profile-switched", &profile_id);
                let _ = storage::save_to_disk(app, &next_state);
            }
        }
    }
}

pub fn spawn_global_listener(
    app: AppHandle,
    table: Arc<RwLock<HotkeyTable>>,
    recording: Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel::<EventType>();

    let _ = thread::Builder::new()
        .name("luair-rdev-hook".to_string())
        .spawn(move || {
            let callback = move |event: Event| {
                if matches!(
                    event.event_type,
                    EventType::KeyPress(_)
                        | EventType::KeyRelease(_)
                        | EventType::ButtonPress(_)
                        | EventType::ButtonRelease(_)
                ) {
                    let _ = tx.send(event.event_type);
                }
            };
            if let Err(err) = listen(callback) {
                eprintln!("Luair global input hook terminated: {err:?}");
            }
        });

    let _ = thread::Builder::new()
        .name("luair-hotkey-worker".to_string())
        .spawn(move || {
            let mut mods = ModifierState::default();
            let mut pressed_keys: HashSet<Key> = HashSet::new();
            let mut pressed_buttons: HashSet<Button> = HashSet::new();
            let mut last_menu_toggle = Instant::now() - Duration::from_millis(500);
            let mut firing_emitted = false;

            while let Ok(event_type) = rx.recv() {
                match event_type {
                    EventType::KeyRelease(key) => {
                        mods.update(key, false);
                        pressed_keys.remove(&key);
                    }
                    EventType::KeyPress(key) => {
                        if mods.update(key, true) {
                            continue;
                        }
                        if !pressed_keys.insert(key) {
                            continue;
                        }
                        if recording.load(Ordering::Relaxed) {
                            continue;
                        }

                        let menu_focused = app
                            .get_webview_window("menu")
                            .and_then(|w| w.is_focused().ok())
                            .unwrap_or(false);

                        let action = table.read().ok().and_then(|guard| {
                            match_action(&guard, &mods, InputTrigger::Key(key), menu_focused)
                        });

                        if let Some(action) = action {
                            dispatch_action(&app, action, &mut last_menu_toggle);
                        }
                    }
                    EventType::ButtonRelease(btn) => {
                        pressed_buttons.remove(&btn);
                        if btn == Button::Left && firing_emitted {
                            firing_emitted = false;
                            let _ = app.emit("luair://fire-state", false);
                        }
                    }
                    EventType::ButtonPress(btn) => {
                        if recording.load(Ordering::Relaxed) {
                            if btn != Button::Left {
                                let formatted = format_mouse_binding(&mods, btn);
                                let _ = app.emit("luair://mouse-captured", formatted);
                            }
                            continue;
                        }

                        if !pressed_buttons.insert(btn) {
                            continue;
                        }

                        let menu_focused = app
                            .get_webview_window("menu")
                            .and_then(|w| w.is_focused().ok())
                            .unwrap_or(false);

                        let (live_recoil, action) = table
                            .read()
                            .ok()
                            .map(|guard| {
                                (
                                    guard.live_recoil_tracking,
                                    match_action(
                                        &guard,
                                        &mods,
                                        InputTrigger::Mouse(btn),
                                        menu_focused,
                                    ),
                                )
                            })
                            .unwrap_or((false, None));

                        if btn == Button::Left && !menu_focused && live_recoil {
                            firing_emitted = true;
                            let _ = app.emit("luair://fire-state", true);
                        }

                        if let Some(action) = action {
                            dispatch_action(&app, action, &mut last_menu_toggle);
                        }
                    }
                    _ => {}
                }
            }
        });
}

#[tauri::command]
pub fn set_hotkey_recording(app_state: State<'_, AppState>, recording: bool) {
    app_state
        .recording_hotkey
        .store(recording, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::LuairState;

    #[test]
    fn parses_single_keys_and_modifier_combos() {
        assert_eq!(
            parse_hotkey("1"),
            Some(InputBinding {
                ctrl: false,
                shift: false,
                alt: false,
                meta: false,
                trigger: InputTrigger::Key(Key::Num1),
            })
        );
        assert_eq!(
            parse_hotkey("F10"),
            Some(InputBinding {
                ctrl: false,
                shift: false,
                alt: false,
                meta: false,
                trigger: InputTrigger::Key(Key::F10),
            })
        );
        assert_eq!(
            parse_hotkey("Ctrl+Shift+X"),
            Some(InputBinding {
                ctrl: true,
                shift: true,
                alt: false,
                meta: false,
                trigger: InputTrigger::Key(Key::KeyX),
            })
        );
        assert_eq!(
            parse_hotkey("Alt+X"),
            Some(InputBinding {
                ctrl: false,
                shift: false,
                alt: true,
                meta: false,
                trigger: InputTrigger::Key(Key::KeyX),
            })
        );
        assert_eq!(parse_hotkey("None"), None);
    }

    #[test]
    fn parses_and_formats_mouse_buttons() {
        assert_eq!(
            parse_hotkey("Mouse4"),
            Some(InputBinding {
                ctrl: false,
                shift: false,
                alt: false,
                meta: false,
                trigger: InputTrigger::Mouse(Button::Unknown(1)),
            })
        );
        assert_eq!(
            parse_hotkey("XButton2"),
            Some(InputBinding {
                ctrl: false,
                shift: false,
                alt: false,
                meta: false,
                trigger: InputTrigger::Mouse(Button::Unknown(2)),
            })
        );
        assert_eq!(
            parse_hotkey("Ctrl+MouseMiddle"),
            Some(InputBinding {
                ctrl: true,
                shift: false,
                alt: false,
                meta: false,
                trigger: InputTrigger::Mouse(Button::Middle),
            })
        );
        assert_eq!(
            parse_hotkey("Alt+MouseRight"),
            Some(InputBinding {
                ctrl: false,
                shift: false,
                alt: true,
                meta: false,
                trigger: InputTrigger::Mouse(Button::Right),
            })
        );

        let mut mods = ModifierState::default();
        assert_eq!(format_mouse_binding(&mods, Button::Unknown(1)), "Mouse4");
        assert_eq!(format_mouse_binding(&mods, Button::Unknown(2)), "Mouse5");
        mods.ctrl_left = true;
        assert_eq!(
            format_mouse_binding(&mods, Button::Middle),
            "Ctrl+MouseMiddle"
        );
    }

    #[test]
    fn matches_ctrl_shift_combo_with_exact_modifiers() {
        let mut state = LuairState::default();
        state.ui.hotkey = "Ctrl+Shift+X".to_string();
        let table = HotkeyTable::from_state(&state);

        let mut mods = ModifierState::default();
        assert_eq!(
            match_action(&table, &mods, InputTrigger::Key(Key::KeyX), false),
            None
        );

        mods.ctrl_left = true;
        mods.shift_left = true;
        assert_eq!(
            match_action(&table, &mods, InputTrigger::Key(Key::KeyX), true),
            Some(HotkeyAction::ToggleMenu)
        );

        mods.alt_left = true;
        assert_eq!(
            match_action(&table, &mods, InputTrigger::Key(Key::KeyX), false),
            None
        );
    }

    #[test]
    fn bare_weapon_key_and_mouse_button_switch_slots_pass_through() {
        let state = LuairState::default();
        let table = HotkeyTable::from_state(&state);

        let bare_mods = ModifierState::default();
        assert_eq!(
            match_action(&table, &bare_mods, InputTrigger::Key(Key::Num1), false),
            Some(HotkeyAction::SwitchProfile("slot-1".to_string()))
        );
        assert_eq!(
            match_action(
                &table,
                &bare_mods,
                InputTrigger::Mouse(Button::Unknown(1)),
                false
            ),
            Some(HotkeyAction::SwitchProfile("slot-4".to_string()))
        );

        let mut sprint_mods = ModifierState::default();
        sprint_mods.shift_left = true;
        assert_eq!(
            match_action(
                &table,
                &sprint_mods,
                InputTrigger::Mouse(Button::Unknown(1)),
                false
            ),
            Some(HotkeyAction::SwitchProfile("slot-4".to_string()))
        );

        let mut alt_mods = ModifierState::default();
        alt_mods.alt_left = true;
        assert_eq!(
            match_action(
                &table,
                &alt_mods,
                InputTrigger::Mouse(Button::Unknown(1)),
                false
            ),
            None
        );

        // Suppressed when menu is focused; menu toggle F10 still works.
        assert_eq!(
            match_action(
                &table,
                &bare_mods,
                InputTrigger::Mouse(Button::Unknown(1)),
                true
            ),
            None
        );
        assert_eq!(
            match_action(&table, &bare_mods, InputTrigger::Key(Key::F10), true),
            Some(HotkeyAction::ToggleMenu)
        );
    }

    #[test]
    fn modified_profile_bind_takes_priority_over_bare_bind() {
        let mut state = LuairState::default();
        state.profiles[1].hotkey = Some("Ctrl+1".to_string());
        state.profiles[2].hotkey = Some("Ctrl+Mouse4".to_string());
        let table = HotkeyTable::from_state(&state);

        let mut ctrl_mods = ModifierState::default();
        ctrl_mods.ctrl_left = true;
        assert_eq!(
            match_action(&table, &ctrl_mods, InputTrigger::Key(Key::Num1), false),
            Some(HotkeyAction::SwitchProfile("slot-2".to_string()))
        );
        assert_eq!(
            match_action(
                &table,
                &ctrl_mods,
                InputTrigger::Mouse(Button::Unknown(1)),
                false
            ),
            Some(HotkeyAction::SwitchProfile("slot-3".to_string()))
        );
    }
}
