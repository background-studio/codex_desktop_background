use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    #[default]
    Cover,
    Contain,
    Fill,
    Tile,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettings {
    pub fit: FitMode,
    pub position_x: f64,
    pub position_y: f64,
    pub opacity: f64,
    pub blur: f64,
    pub scale: f64,
    pub overlay_color: String,
    pub overlay_opacity: f64,
    pub home_intensity: f64,
    pub task_intensity: f64,
    pub sidebar_opacity: f64,
    pub surface_opacity: f64,
    pub composer_opacity: f64,
    pub menu_opacity: f64,
    pub terminal_opacity: f64,
    pub enabled_on_home: bool,
    pub enabled_on_tasks: bool,
    pub video_muted: bool,
    pub video_playback_rate: f64,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            fit: FitMode::Cover,
            position_x: 50.0,
            position_y: 50.0,
            opacity: 0.72,
            blur: 0.0,
            scale: 1.0,
            overlay_color: "#101416".to_string(),
            overlay_opacity: 0.12,
            home_intensity: 1.0,
            task_intensity: 0.32,
            sidebar_opacity: 0.78,
            surface_opacity: 0.82,
            composer_opacity: 0.88,
            menu_opacity: 0.9,
            terminal_opacity: 0.9,
            enabled_on_home: true,
            enabled_on_tasks: true,
            video_muted: true,
            video_playback_rate: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub phase: String,
    pub message: String,
    pub active_targets: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            phase: "waiting".to_string(),
            message: crate::managed_launch::MSG_UNCONFIGURED.to_string(),
            active_targets: 0,
            codex_version: None,
            last_error: None,
        }
    }
}

const DISPLAY_KEYS: &[&str] = &[
    "fit",
    "positionX",
    "positionY",
    "opacity",
    "blur",
    "scale",
    "overlayColor",
    "overlayOpacity",
    "homeIntensity",
    "taskIntensity",
    "sidebarOpacity",
    "surfaceOpacity",
    "composerOpacity",
    "menuOpacity",
    "terminalOpacity",
    "enabledOnHome",
    "enabledOnTasks",
    "videoMuted",
    "videoPlaybackRate",
];

fn number_field(map: &serde_json::Map<String, Value>, key: &str) -> Result<Option<f64>, String> {
    let Some(value) = map.get(key) else {
        return Ok(None);
    };
    let number = value
        .as_f64()
        .or_else(|| value.as_i64().map(|value| value as f64))
        .or_else(|| value.as_u64().map(|value| value as f64))
        .ok_or_else(|| format!("display.{key} 必须是数字。"))?;
    if !number.is_finite() {
        return Err(format!("display.{key} 不是有效数字。"));
    }
    Ok(Some(number))
}

fn require_range(value: f64, minimum: f64, maximum: f64, key: &str) -> Result<f64, String> {
    if value < minimum || value > maximum {
        return Err(format!("display.{key} 超出范围 {minimum}..={maximum}。"));
    }
    Ok(value)
}

fn boolean_field(map: &serde_json::Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    let Some(value) = map.get(key) else {
        return Ok(None);
    };
    value
        .as_bool()
        .ok_or_else(|| format!("display.{key} 必须是布尔值。"))
        .map(Some)
}

