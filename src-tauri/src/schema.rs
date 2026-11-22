use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CrosshairConfig {
    pub color: String,
    pub outline: bool,
    pub outline_color: String,
    pub outline_thickness: u32,
    pub length: u32,
    pub thickness: u32,
    pub gap: u32,
    pub rotation: u32,
    pub dot: bool,
    pub dot_size: u32,
    pub dot_shape: String,
    pub t_shape: bool,
    pub opacity: f32,
    pub outline_opacity: f32,
    pub blur: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub color_mode: String,
    pub gradient_color: String,
    pub gradient_angle: u32,
    pub dynamic_spread: bool,
    pub spread_amount: u32,
    pub spread_speed: u32,
    pub recoil_color_enabled: bool,
    pub recoil_color: String,
    pub animate: bool,
    pub spin_speed: u32,
    pub rainbow: bool,
    pub rainbow_speed: u32,
    pub img_src: Option<String>,
    pub has_circle: bool,
    pub circle_radius: u32,
    pub hide_bars: bool,
    pub hide_crosshair: bool,
    pub show_top: bool,
    pub show_bottom: bool,
    pub show_left: bool,
    pub show_right: bool,
}

impl Default for CrosshairConfig {
    fn default() -> Self {
        Self {
            color: "#06b6d4".to_string(),
            outline: true,
            outline_color: "#000000".to_string(),
            outline_thickness: 1,
            length: 10,
            thickness: 2,
            gap: 5,
            rotation: 0,
            dot: false,
            dot_size: 3,
            dot_shape: "circle".to_string(),
            t_shape: false,
            opacity: 1.0,
            outline_opacity: 1.0,
            blur: 0,
            offset_x: 0,
            offset_y: 0,
            color_mode: "solid".to_string(),
            gradient_color: "#818cf8".to_string(),
            gradient_angle: 45,
            dynamic_spread: false,
            spread_amount: 14,
            spread_speed: 10,
            recoil_color_enabled: false,
            recoil_color: "#ff2a55".to_string(),
            animate: false,
            spin_speed: 2,
            rainbow: false,
            rainbow_speed: 5,
            img_src: None,
            has_circle: false,
            circle_radius: 15,
            hide_bars: false,
            hide_crosshair: false,
            show_top: true,
            show_bottom: true,
            show_left: true,
            show_right: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeConfig {
    pub id: String,
    pub name: String,
    pub surface_mode: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub muted_text: String,
    pub accent: String,
    pub highlight: String,
    pub blur_px: u32,
    pub glass_opacity: f32,
    pub border_refraction: u32,
    pub dynamic_lighting: bool,
    pub corner_radius: u32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            id: "deep-space".to_string(),
            name: "Deep Space".to_string(),
            surface_mode: "liquid-glass".to_string(),
            background: "#070913".to_string(),
            surface: "#101528".to_string(),
            text: "#eef4ff".to_string(),
            muted_text: "#8a99bd".to_string(),
            accent: "#38bdf8".to_string(),
            highlight: "#818cf8".to_string(),
            blur_px: 22,
            glass_opacity: 0.76,
            border_refraction: 78,
            dynamic_lighting: true,
            corner_radius: 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiConfig {
    pub ui_theme: String,
    pub ui_window: String,
    pub ui_button: String,
    pub ui_layout: String,
    pub ui_accent: String,
    pub theme: ThemeConfig,
    pub hotkey: String,
    pub ads_hotkey: Option<String>,
    pub enable_profile_hotkeys: bool,
    pub streamproof: bool,
    pub cloak_menu: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            ui_theme: "Modern Dark".to_string(),
            ui_window: "glass".to_string(),
            ui_button: "modern".to_string(),
            ui_layout: "split".to_string(),
            ui_accent: "#38bdf8".to_string(),
            theme: ThemeConfig::default(),
            hotkey: "F10".to_string(),
            ads_hotkey: Some("Alt+X".to_string()),
            enable_profile_hotkeys: true,
            streamproof: false,
            cloak_menu: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CrosshairProfile {
    pub id: String,
    pub name: String,
    pub hotkey: Option<String>,
    pub config: CrosshairConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LuairState {
    pub crosshair: CrosshairConfig,
    pub ui: UiConfig,
    pub profiles: Vec<CrosshairProfile>,
    pub active_profile_id: Option<String>,
}

pub fn default_profiles() -> Vec<CrosshairProfile> {
    vec![
        CrosshairProfile {
            id: "slot-1".to_string(),
            name: "Primary (Rifle)".to_string(),
            hotkey: Some("1".to_string()),
            config: CrosshairConfig::default(),
        },
        CrosshairProfile {
            id: "slot-2".to_string(),
            name: "Secondary (Precision Dot)".to_string(),
            hotkey: Some("2".to_string()),
            config: CrosshairConfig {
                color: "#22c55e".to_string(),
                hide_bars: true,
                dot: true,
                dot_size: 5,
                dot_shape: "circle".to_string(),
                thickness: 4,
                outline: true,
                ..CrosshairConfig::default()
            },
        },
        CrosshairProfile {
            id: "slot-3".to_string(),
            name: "Close Quarters (Dynamic)".to_string(),
            hotkey: Some("3".to_string()),
            config: CrosshairConfig {
                color: "#f59e0b".to_string(),
                hide_bars: true,
                dot: true,
                dot_size: 3,
                has_circle: true,
                circle_radius: 24,
                thickness: 2,
                outline: true,
                dynamic_spread: true,
                spread_amount: 16,
                recoil_color_enabled: true,
                recoil_color: "#ff2a55".to_string(),
                ..CrosshairConfig::default()
            },
        },
        CrosshairProfile {
            id: "slot-4".to_string(),
            name: "Tactical T-Shape".to_string(),
            hotkey: Some("Mouse4".to_string()),
            config: CrosshairConfig {
                color: "#22d3ee".to_string(),
                color_mode: "gradient".to_string(),
                gradient_color: "#818cf8".to_string(),
                gradient_angle: 45,
                t_shape: true,
                dot: true,
                dot_size: 3,
                length: 12,
                gap: 4,
                ..CrosshairConfig::default()
            },
        },
    ]
}

impl Default for LuairState {
    fn default() -> Self {
        Self {
            crosshair: CrosshairConfig::default(),
            ui: UiConfig::default(),
            profiles: default_profiles(),
            active_profile_id: Some("slot-1".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_crosshair_matches_legacy_and_studio_defaults() {
        let cfg = CrosshairConfig::default();
        assert_eq!(cfg.color, "#06b6d4");
        assert!(cfg.outline);
        assert_eq!(cfg.outline_color, "#000000");
        assert_eq!(cfg.outline_thickness, 1);
        assert_eq!(cfg.length, 10);
        assert_eq!(cfg.thickness, 2);
        assert_eq!(cfg.gap, 5);
        assert_eq!(cfg.rotation, 0);
        assert!(!cfg.dot);
        assert_eq!(cfg.dot_size, 3);
        assert_eq!(cfg.dot_shape, "circle");
        assert!(!cfg.t_shape);
        assert_eq!(cfg.opacity, 1.0);
        assert_eq!(cfg.outline_opacity, 1.0);
        assert_eq!(cfg.blur, 0);
        assert_eq!(cfg.offset_x, 0);
        assert_eq!(cfg.offset_y, 0);
        assert_eq!(cfg.color_mode, "solid");
        assert_eq!(cfg.gradient_color, "#818cf8");
        assert_eq!(cfg.gradient_angle, 45);
        assert!(!cfg.dynamic_spread);
        assert_eq!(cfg.spread_amount, 14);
        assert_eq!(cfg.spread_speed, 10);
        assert!(!cfg.recoil_color_enabled);
        assert_eq!(cfg.recoil_color, "#ff2a55");
        assert!(!cfg.animate);
        assert_eq!(cfg.spin_speed, 2);
        assert!(!cfg.rainbow);
        assert_eq!(cfg.rainbow_speed, 5);
        assert_eq!(cfg.img_src, None);
        assert!(!cfg.has_circle);
        assert_eq!(cfg.circle_radius, 15);
        assert!(!cfg.hide_bars);
        assert!(!cfg.hide_crosshair);
        assert!(cfg.show_top);
        assert!(cfg.show_bottom);
        assert!(cfg.show_left);
        assert!(cfg.show_right);
    }

    #[test]
    fn default_state_includes_starter_profiles_and_phase2_ui_defaults() {
        let state = LuairState::default();
        assert_eq!(state.ui.hotkey, "F10");
        assert_eq!(state.ui.ads_hotkey.as_deref(), Some("Alt+X"));
        assert!(state.ui.enable_profile_hotkeys);
        assert!(!state.ui.streamproof);
        assert!(state.ui.cloak_menu);
        assert_eq!(state.profiles.len(), 4);
        assert_eq!(state.profiles[0].hotkey.as_deref(), Some("1"));
        assert_eq!(state.profiles[1].hotkey.as_deref(), Some("2"));
        assert_eq!(state.profiles[2].hotkey.as_deref(), Some("3"));
        assert_eq!(state.profiles[3].hotkey.as_deref(), Some("Mouse4"));
        assert_eq!(state.active_profile_id.as_deref(), Some("slot-1"));
    }

    #[test]
    fn theme_config_round_trips_camelcase_json() {
        let mut theme = ThemeConfig::default();
        assert_eq!(theme.id, "deep-space");
        assert_eq!(theme.surface_mode, "liquid-glass");
        assert_eq!(theme.blur_px, 22);
        assert!(theme.dynamic_lighting);

        theme.surface_mode = "solid-minimal".to_string();
        theme.accent = "#f59e0b".to_string();
        theme.corner_radius = 12;

        let json = serde_json::to_string(&theme).expect("serialize theme");
        assert!(json.contains("\"surfaceMode\""));
        assert!(json.contains("\"mutedText\""));
        assert!(json.contains("\"glassOpacity\""));
        assert!(json.contains("\"borderRefraction\""));
        assert!(json.contains("\"dynamicLighting\""));
        assert!(json.contains("\"cornerRadius\""));

        let parsed: ThemeConfig = serde_json::from_str(&json).expect("deserialize theme");
        assert_eq!(parsed, theme);
    }

    #[test]
    fn studio_crosshair_fields_round_trip_camelcase_json() {
        let mut cfg = CrosshairConfig::default();
        cfg.dot = true;
        cfg.dot_size = 6;
        cfg.dot_shape = "square".to_string();
        cfg.t_shape = true;
        cfg.opacity = 0.85;
        cfg.outline_opacity = 0.6;
        cfg.blur = 4;
        cfg.offset_x = -12;
        cfg.offset_y = 8;
        cfg.color_mode = "gradient".to_string();
        cfg.gradient_color = "#f43f5e".to_string();
        cfg.gradient_angle = 135;
        cfg.dynamic_spread = true;
        cfg.spread_amount = 22;
        cfg.spread_speed = 18;
        cfg.recoil_color_enabled = true;
        cfg.recoil_color = "#ff0044".to_string();

        let json = serde_json::to_string(&cfg).expect("serialize crosshair");
        assert!(json.contains("\"dotSize\""));
        assert!(json.contains("\"dotShape\""));
        assert!(json.contains("\"tShape\""));
        assert!(json.contains("\"outlineOpacity\""));
        assert!(json.contains("\"offsetX\""));
        assert!(json.contains("\"offsetY\""));
        assert!(json.contains("\"colorMode\""));
        assert!(json.contains("\"gradientColor\""));
        assert!(json.contains("\"gradientAngle\""));
        assert!(json.contains("\"dynamicSpread\""));
        assert!(json.contains("\"spreadAmount\""));
        assert!(json.contains("\"spreadSpeed\""));
        assert!(json.contains("\"recoilColorEnabled\""));
        assert!(json.contains("\"recoilColor\""));

        let parsed: CrosshairConfig =
            serde_json::from_str(&json).expect("deserialize crosshair");
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn state_round_trips_camelcase_json() {
        let mut state = LuairState::default();
        state.crosshair.length = 16;
        state.crosshair.dot = true;
        state.ui.streamproof = true;
        state.ui.hotkey = "Ctrl+Shift+X".to_string();
        state.ui.theme.accent = "#00f0ff".to_string();

        let json = serde_json::to_string(&state).expect("serialize state");
        assert!(json.contains("\"outlineColor\""));
        assert!(json.contains("\"circleRadius\""));
        assert!(json.contains("\"activeProfileId\""));
        assert!(json.contains("\"adsHotkey\""));
        assert!(json.contains("\"enableProfileHotkeys\""));
        assert!(json.contains("\"cloakMenu\""));
        assert!(json.contains("\"surfaceMode\""));

        let parsed: LuairState = serde_json::from_str(&json).expect("deserialize state");
        assert_eq!(parsed, state);
    }

    #[test]
    fn partial_json_falls_back_to_defaults_and_ignores_legacy_fields() {
        let legacy_json = r##"{
            "color": "#ff0054",
            "gap": 8,
            "deprecatedExtraFlag": true,
            "unknownLegacyKey": "ignored"
        }"##;

        let parsed: CrosshairConfig =
            serde_json::from_str(legacy_json).expect("parse partial legacy json");
        assert_eq!(parsed.color, "#ff0054");
        assert_eq!(parsed.gap, 8);
        assert_eq!(parsed.length, 10);
        assert_eq!(parsed.dot_size, 3);
        assert_eq!(parsed.opacity, 1.0);
        assert!(parsed.outline);
        assert!(parsed.show_top);
    }
}
