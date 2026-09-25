use std::collections::BTreeMap;

pub fn keys() -> BTreeMap<String, u8> {
    let mut result = BTreeMap::new();
    for (index, letter) in ('A'..='Z').enumerate() {
        result.insert(letter.to_string(), 4 + index as u8);
    }
    for number in 1..=9 {
        result.insert(number.to_string(), 29 + number as u8);
    }
    result.insert("0".into(), 39);
    for (name, usage) in [
        ("Enter", 40),
        ("Esc", 41),
        ("Backspace", 42),
        ("Tab", 43),
        ("Space", 44),
        ("Minus", 45),
        ("Equal", 46),
        ("LeftBracket", 47),
        ("RightBracket", 48),
        ("Backslash", 49),
        ("NonUSHash", 50),
        ("Semicolon", 51),
        ("Quote", 52),
        ("Grave", 53),
        ("Comma", 54),
        ("Period", 55),
        ("Slash", 56),
        ("CapsLock", 57),
        ("PrintScreen", 70),
        ("ScrollLock", 71),
        ("Pause", 72),
        ("Insert", 73),
        ("Home", 74),
        ("PageUp", 75),
        ("Delete", 76),
        ("End", 77),
        ("PageDown", 78),
        ("Right", 79),
        ("Left", 80),
        ("Down", 81),
        ("Up", 82),
        ("NumLock", 83),
        ("NonUSBackslash", 100),
        ("LeftCtrl", 224),
        ("LeftShift", 225),
        ("LeftAlt", 226),
        ("LeftMeta", 227),
        ("RightCtrl", 228),
        ("RightShift", 229),
        ("RightAlt", 230),
        ("RightMeta", 231),
    ] {
        result.insert(name.into(), usage);
    }
    for number in 1..=12 {
        result.insert(format!("F{number}"), 57 + number as u8);
    }
    result
}

pub fn parse_key(value: &str) -> Result<u8, String> {
    if let Some(usage) = keys().get(value) {
        return Ok(*usage);
    }
    value
        .parse::<u8>()
        .map_err(|_| format!("Unknown HID key: {value}"))
}

pub fn target_names() -> Vec<String> {
    [
        "media:previous",
        "media:next",
        "media:stop",
        "media:play-pause",
        "media:player",
        "media:mute",
        "media:volume-down",
        "media:volume-up",
        "system:calculator",
        "system:mail",
        "system:computer",
        "system:search",
        "system:home",
        "system:brightness-down",
        "system:brightness-up",
        "system:refresh",
        "system:lock",
        "mouse:left",
        "mouse:right",
        "mouse:middle",
        "mouse:forward",
        "mouse:back",
        "mouse:wheel-left",
        "mouse:wheel-right",
        "mouse:scroll-up",
        "mouse:scroll-down",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn parse_target(value: &str, modifier: u8, second: u8) -> Result<[u8; 4], String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("disabled") {
        return Ok([0; 4]);
    }
    let payload = match normalized.to_ascii_lowercase().as_str() {
        "media:previous" => [3, 0, 182, 0],
        "media:next" => [3, 0, 181, 0],
        "media:stop" => [3, 0, 183, 0],
        "media:play-pause" => [3, 0, 205, 0],
        "media:player" => [3, 0, 131, 1],
        "media:mute" => [3, 0, 226, 0],
        "media:volume-down" => [3, 0, 234, 0],
        "media:volume-up" => [3, 0, 233, 0],
        "system:calculator" => [3, 0, 146, 1],
        "system:mail" => [3, 0, 138, 1],
        "system:computer" => [3, 0, 148, 1],
        "system:search" => [3, 0, 33, 2],
        "system:home" => [3, 0, 35, 2],
        "system:brightness-down" => [3, 0, 112, 0],
        "system:brightness-up" => [3, 0, 111, 0],
        "system:refresh" => [3, 0, 39, 2],
        "system:lock" => [0, 0, 227, 15],
        "mouse:left" => [1, 0, 240, 0],
        "mouse:right" => [1, 0, 241, 0],
        "mouse:middle" => [1, 0, 242, 0],
        "mouse:forward" => [1, 0, 243, 0],
        "mouse:back" => [1, 0, 244, 0],
        "mouse:wheel-left" => [1, 0, 245, 0],
        "mouse:wheel-right" => [1, 0, 246, 0],
        "mouse:scroll-up" => [1, 0, 245, 1],
        "mouse:scroll-down" => [1, 0, 245, 255],
        _ => [0, modifier, parse_key(normalized)?, second],
    };
    Ok(payload)
}

pub fn matrix_index(usage: u8) -> Result<u8, String> {
    const MATRIX: &[(u8, u8)] = &[
        (0, 41),
        (1, 53),
        (2, 43),
        (3, 57),
        (4, 225),
        (5, 224),
        (6, 58),
        (7, 30),
        (8, 20),
        (9, 4),
        (10, 100),
        (12, 59),
        (13, 31),
        (14, 26),
        (15, 22),
        (16, 29),
        (17, 227),
        (18, 60),
        (19, 32),
        (20, 8),
        (21, 7),
        (22, 27),
        (23, 226),
        (24, 61),
        (25, 33),
        (26, 21),
        (27, 9),
        (28, 6),
        (30, 62),
        (31, 34),
        (32, 23),
        (33, 10),
        (34, 25),
        (36, 63),
        (37, 35),
        (38, 28),
        (39, 11),
        (40, 5),
        (41, 44),
        (42, 64),
        (43, 36),
        (44, 24),
        (45, 13),
        (46, 17),
        (48, 65),
        (49, 37),
        (50, 12),
        (51, 14),
        (52, 16),
        (54, 66),
        (55, 38),
        (56, 18),
        (57, 15),
        (58, 54),
        (59, 230),
        (60, 67),
        (61, 39),
        (62, 19),
        (63, 51),
        (64, 55),
        (66, 68),
        (67, 45),
        (68, 47),
        (69, 52),
        (70, 56),
        (71, 228),
        (72, 69),
        (73, 46),
        (74, 48),
        (75, 50),
        (76, 229),
        (77, 80),
        (78, 76),
        (79, 42),
        (80, 49),
        (81, 40),
        (82, 82),
        (83, 81),
        (86, 74),
        (87, 75),
        (88, 78),
        (89, 79),
    ];
    MATRIX
        .iter()
        .find_map(|(index, value)| (*value == usage).then_some(*index))
        .ok_or_else(|| format!("HID usage {usage} is not present on the K8"))
}
