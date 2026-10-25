use crate::{
    controller::{Controller, KeyboardOptions, LightingState},
    keys,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MacroAction {
    Down,
    Up,
}

#[derive(Deserialize)]
pub struct MacroEvent {
    key: String,
    action: MacroAction,
    delay: u16,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    #[default]
    None,
    Ctrl,
    Shift,
    Alt,
    Meta,
}

#[derive(Deserialize)]
#[serde(tag = "path", content = "body")]
pub enum WriteRequest {
    #[serde(rename = "/api/rgb")]
    Lighting(LightingState),
    #[serde(rename = "/api/profile")]
    Profile { profile: u8 },
    #[serde(rename = "/api/rate")]
    Rate { hz: u16, profile: u8 },
    #[serde(rename = "/api/debounce")]
    Debounce { milliseconds: u8 },
    #[serde(rename = "/api/options")]
    Options {
        profile: u8,
        #[serde(flatten)]
        options: KeyboardOptions,
    },
    #[serde(rename = "/api/sleep")]
    Sleep {
        bluetooth: u16,
        wireless: u16,
        bluetooth_deep: u16,
        wireless_deep: u16,
    },
    #[serde(rename = "/api/clock")]
    Clock {},
    #[serde(rename = "/api/remap")]
    Remap {
        source: String,
        target: String,
        #[serde(default)]
        modifier: Modifier,
        #[serde(default)]
        second_usage: u8,
        profile: u8,
        fn_layer: Option<u8>,
    },
    #[serde(rename = "/api/key-color")]
    KeyColor {
        key: String,
        color: String,
        slot: u8,
    },
    #[serde(rename = "/api/live-screen")]
    Screen { rgba: [u8; 4] },
    #[serde(rename = "/api/live-audio")]
    Audio { levels: [u8; 32] },
    #[serde(rename = "/api/macro")]
    Macro {
        source: String,
        events: Vec<MacroEvent>,
        index: u8,
        repeat_count: u16,
        mode: String,
        profile: u8,
    },
    #[serde(rename = "/api/reset")]
    Reset {},
}

impl WriteRequest {
    pub fn parse(path: &str, body: Option<Value>) -> Result<Self, String> {
        let request: Self = serde_json::from_value(serde_json::json!({
            "path": path, "body": body.unwrap_or_else(|| serde_json::json!({}))
        }))
        .map_err(|error| format!("Invalid request: {error}"))?;
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), String> {
        let profile = match self {
            Self::Profile { profile }
            | Self::Rate { profile, .. }
            | Self::Options { profile, .. }
            | Self::Remap { profile, .. }
            | Self::Macro { profile, .. } => Some(*profile),
            _ => None,
        };
        if profile.is_some_and(|value| value > 2) {
            return Err("Profile must be 0..2".into());
        }
        match self {
            Self::Lighting(value) => value.validate()?,
            Self::Rate { hz, .. } if ![125, 250, 500, 1000].contains(hz) => {
                return Err("Report rate must be 125, 250, 500 or 1000".into())
            }
            Self::Debounce { milliseconds } if *milliseconds > 10 => {
                return Err("Debounce must be 0..10 ms".into())
            }
            Self::Sleep {
                bluetooth,
                wireless,
                bluetooth_deep,
                wireless_deep,
            } => {
                if *bluetooth > 60
                    || *wireless > 60
                    || !(10..=60).contains(bluetooth_deep)
                    || !(10..=60).contains(wireless_deep)
                {
                    return Err(
                        "Sleep must be 0..60 minutes; deep sleep must be 10..60 minutes".into(),
                    );
                }
            }
            Self::Remap {
                source, fn_layer, ..
            } => {
                keys::matrix_index(keys::parse_key(source)?)?;
                if !matches!(fn_layer, None | Some(0)) {
                    return Err("K8 has one Fn layer".into());
                }
            }
            Self::KeyColor { key, slot, .. } => {
                keys::matrix_index(keys::parse_key(key)?)?;
                if *slot > 3 {
                    return Err("Lighting slot must be 0..3".into());
                }
            }
            Self::Audio { levels } if levels.iter().any(|level| *level > 6) => {
                return Err("Audio levels must be 0..6".into())
            }
            Self::Macro {
                source,
                index,
                mode,
                ..
            } => {
                keys::matrix_index(keys::parse_key(source)?)?;
                if *index >= 50 {
                    return Err("Macro slot must be 0..49".into());
                }
                if !["repeat", "toggle", "while_held"].contains(&mode.as_str()) {
                    return Err("Invalid macro mode".into());
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn execute(self, controller: &Controller) -> Result<(), String> {
        match self {
            Self::Lighting(value) => controller.set_lighting(&value),
            Self::Profile { profile } => controller.set_profile(profile),
            Self::Rate { hz, profile } => {
                controller.set_report_rate(hz, profile)?;
                let actual = controller.get_report_rate(profile)?;
                if actual != hz {
                    return Err(format!(
                        "Keyboard reported {actual} Hz after requesting {hz} Hz"
                    ));
                }
                Ok(())
            }
            Self::Debounce { milliseconds } => controller.set_debounce(milliseconds),
            Self::Options {
                profile,
                mut options,
            } => {
                // This flag has no editor. Preserve it instead of clearing it on every save.
                options.keyboard_lock = controller.get_options(profile)?.keyboard_lock;
                controller.set_options(&options, profile)
            }
            Self::Sleep {
                bluetooth,
                wireless,
                bluetooth_deep,
                wireless_deep,
            } => controller.set_sleep([
                bluetooth * 60,
                wireless * 60,
                bluetooth_deep * 60,
                wireless_deep * 60,
            ]),
            Self::Clock {} => controller.set_clock(),
            Self::Remap {
                source,
                target,
                modifier,
                second_usage,
                profile,
                fn_layer,
            } => {
                let modifier = match modifier {
                    Modifier::None => 0,
                    Modifier::Ctrl => 224,
                    Modifier::Shift => 225,
                    Modifier::Alt => 226,
                    Modifier::Meta => 227,
                };
                controller.set_key_payload(
                    keys::parse_key(&source)?,
                    keys::parse_target(&target, modifier, second_usage)?,
                    profile,
                    fn_layer,
                )
            }
            Self::KeyColor { key, color, slot } => {
                controller.set_key_color(keys::parse_key(&key)?, &color, slot)
            }
            Self::Screen { rgba } => controller.screen_color(&rgba),
            Self::Audio { levels } => controller.music(&levels),
            Self::Macro {
                source,
                events,
                index,
                repeat_count,
                mode,
                profile,
            } => {
                let encoded = events
                    .into_iter()
                    .map(|event| {
                        Ok((
                            keys::parse_key(&event.key)?,
                            matches!(event.action, MacroAction::Down),
                            event.delay,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                controller.macro_assign(
                    keys::parse_key(&source)?,
                    &encoded,
                    index,
                    repeat_count,
                    &mode,
                    profile,
                )
            }
            Self::Reset {} => controller.reset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_wrapping_numbers_and_invalid_sleep() {
        for (path, body) in [
            ("/api/profile", json!({"profile":256})),
            ("/api/debounce", json!({"milliseconds":256})),
            ("/api/rate", json!({"hz":66536,"profile":0})),
            (
                "/api/sleep",
                json!({"bluetooth":1093,"wireless":1,"bluetooth_deep":10,"wireless_deep":10}),
            ),
            (
                "/api/macro",
                json!({"source":"F12","events":[],"index":256,"repeat_count":1,"mode":"repeat","profile":0}),
            ),
        ] {
            assert!(WriteRequest::parse(path, Some(body)).is_err(), "{path}");
        }
    }

    #[test]
    fn rejects_unknown_macro_actions() {
        assert!(WriteRequest::parse("/api/macro", Some(json!({"source":"F12","events":[{"key":"A","action":"typo","delay":30}],"index":0,"repeat_count":1,"mode":"repeat","profile":0}))).is_err());
    }
}
