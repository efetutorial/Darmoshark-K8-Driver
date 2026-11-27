//! UI-independent tables and rules: lighting modes and their controls, the
//! physical K8 layout and remap targets. Kept free of GPUI so it is testable.

/// Lighting modes offered by the vendor K8 layout, in vendor order.
pub const EFFECTS: &[(&str, &str)] = &[
    ("off", "Off"),
    ("solid", "Solid"),
    ("breathing", "Breathing"),
    ("neon", "Neon"),
    ("wave", "Wave"),
    ("ripple", "Ripple"),
    ("raindrop", "Raindrop"),
    ("snake", "Snake"),
    ("reactive", "Reactive"),
    ("convergence", "Convergence"),
    ("sine", "Sine wave"),
    ("kaleidoscope", "Kaleidoscope"),
    ("line_wave", "Line wave"),
    ("user_picture", "Custom keys"),
    ("laser", "Laser"),
    ("circle_wave", "Circle wave"),
    ("dazzling", "Dazzling"),
    ("rain_down", "Rain"),
    ("meteor", "Meteor"),
    ("reactive_off", "Reactive fade"),
    ("music_3", "Audio spectrum"),
    ("screen_color", "Screen color"),
    ("music_2", "Audio bars"),
];

/// Modes the firmware can report (e.g. after Fn shortcuts) that the vendor K8
/// UI does not offer. They are shown only while the keyboard is in them.
pub const DEVICE_ONLY_EFFECTS: &[(&str, &str)] = &[
    ("train", "Train"),
    ("fireworks", "Fireworks"),
    ("user_color", "User color"),
];

pub fn is_listed_effect(effect: &str) -> bool {
    EFFECTS.iter().any(|(id, _)| *id == effect)
}

pub fn effect_label(effect: &str) -> String {
    EFFECTS
        .iter()
        .chain(DEVICE_ONLY_EFFECTS)
        .find(|(id, _)| *id == effect)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| format!("Unsupported mode ({effect})"))
}

/// The modes to show in the selector: the vendor list, plus the keyboard's
/// current mode when it is not part of it, so the UI never shows a stale mode.
pub fn effect_options(current: &str) -> Vec<(String, String)> {
    let mut options: Vec<_> = EFFECTS
        .iter()
        .map(|(id, label)| ((*id).to_string(), (*label).to_string()))
        .collect();
    if !current.is_empty() && !is_listed_effect(current) {
        let label = if DEVICE_ONLY_EFFECTS.iter().any(|(id, _)| *id == current) {
            format!("{} (set on keyboard)", effect_label(current))
        } else {
            effect_label(current)
        };
        options.push((current.to_string(), label));
    }
    options
}

/// Which controls a mode uses. Direction values are the protocol nibble names
/// (right/left/down/up) paired with mode-specific labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capabilities {
    pub color: bool,
    pub rainbow: bool,
    pub brightness: bool,
    pub speed: bool,
    pub slot: bool,
    pub directions: Vec<(&'static str, &'static str)>,
    pub note: Option<&'static str>,
}

