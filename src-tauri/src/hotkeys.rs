use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc, Arc, Mutex, RwLock,
};
use std::thread;
use std::time::{Duration, Instant};

use rdev::{listen, Button, Event, EventType, Key};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{overlay, schema::LuairState, state::AppState, storage};

const STALE_KEY_TIMEOUT: Duration = Duration::from_millis(1200);
const MENU_TOGGLE_DEBOUNCE: Duration = Duration::from_millis(250);

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

pub fn key_to_win_vk(key: Key) -> Option<u32> {
    match key {
        Key::F1 => Some(0x70),
        Key::F2 => Some(0x71),
        Key::F3 => Some(0x72),
        Key::F4 => Some(0x73),
        Key::F5 => Some(0x74),
        Key::F6 => Some(0x75),
        Key::F7 => Some(0x76),
        Key::F8 => Some(0x77),
        Key::F9 => Some(0x78),
        Key::F10 => Some(0x79),
        Key::F11 => Some(0x7A),
        Key::F12 => Some(0x7B),
        Key::Num0 => Some(0x30),
        Key::Num1 => Some(0x31),
        Key::Num2 => Some(0x32),
        Key::Num3 => Some(0x33),
        Key::Num4 => Some(0x34),
        Key::Num5 => Some(0x35),
        Key::Num6 => Some(0x36),
        Key::Num7 => Some(0x37),
        Key::Num8 => Some(0x38),
        Key::Num9 => Some(0x39),
        Key::Kp0 => Some(0x60),
        Key::Kp1 => Some(0x61),
        Key::Kp2 => Some(0x62),
        Key::Kp3 => Some(0x63),
        Key::Kp4 => Some(0x64),
        Key::Kp5 => Some(0x65),
        Key::Kp6 => Some(0x66),
        Key::Kp7 => Some(0x67),
        Key::Kp8 => Some(0x68),
        Key::Kp9 => Some(0x69),
        Key::KeyA => Some(0x41),
        Key::KeyB => Some(0x42),
        Key::KeyC => Some(0x43),
        Key::KeyD => Some(0x44),
        Key::KeyE => Some(0x45),
        Key::KeyF => Some(0x46),
        Key::KeyG => Some(0x47),
        Key::KeyH => Some(0x48),
        Key::KeyI => Some(0x49),
        Key::KeyJ => Some(0x4A),
        Key::KeyK => Some(0x4B),
        Key::KeyL => Some(0x4C),
        Key::KeyM => Some(0x4D),
        Key::KeyN => Some(0x4E),
        Key::KeyO => Some(0x4F),
        Key::KeyP => Some(0x50),
        Key::KeyQ => Some(0x51),
        Key::KeyR => Some(0x52),
        Key::KeyS => Some(0x53),
        Key::KeyT => Some(0x54),
        Key::KeyU => Some(0x55),
        Key::KeyV => Some(0x56),
        Key::KeyW => Some(0x57),
        Key::KeyX => Some(0x58),
        Key::KeyY => Some(0x59),
        Key::KeyZ => Some(0x5A),
        Key::Home => Some(0x24),
        Key::End => Some(0x23),
        Key::Insert => Some(0x2D),
        Key::Delete => Some(0x2E),
        Key::PageUp => Some(0x21),
        Key::PageDown => Some(0x22),
        Key::Escape => Some(0x1B),
        Key::Tab => Some(0x09),
        Key::Space => Some(0x20),
        Key::CapsLock => Some(0x14),
        Key::BackQuote => Some(0xC0),
        Key::Minus => Some(0xBD),
        Key::Equal => Some(0xBB),
        Key::LeftBracket => Some(0xDB),
        Key::RightBracket => Some(0xDD),
        Key::SemiColon => Some(0xBA),
        Key::Quote => Some(0xDE),
        Key::BackSlash => Some(0xDC),
        Key::Comma => Some(0xBC),
        Key::Dot => Some(0xBE),
        Key::Slash => Some(0xBF),
        Key::UpArrow => Some(0x26),
        Key::DownArrow => Some(0x28),
        Key::LeftArrow => Some(0x25),
        Key::RightArrow => Some(0x27),
        Key::Return => Some(0x0D),
        Key::Backspace => Some(0x08),
        _ => None,
    }
}

pub fn should_accept_key_press(
    pressed_keys: &mut HashMap<Key, Instant>,
    key: Key,
    now: Instant,
) -> bool {
    match pressed_keys.get(&key).copied() {
        Some(prev) if now.duration_since(prev) < STALE_KEY_TIMEOUT => {
            pressed_keys.insert(key, now);
            false
        }
        _ => {
            pressed_keys.insert(key, now);
            true
        }
    }
}

pub fn notify_menu_hotkey_changed(win_hotkey_tid: &AtomicU32) {
    #[cfg(target_os = "windows")]
    {
        let tid = win_hotkey_tid.load(Ordering::Acquire);
        if tid != 0 {
            use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};
            unsafe {
                let _ = PostThreadMessageW(tid, WM_APP + 1, 0, 0);
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = win_hotkey_tid;
}

#[cfg(target_os = "windows")]
fn sync_windows_modifiers(mods: &mut ModifierState) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    let is_down = |vk: i32| -> bool { unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 } };
    mods.ctrl_left = is_down(0xA2);
    mods.ctrl_right = is_down(0xA3);
    mods.shift_left = is_down(0xA0);
    mods.shift_right = is_down(0xA1);
    mods.alt_left = is_down(0xA4);
    mods.alt_right = is_down(0xA5);
    mods.meta_left = is_down(0x5B);
    mods.meta_right = is_down(0x5C);
}

