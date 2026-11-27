//! A restrained, neutral palette in the spirit of minimal Chromium browsers:
//! a darker window frame, a slightly lifted content surface, hairline borders,
//! monochrome primary actions and no decorative glow.

use gpui::{rgb, rgba, App, Global, Hsla, WindowAppearance};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub dark: bool,
    /// Window frame and sidebar.
    pub frame: Hsla,
    /// The rounded content surface next to the sidebar.
    pub surface: Hsla,
    /// Grouped sections inside the surface.
    pub card: Hsla,
    /// Controls resting state (inputs, segmented tracks).
    pub control: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub primary: Hsla,
    pub primary_text: Hsla,
    pub danger: Hsla,
    pub success: Hsla,
    pub overlay: Hsla,
    pub keycap: Hsla,
    pub keycap_border: Hsla,
}

impl Global for Theme {}

impl Theme {
    pub fn dark() -> Self {
        Self {
            dark: true,
            frame: rgb(0x151515).into(),
            surface: rgb(0x1d1d1d).into(),
            card: rgb(0x232323).into(),
            control: rgb(0x2a2a2a).into(),
            hover: rgba(0xffffff0f).into(),
            active: rgba(0xffffff1a).into(),
            border: rgba(0xffffff12).into(),
            border_strong: rgba(0xffffff24).into(),
            text: rgb(0xececec).into(),
            muted: rgb(0xa3a3a3).into(),
            faint: rgb(0x6f6f6f).into(),
            primary: rgb(0xececec).into(),
            primary_text: rgb(0x151515).into(),
            danger: rgb(0xff6b6b).into(),
            success: rgb(0x4ade80).into(),
            overlay: rgba(0x00000080).into(),
            keycap: rgb(0x2b2b2b).into(),
            keycap_border: rgba(0xffffff14).into(),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            frame: rgb(0xe8e8e8).into(),
            surface: rgb(0xf8f8f8).into(),
            card: rgb(0xffffff).into(),
            control: rgb(0xf0f0f0).into(),
            hover: rgba(0x0000000b).into(),
            active: rgba(0x00000014).into(),
            border: rgba(0x00000014).into(),
            border_strong: rgba(0x00000026).into(),
            text: rgb(0x171717).into(),
            muted: rgb(0x5c5c5c).into(),
            faint: rgb(0x9b9b9b).into(),
            primary: rgb(0x171717).into(),
            primary_text: rgb(0xffffff).into(),
            danger: rgb(0xd93a3a).into(),
            success: rgb(0x15964b).into(),
            overlay: rgba(0x00000040).into(),
            keycap: rgb(0xffffff).into(),
            keycap_border: rgba(0x0000001c).into(),
        }
    }

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
        }
    }
}

pub fn theme(cx: &App) -> Theme {
    *cx.global::<Theme>()
}

/// Parse `#rrggbb` into a color, if valid.
pub fn hex_color(value: &str) -> Option<Hsla> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok().map(|v| rgb(v).into())
}

/// HSV (h in 0..360, s and v in 0..1) to `#rrggbb`.
pub fn hsv_hex(h: f32, s: f32, v: f32) -> String {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let byte = |value: f32| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

/// `#rrggbb` to HSV.
pub fn hex_hsv(value: &str) -> Option<(f32, f32, f32)> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    let (r, g, b) = (
        ((v >> 16) & 255) as f32 / 255.0,
        ((v >> 8) & 255) as f32 / 255.0,
        (v & 255) as f32 / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    Some((h, s, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_round_trips() {
        for hex in ["#ff0000", "#00ff80", "#123456", "#ffffff", "#000000"] {
            let (h, s, v) = hex_hsv(hex).unwrap();
            assert_eq!(hsv_hex(h, s, v), hex);
        }
        assert!(hex_color("#12345").is_none());
    }
}