impl DisplaySettings {
    pub fn from_configure(value: &Value) -> Result<Self, String> {
        let map = value
            .as_object()
            .ok_or_else(|| "display 必须是对象。".to_string())?;
        for key in map.keys() {
            if !DISPLAY_KEYS.contains(&key.as_str()) {
                return Err(format!("不支持的 display 字段：{key}。"));
            }
        }

        let defaults = Self::default();
        let fit = match map.get("fit") {
            None => defaults.fit,
            Some(value) => match value.as_str() {
                Some("cover") => FitMode::Cover,
                Some("contain") => FitMode::Contain,
                Some("fill") => FitMode::Fill,
                Some("tile") => FitMode::Tile,
                _ => return Err("display.fit 必须是 cover、contain、fill 或 tile。".to_string()),
            },
        };
        let overlay_color = match map.get("overlayColor") {
            None => defaults.overlay_color,
            Some(value) => {
                let color = value
                    .as_str()
                    .ok_or_else(|| "display.overlayColor 必须是字符串。".to_string())?;
                if color.len() != 7
                    || !color.starts_with('#')
                    || !color[1..]
                        .chars()
                        .all(|character| character.is_ascii_hexdigit())
                {
                    return Err("display.overlayColor 必须是 #RRGGBB。".to_string());
                }
                color.to_ascii_lowercase()
            }
        };

        Ok(Self {
            fit,
            position_x: number_field(map, "positionX")?
                .map(|value| require_range(value, 0.0, 100.0, "positionX"))
                .transpose()?
                .unwrap_or(defaults.position_x),
            position_y: number_field(map, "positionY")?
                .map(|value| require_range(value, 0.0, 100.0, "positionY"))
                .transpose()?
                .unwrap_or(defaults.position_y),
            opacity: number_field(map, "opacity")?
                .map(|value| require_range(value, 0.0, 1.0, "opacity"))
                .transpose()?
                .unwrap_or(defaults.opacity),
            blur: number_field(map, "blur")?
                .map(|value| require_range(value, 0.0, 40.0, "blur"))
                .transpose()?
                .unwrap_or(defaults.blur),
            scale: number_field(map, "scale")?
                .map(|value| require_range(value, 1.0, 1.3, "scale"))
                .transpose()?
                .unwrap_or(defaults.scale),
            overlay_color,
            overlay_opacity: number_field(map, "overlayOpacity")?
                .map(|value| require_range(value, 0.0, 0.9, "overlayOpacity"))
                .transpose()?
                .unwrap_or(defaults.overlay_opacity),
            home_intensity: number_field(map, "homeIntensity")?
                .map(|value| require_range(value, 0.0, 1.0, "homeIntensity"))
                .transpose()?
                .unwrap_or(defaults.home_intensity),
            task_intensity: number_field(map, "taskIntensity")?
                .map(|value| require_range(value, 0.0, 1.0, "taskIntensity"))
                .transpose()?
                .unwrap_or(defaults.task_intensity),
            sidebar_opacity: number_field(map, "sidebarOpacity")?
                .map(|value| require_range(value, 0.0, 1.0, "sidebarOpacity"))
                .transpose()?
                .unwrap_or(defaults.sidebar_opacity),
            surface_opacity: number_field(map, "surfaceOpacity")?
                .map(|value| require_range(value, 0.0, 1.0, "surfaceOpacity"))
                .transpose()?
                .unwrap_or(defaults.surface_opacity),
            composer_opacity: number_field(map, "composerOpacity")?
                .map(|value| require_range(value, 0.0, 1.0, "composerOpacity"))
                .transpose()?
                .unwrap_or(defaults.composer_opacity),
            menu_opacity: number_field(map, "menuOpacity")?
                .map(|value| require_range(value, 0.0, 1.0, "menuOpacity"))
                .transpose()?
                .unwrap_or(defaults.menu_opacity),
            terminal_opacity: number_field(map, "terminalOpacity")?
                .map(|value| require_range(value, 0.0, 1.0, "terminalOpacity"))
                .transpose()?
                .unwrap_or(defaults.terminal_opacity),
            enabled_on_home: boolean_field(map, "enabledOnHome")?
                .unwrap_or(defaults.enabled_on_home),
            enabled_on_tasks: boolean_field(map, "enabledOnTasks")?
                .unwrap_or(defaults.enabled_on_tasks),
            video_muted: boolean_field(map, "videoMuted")?.unwrap_or(defaults.video_muted),
            video_playback_rate: number_field(map, "videoPlaybackRate")?
                .map(|value| require_range(value, 0.25, 2.0, "videoPlaybackRate"))
                .transpose()?
                .unwrap_or(defaults.video_playback_rate),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_unknown_or_out_of_range_display_fields() {
        assert!(DisplaySettings::from_configure(&json!({ "slideshow": true })).is_err());
        assert!(DisplaySettings::from_configure(&json!({ "opacity": 9 })).is_err());
        assert!(DisplaySettings::from_configure(&json!({ "overlayColor": "red" })).is_err());
        assert!(DisplaySettings::from_configure(&json!({ "fit": "stretch" })).is_err());
    }

    #[test]
    fn accepts_partial_display_and_keeps_defaults() {
        let display = DisplaySettings::from_configure(&json!({
            "opacity": 0.4,
            "enabledOnHome": false
        }))
        .unwrap();
        assert_eq!(display.opacity, 0.4);
        assert!(!display.enabled_on_home);
        assert_eq!(display.fit, FitMode::Cover);
        assert_eq!(display.scale, 1.0);
    }
}