fn directions(labels: &[&'static str]) -> Vec<(&'static str, &'static str)> {
    ["right", "left", "down", "up"]
        .into_iter()
        .zip(labels.iter().copied())
        .collect()
}

pub fn capabilities(effect: &str) -> Capabilities {
    let animated = Capabilities {
        color: true,
        rainbow: true,
        brightness: true,
        speed: true,
        ..Default::default()
    };
    let audio = Capabilities {
        color: true,
        rainbow: true,
        brightness: true,
        directions: directions(&["Upright", "Separate", "Intersect"]),
        note: Some("Start audio sync below to drive this mode from your speakers."),
        ..Default::default()
    };
    match effect {
        "off" | "user_color" => Capabilities::default(),
        "solid" => Capabilities {
            color: true,
            rainbow: true,
            brightness: true,
            ..Default::default()
        },
        "neon" => Capabilities {
            brightness: true,
            speed: true,
            ..Default::default()
        },
        "user_picture" => Capabilities {
            brightness: true,
            slot: true,
            ..Default::default()
        },
        "screen_color" => Capabilities {
            note: Some("Start screen sync below to mirror your display."),
            ..Default::default()
        },
        "wave" => Capabilities {
            directions: directions(&["Right", "Left", "Down", "Up"]),
            ..animated
        },
        "snake" => Capabilities {
            directions: directions(&["Zigzag", "Return"]),
            ..animated
        },
        "kaleidoscope" => Capabilities {
            directions: directions(&["Outward", "Inward"]),
            ..animated
        },
        "line_wave" | "fireworks" => Capabilities {
            directions: directions(&["Right", "Left"]),
            ..animated
        },
        "circle_wave" => Capabilities {
            directions: directions(&["Counterclockwise", "Clockwise"]),
            ..animated
        },
        "music_2" | "music_3" => audio,
        _ => animated,
    }
}

/// Swatches offered next to the color picker.
pub const SWATCHES: &[&str] = &[
    "#ff2d55", "#ff9500", "#ffd60a", "#30d158", "#00c7be", "#0a84ff", "#bf5af2", "#ffffff",
];

/// One keycap in the on-screen K8. `key` is `None` for caps that cannot be
/// remapped (Fn, LCD, logo key); `gap` entries are spacing only.
#[derive(Clone, Copy, Debug)]
pub struct Cap {
    pub key: Option<&'static str>,
    pub label: &'static str,
    pub width: f32,
    pub gap: bool,
}

const fn k(key: &'static str, label: &'static str, width: f32) -> Cap {
    Cap {
        key: Some(key),
        label,
        width,
        gap: false,
    }
}
const fn fixed(label: &'static str, width: f32) -> Cap {
    Cap {
        key: None,
        label,
        width,
        gap: false,
    }
}
const fn gap(width: f32) -> Cap {
    Cap {
        key: None,
        label: "",
        width,
        gap: true,
    }
}

/// The physical K8 layout (matches the vendor product image and key matrix).
pub const LAYOUT: &[&[Cap]] = &[
    &[
        k("Esc", "Esc", 1.0),
        gap(0.55),
        k("F1", "F1", 1.0),
        k("F2", "F2", 1.0),
        k("F3", "F3", 1.0),
        k("F4", "F4", 1.0),
        gap(0.25),
        k("F5", "F5", 1.0),
        k("F6", "F6", 1.0),
        k("F7", "F7", 1.0),
        k("F8", "F8", 1.0),
        gap(0.25),
        k("F9", "F9", 1.0),
        k("F10", "F10", 1.0),
        k("F11", "F11", 1.0),
        k("F12", "F12", 1.0),
        k("Delete", "Del", 1.0),
        fixed("LCD", 1.0),
    ],
    &[
        k("Grave", "`", 1.0),
        k("1", "1", 1.0),
        k("2", "2", 1.0),
        k("3", "3", 1.0),
        k("4", "4", 1.0),
        k("5", "5", 1.0),
        k("6", "6", 1.0),
        k("7", "7", 1.0),
        k("8", "8", 1.0),
        k("9", "9", 1.0),
        k("0", "0", 1.0),
        k("Minus", "-", 1.0),
        k("Equal", "=", 1.0),
        k("Backspace", "Back", 2.0),
        k("Home", "Home", 1.0),
    ],
    &[
        k("Tab", "Tab", 1.5),
        k("Q", "Q", 1.0),
        k("W", "W", 1.0),
        k("E", "E", 1.0),
        k("R", "R", 1.0),
        k("T", "T", 1.0),
        k("Y", "Y", 1.0),
        k("U", "U", 1.0),
        k("I", "I", 1.0),
        k("O", "O", 1.0),
        k("P", "P", 1.0),
        k("LeftBracket", "[", 1.0),
        k("RightBracket", "]", 1.0),
        k("Backslash", "\\", 1.5),
        k("PageUp", "PgUp", 1.0),
    ],
    &[
        k("CapsLock", "Caps", 1.8),
        k("A", "A", 1.0),
        k("S", "S", 1.0),
        k("D", "D", 1.0),
        k("F", "F", 1.0),
        k("G", "G", 1.0),
        k("H", "H", 1.0),
        k("J", "J", 1.0),
        k("K", "K", 1.0),
        k("L", "L", 1.0),
        k("Semicolon", ";", 1.0),
        k("Quote", "'", 1.0),
        k("Enter", "Enter", 2.2),
        k("PageDown", "PgDn", 1.0),
    ],
    &[
        k("LeftShift", "Shift", 1.4),
        k("NonUSBackslash", "<", 1.0),
        k("Z", "Z", 1.0),
        k("X", "X", 1.0),
        k("C", "C", 1.0),
        k("V", "V", 1.0),
        k("B", "B", 1.0),
        k("N", "N", 1.0),
        k("M", "M", 1.0),
        k("Comma", ",", 1.0),
        k("Period", ".", 1.0),
        k("Slash", "/", 1.0),
        k("RightShift", "Shift", 1.5),
        k("Up", "↑", 1.0),
        fixed("◆", 1.0),
    ],
    &[
        k("LeftCtrl", "Ctrl", 1.3),
        k("LeftMeta", "Meta", 1.2),
        k("LeftAlt", "Alt", 1.2),
        k("Space", "", 5.8),
        k("RightAlt", "Alt", 1.2),
        fixed("Fn", 1.1),
        k("RightCtrl", "Ctrl", 1.1),
        k("Left", "←", 1.0),
        k("Down", "↓", 1.0),
        k("Right", "→", 1.0),
    ],
];

pub fn key_label(key: &str) -> &str {
    LAYOUT
        .iter()
        .flat_map(|row| row.iter())
        .find(|cap| cap.key == Some(key))
        .map(|cap| if cap.label.is_empty() { key } else { cap.label })
        .unwrap_or(key)
}

/// Human-readable names for the special remap targets.
pub fn target_label(target: &str) -> String {
    let label = match target {
        "disabled" => "Disabled",
        "media:previous" => "Previous track",
        "media:next" => "Next track",
        "media:stop" => "Stop",
        "media:play-pause" => "Play / pause",
        "media:player" => "Media player",
        "media:mute" => "Mute",
        "media:volume-down" => "Volume down",
        "media:volume-up" => "Volume up",
        "system:calculator" => "Calculator",
        "system:mail" => "Mail",
        "system:computer" => "My computer",
        "system:search" => "Search",
        "system:home" => "Browser home",
        "system:brightness-down" => "Screen brightness down",
        "system:brightness-up" => "Screen brightness up",
        "system:refresh" => "Refresh",
        "system:lock" => "Lock screen",
        "system:back" => "Browser back",
        "system:mic-toggle" => "Microphone on / off",
        "system:zoom-in" => "Zoom in",
        "system:zoom-out" => "Zoom out",
        "keyboard:fn" => "Fn",
        "keyboard:fn-lock" => "Fn lock",
        "profile:next" => "Next profile",
        "profile:previous" => "Previous profile",
        "profile:cycle" => "Cycle profiles",
        "profile:1" => "Profile 1",
        "profile:2" => "Profile 2",
        "profile:3" => "Profile 3",
        "mouse:left" => "Left click",
        "mouse:right" => "Right click",
        "mouse:middle" => "Middle click",
        "mouse:forward" => "Mouse forward",
        "mouse:back" => "Mouse back",
        "mouse:wheel-left" => "Wheel left",
        "mouse:wheel-right" => "Wheel right",
        "mouse:wheel-forward" => "Wheel forward",
        "mouse:wheel-back" => "Wheel back",
        "mouse:scroll-up" => "Scroll up",
        "mouse:scroll-down" => "Scroll down",
        other => return key_label(other).to_string(),
    };
    label.to_string()
}

pub fn target_group(target: &str) -> &'static str {
    match target.split_once(':').map(|(group, _)| group) {
        Some("media") => "Media",
        Some("system") => "System",
        Some("keyboard") | Some("profile") => "Keyboard",
        Some("mouse") => "Mouse",
        _ if target == "disabled" => "General",
        _ => "Keys",
    }
}