#[cfg(target_os = "windows")]
fn sync_registered_menu_hotkey(
    table: &Arc<RwLock<HotkeyTable>>,
    recording: &Arc<AtomicBool>,
    active_registered: &mut bool,
) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    };
    const MENU_HOTKEY_ID: i32 = 1;

    if *active_registered {
        unsafe {
            let _ = UnregisterHotKey(std::ptr::null_mut(), MENU_HOTKEY_ID);
        }
        *active_registered = false;
    }

    if recording.load(Ordering::Relaxed) {
        return;
    }

    let menu_bind = table.read().ok().and_then(|guard| guard.menu_toggle.clone());
    if let Some(bind) = menu_bind {
        if let InputTrigger::Key(key) = bind.trigger {
            if let Some(vk) = key_to_win_vk(key) {
                let mut fs_mods = MOD_NOREPEAT;
                if bind.ctrl {
                    fs_mods |= MOD_CONTROL;
                }
                if bind.shift {
                    fs_mods |= MOD_SHIFT;
                }
                if bind.alt {
                    fs_mods |= MOD_ALT;
                }
                if bind.meta {
                    fs_mods |= MOD_WIN;
                }
                let ok =
                    unsafe { RegisterHotKey(std::ptr::null_mut(), MENU_HOTKEY_ID, fs_mods, vk) };
                if ok != 0 {
                    *active_registered = true;
                }
            }
        }
    }
}

fn try_toggle_menu(app: &AppHandle, last_menu_toggle: &Arc<Mutex<Instant>>) -> bool {
    let now = Instant::now();
    let allowed = if let Ok(mut guard) = last_menu_toggle.lock() {
        if now.duration_since(*guard) >= MENU_TOGGLE_DEBOUNCE {
            *guard = now;
            true
        } else {
            false
        }
    } else {
        false
    };

    if allowed {
        let _ = overlay::toggle_menu_window(app);
    }
    allowed
}

fn dispatch_action(
    app: &AppHandle,
    action: HotkeyAction,
    last_menu_toggle: &Arc<Mutex<Instant>>,
) {
    match action {
        HotkeyAction::ToggleMenu => {
            let _ = try_toggle_menu(app, last_menu_toggle);
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
    win_hotkey_tid: Arc<AtomicU32>,
) {
    let last_menu_toggle = Arc::new(Mutex::new(
        Instant::now() - Duration::from_millis(500),
    ));

    #[cfg(target_os = "windows")]
    {
        let win_app = app.clone();
        let win_table = Arc::clone(&table);
        let win_recording = Arc::clone(&recording);
        let win_last_toggle = Arc::clone(&last_menu_toggle);
        let win_tid = Arc::clone(&win_hotkey_tid);
        let _ = thread::Builder::new()
            .name("luair-win-hotkey".to_string())
            .spawn(move || {
                use windows_sys::Win32::System::Threading::GetCurrentThreadId;
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    GetMessageW, PeekMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY,
                };

                const MENU_HOTKEY_ID: usize = 1;
                const WM_SYNC_HOTKEY: u32 = WM_APP + 1;

                let mut msg: MSG = unsafe { std::mem::zeroed() };
                unsafe {
                    let _ = PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                }
                let tid = unsafe { GetCurrentThreadId() };
                win_tid.store(tid, Ordering::Release);

                let mut active_registered = false;
                sync_registered_menu_hotkey(&win_table, &win_recording, &mut active_registered);

                while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                    if msg.message == WM_HOTKEY && msg.wParam == MENU_HOTKEY_ID {
                        if !win_recording.load(Ordering::Relaxed) {
                            let _ = try_toggle_menu(&win_app, &win_last_toggle);
                        }
                    } else if msg.message == WM_SYNC_HOTKEY {
                        sync_registered_menu_hotkey(
                            &win_table,
                            &win_recording,
                            &mut active_registered,
                        );
                    }
                }
            });
    }

    #[cfg(not(target_os = "windows"))]
    let _ = win_hotkey_tid;

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
            let mut pressed_keys: HashMap<Key, Instant> = HashMap::new();
            let mut pressed_buttons: HashSet<Button> = HashSet::new();
            let mut firing_emitted = false;

            while let Ok(event_type) = rx.recv() {
                match event_type {
                    EventType::KeyRelease(key) => {
                        mods.update(key, false);
                        pressed_keys.remove(&key);
                    }
                    EventType::KeyPress(key) => {
                        #[cfg(target_os = "windows")]
                        sync_windows_modifiers(&mut mods);

                        if mods.update(key, true) {
                            continue;
                        }
                        if !should_accept_key_press(&mut pressed_keys, key, Instant::now()) {
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
                            dispatch_action(&app, action, &last_menu_toggle);
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
                        #[cfg(target_os = "windows")]
                        sync_windows_modifiers(&mut mods);

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
                            dispatch_action(&app, action, &last_menu_toggle);
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
    notify_menu_hotkey_changed(&app_state.win_hotkey_tid);
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

    #[test]
    fn maps_common_keys_including_f10_to_win_vk() {
        assert_eq!(key_to_win_vk(Key::F10), Some(0x79));
        assert_eq!(key_to_win_vk(Key::Home), Some(0x24));
        assert_eq!(key_to_win_vk(Key::KeyX), Some(0x58));
        assert_eq!(key_to_win_vk(Key::Num1), Some(0x31));
    }

    #[test]
    fn stale_pressed_key_recovers_after_timeout() {
        let mut pressed = HashMap::new();
        let t0 = Instant::now();
        assert!(should_accept_key_press(&mut pressed, Key::F10, t0));
        assert!(!should_accept_key_press(
            &mut pressed,
            Key::F10,
            t0 + Duration::from_millis(200)
        ));
        assert!(should_accept_key_press(
            &mut pressed,
            Key::F10,
            t0 + Duration::from_millis(1450)
        ));
    }
}
