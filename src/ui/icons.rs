//! Small line icons drawn for this app (24×24, 1.75px strokes). GPUI renders
//! SVGs as monochrome masks, so the stroke color here is irrelevant.

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Lighting,
    Display,
    Keyboard,
    Macro,
    Settings,
    Refresh,
    Activity,
    Search,
    Close,
    Minimize,
    Maximize,
    Restore,
    Plus,
    Trash,
    Upload,
    Download,
    Warning,
    Record,
    Stop,
    Music,
    Screen,
    Check,
    ChevronDown,
    Image,
    Reset,
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Icon::Lighting => "icons/lighting.svg",
            Icon::Display => "icons/display.svg",
            Icon::Keyboard => "icons/keyboard.svg",
            Icon::Macro => "icons/macro.svg",
            Icon::Settings => "icons/settings.svg",
            Icon::Refresh => "icons/refresh.svg",
            Icon::Activity => "icons/activity.svg",
            Icon::Search => "icons/search.svg",
            Icon::Close => "icons/close.svg",
            Icon::Minimize => "icons/minimize.svg",
            Icon::Maximize => "icons/maximize.svg",
            Icon::Restore => "icons/restore.svg",
            Icon::Plus => "icons/plus.svg",
            Icon::Trash => "icons/trash.svg",
            Icon::Upload => "icons/upload.svg",
            Icon::Download => "icons/download.svg",
            Icon::Warning => "icons/warning.svg",
            Icon::Record => "icons/record.svg",
            Icon::Stop => "icons/stop.svg",
            Icon::Music => "icons/music.svg",
            Icon::Screen => "icons/screen.svg",
            Icon::Check => "icons/check.svg",
            Icon::ChevronDown => "icons/chevron-down.svg",
            Icon::Image => "icons/image.svg",
            Icon::Reset => "icons/reset.svg",
        }
    }
}

fn wrap(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
    )
}

fn body(path: &str) -> Option<&'static str> {
    Some(match path {
        "icons/lighting.svg" => {
            r#"<path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4 7 17M17 7l1.4-1.4"/><circle cx="12" cy="12" r="3.5"/>"#
        }
        "icons/display.svg" => {
            r#"<rect x="5" y="4" width="14" height="14" rx="2.5"/><path d="M9 21h6"/><path d="m8 14 2.5-3 2 2 1.5-2 2 3"/>"#
        }
        "icons/keyboard.svg" => {
            r#"<rect x="2.5" y="6" width="19" height="12" rx="2.5"/><path d="M6.5 10h1M10 10h1M13.5 10h1M17 10h.5M6.5 13.5h1M17 13.5h.5M10 14h4"/>"#
        }
        "icons/macro.svg" => {
            r#"<path d="M4 7h6M4 12h10M4 17h6"/><path d="m15 14 5 3-5 3z"/>"#
        }
        "icons/settings.svg" => {
            r#"<path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="18" r="2"/>"#
        }
        "icons/refresh.svg" => {
            r#"<path d="M20 11a8 8 0 0 0-14.3-4.6L4 8"/><path d="M4 4v4h4"/><path d="M4 13a8 8 0 0 0 14.3 4.6L20 16"/><path d="M20 20v-4h-4"/>"#
        }
        "icons/activity.svg" => {
            r#"<path d="M8 6h12M8 12h12M8 18h12"/><circle cx="4" cy="6" r=".6"/><circle cx="4" cy="12" r=".6"/><circle cx="4" cy="18" r=".6"/>"#
        }
        "icons/search.svg" => r#"<circle cx="11" cy="11" r="6.5"/><path d="m20 20-4.2-4.2"/>"#,
        "icons/close.svg" => r#"<path d="M6 6l12 12M18 6 6 18"/>"#,
        "icons/minimize.svg" => r#"<path d="M6 12h12"/>"#,
        "icons/maximize.svg" => r#"<rect x="6" y="6" width="12" height="12" rx="1.5"/>"#,
        "icons/restore.svg" => {
            r#"<rect x="5" y="8.5" width="10.5" height="10.5" rx="1.5"/><path d="M8.5 5.5h8.5a1.5 1.5 0 0 1 1.5 1.5v8.5"/>"#
        }
        "icons/plus.svg" => r#"<path d="M12 5v14M5 12h14"/>"#,
        "icons/trash.svg" => {
            r#"<path d="M4.5 7h15M10 4h4M6.5 7l.8 12a1.5 1.5 0 0 0 1.5 1.4h6.4a1.5 1.5 0 0 0 1.5-1.4l.8-12"/>"#
        }
        "icons/upload.svg" => r#"<path d="M12 15V4M7.5 8.5 12 4l4.5 4.5"/><path d="M5 15v3.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V15"/>"#,
        "icons/download.svg" => r#"<path d="M12 4v11M7.5 10.5 12 15l4.5-4.5"/><path d="M5 15v3.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V15"/>"#,
        "icons/warning.svg" => {
            r#"<path d="M12 4 2.8 19.5h18.4z"/><path d="M12 10v4M12 16.8v.2"/>"#
        }
        "icons/record.svg" => r#"<circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="3" fill="black"/>"#,
        "icons/stop.svg" => r#"<rect x="7" y="7" width="10" height="10" rx="1.5"/>"#,
        "icons/music.svg" => {
            r#"<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>"#
        }
        "icons/screen.svg" => {
            r#"<rect x="3" y="4.5" width="18" height="12" rx="2"/><path d="M8.5 20h7M12 16.5V20"/>"#
        }
        "icons/check.svg" => r#"<path d="m5 12.5 4.5 4.5L19 7.5"/>"#,
        "icons/chevron-down.svg" => r#"<path d="m7 10 5 5 5-5"/>"#,
        "icons/image.svg" => {
            r#"<rect x="4" y="4" width="16" height="16" rx="2.5"/><circle cx="9" cy="9.5" r="1.5"/><path d="m5 18 5-5 3 3 2-2 4 4"/>"#
        }
        "icons/reset.svg" => r#"<path d="M4 12a8 8 0 1 0 2.4-5.7L4 8.5"/><path d="M4 4v4.5h4.5"/>"#,
        _ => return None,
    })
}

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(body(path).map(|svg| Cow::Owned(wrap(svg).into_bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(if path.starts_with("icons") {
            ALL.iter().map(|icon| icon.path().into()).collect()
        } else {
            Vec::new()
        })
    }
}

const ALL: &[Icon] = &[
    Icon::Lighting,
    Icon::Display,
    Icon::Keyboard,
    Icon::Macro,
    Icon::Settings,
    Icon::Refresh,
    Icon::Activity,
    Icon::Search,
    Icon::Close,
    Icon::Minimize,
    Icon::Maximize,
    Icon::Restore,
    Icon::Plus,
    Icon::Trash,
    Icon::Upload,
    Icon::Download,
    Icon::Warning,
    Icon::Record,
    Icon::Stop,
    Icon::Music,
    Icon::Screen,
    Icon::Check,
    Icon::ChevronDown,
    Icon::Image,
    Icon::Reset,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_an_svg() {
        for icon in ALL {
            assert!(Assets.load(icon.path()).unwrap().is_some(), "{icon:?}");
        }
    }
}