/// Map a GPUI keystroke key name (layout independent where possible) to the
/// HID key name used by `keys::keys()`. Returns `None` for keys the K8 macro
/// engine cannot send.
pub fn hid_name_for_key(key: &str) -> Option<String> {
    let lower = key.to_ascii_lowercase();
    let mapped = match lower.as_str() {
        "escape" => "Esc",
        "enter" => "Enter",
        "backspace" => "Backspace",
        "tab" => "Tab",
        "space" => "Space",
        "-" => "Minus",
        "=" => "Equal",
        "[" => "LeftBracket",
        "]" => "RightBracket",
        "\\" => "Backslash",
        ";" => "Semicolon",
        "'" => "Quote",
        "`" => "Grave",
        "," => "Comma",
        "." => "Period",
        "/" => "Slash",
        "capslock" => "CapsLock",
        "insert" => "Insert",
        "home" => "Home",
        "pageup" => "PageUp",
        "delete" => "Delete",
        "end" => "End",
        "pagedown" => "PageDown",
        "right" => "Right",
        // Shifted symbols on a US layout map back to their physical keys.
        "!" => "1",
        "@" => "2",
        "#" => "3",
        "$" => "4",
        "%" => "5",
        "^" => "6",
        "&" => "7",
        "*" => "8",
        "(" => "9",
        ")" => "0",
        "_" => "Minus",
        "+" => "Equal",
        "{" => "LeftBracket",
        "}" => "RightBracket",
        "|" => "Backslash",
        ":" => "Semicolon",
        "\"" => "Quote",
        "~" => "Grave",
        "<" => "Comma",
        ">" => "Period",
        "?" => "Slash",
        "left" => "Left",
        "down" => "Down",
        "up" => "Up",
        _ => "",
    };
    if !mapped.is_empty() {
        return Some(mapped.into());
    }
    if let Some(number) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        if (1..=12).contains(&number) {
            return Some(format!("F{number}"));
        }
    }
    let mut chars = lower.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_ascii_uppercase().to_string()),
        _ => None,
    }
}

/// HID key name for a usage, as used in macro events.
pub fn usage_name(usage: u8) -> Option<String> {
    crate::keys::keys()
        .into_iter()
        .find(|(_, value)| *value == usage)
        .map(|(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;

    #[test]
    fn usages_round_trip_to_names() {
        assert_eq!(usage_name(4).as_deref(), Some("A"));
        assert_eq!(usage_name(225).as_deref(), Some("LeftShift"));
        assert_eq!(hid_name_for_key("!").as_deref(), Some("1"));
        assert!(usage_name(3).is_none());
    }

    #[test]
    fn unknown_mode_is_listed_without_hiding_vendor_modes() {
        let options = effect_options("unknown_42");
        assert_eq!(options.len(), EFFECTS.len() + 1);
        assert_eq!(options.last().unwrap().1, "Unsupported mode (unknown_42)");
        assert_eq!(effect_options("wave").len(), EFFECTS.len());
        assert_eq!(
            effect_options("train").last().unwrap().1,
            "Train (set on keyboard)"
        );
    }

    #[test]
    fn capabilities_match_vendor_layout() {
        assert!(!capabilities("neon").color);
        assert!(capabilities("solid").color && !capabilities("solid").speed);
        assert_eq!(capabilities("wave").directions.len(), 4);
        assert_eq!(capabilities("snake").directions[1], ("left", "Return"));
        assert!(capabilities("user_picture").slot);
        assert!(!capabilities("music_3").speed);
        assert_eq!(capabilities("fireworks").directions.len(), 2);
    }

    #[test]
    fn every_layout_key_is_in_the_k8_matrix() {
        for cap in LAYOUT.iter().flat_map(|row| row.iter()) {
            if let Some(key) = cap.key {
                let usage = keys::parse_key(key).unwrap();
                assert!(keys::matrix_index(usage).is_ok(), "{key}");
            }
        }
    }

    #[test]
    fn every_target_has_a_label() {
        for target in keys::target_names() {
            assert_ne!(target_label(&target), target, "{target}");
        }
    }

    #[test]
    fn keystroke_names_map_to_hid_names() {
        assert_eq!(hid_name_for_key("a").as_deref(), Some("A"));
        assert_eq!(hid_name_for_key("f12").as_deref(), Some("F12"));
        assert_eq!(hid_name_for_key("escape").as_deref(), Some("Esc"));
        assert_eq!(hid_name_for_key("ş"), None);
        for name in ["a", "1", "space", "f5", "pageup", "/"] {
            let hid = hid_name_for_key(name).unwrap();
            assert!(keys::keys().contains_key(&hid), "{hid}");
        }
    }
}
