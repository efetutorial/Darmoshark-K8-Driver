//! The on-screen K8, used as a lighting preview and as a key picker.

use super::theme::{hex_color, Theme};
use crate::{controller::LightingState, model};
use gpui::{div, hsla, prelude::*, px, relative, App, Div, Hsla, SharedString, Window};
use std::{collections::HashMap, rc::Rc};

const ROW_UNITS: f32 = 16.05;

pub enum Mode<'a> {
    Preview {
        light: &'a LightingState,
        phase: f32,
    },
    Picker {
        selected: Option<&'static str>,
        painted: &'a HashMap<&'static str, String>,
        on_select: Rc<dyn Fn(&'static str, &mut Window, &mut App)>,
    },
}

/// The LED color a preview key shows for an effect at an animation phase.
fn preview_color(light: &LightingState, x: f32, y: f32, phase: f32) -> Option<Hsla> {
    use std::f32::consts::TAU;
    let level = 0.3 + 0.7 * light.brightness as f32 / 4.0;
    let base = hex_color(&light.color).unwrap_or(hsla(0., 0., 1., 1.));
    let caps = model::capabilities(&light.effect);
    let rainbow = caps.rainbow && light.rainbow || light.effect == "neon";
    // Base LED color at this key, before the effect's brightness pattern.
    let color = |offset: f32| {
        if rainbow {
            hsla((offset + phase * 0.5).rem_euclid(1.0), 0.85, 0.6, 1.0)
        } else {
            base
        }
    };
    let hue_offset = x * 0.8 + y * 0.1;
    Some(match light.effect.as_str() {
        "off" | "screen_color" | "user_picture" | "user_color" => return None,
        "solid" | "neon" => color(hue_offset).opacity(level),
        "breathing" => color(hue_offset).opacity(level * (0.25 + 0.75 * (phase * TAU).sin().abs())),
        "wave" | "line_wave" | "sine" | "circle_wave" | "kaleidoscope" | "fireworks" | "snake" => {
            let along = match light.direction.as_str() {
                "left" => 1.0 - x,
                "down" => y,
                "up" => 1.0 - y,
                _ => x,
            };
            let pulse = 0.5 + 0.5 * ((along - phase) * TAU).cos();
            color(hue_offset).opacity(level * (0.2 + 0.8 * pulse))
        }
        "music_2" | "music_3" => {
            let bar = (((x * 9.0).floor() * 1.7 + phase * 6.0).sin() * 0.5 + 0.5) * 6.0;
            let lit = (1.0 - y) * 6.0 <= bar + 0.5;
            color(hue_offset).opacity(level * if lit { 1.0 } else { 0.12 })
        }
        _ => {
            // Twinkle for the reactive/raindrop family.
            let seed = ((x * 37.0 + y * 91.0).sin() * 43758.5).fract().abs();
            let on = ((seed + phase) * 3.0).fract() < 0.18;
            color(seed).opacity(level * if on { 1.0 } else { 0.18 })
        }
    })
}

pub fn keyboard(t: &Theme, id: &'static str, mode: Mode<'_>) -> Div {
    let rows = model::LAYOUT.len() as f32;
    div()
        .flex()
        .flex_col()
        .w_full()
        .p(px(10.))
        .rounded(px(14.))
        .bg(if t.dark { t.frame } else { t.control })
        .border_1()
        .border_color(t.border)
        .children(model::LAYOUT.iter().enumerate().map(|(row_index, row)| {
            let mut cursor = 0.0f32;
            div()
                .flex()
                .w_full()
                .children(row.iter().enumerate().map(|(col_index, cap)| {
                    let start = cursor;
                    cursor += cap.width;
                    let width = relative(cap.width / ROW_UNITS);
                    if cap.gap {
                        return div().w(width).into_any_element();
                    }
                    let x = (start + cap.width / 2.0) / ROW_UNITS;
                    let y = row_index as f32 / (rows - 1.0);
                    let (led, selected, clickable) = match &mode {
                        Mode::Preview { light, phase } => {
                            (preview_color(light, x, y, *phase), false, false)
                        }
                        Mode::Picker {
                            selected, painted, ..
                        } => (
                            cap.key
                                .and_then(|key| painted.get(key))
                                .and_then(|c| hex_color(c)),
                            cap.key.is_some() && *selected == cap.key,
                            cap.key.is_some(),
                        ),
                    };
                    let label: SharedString = cap.label.into();
                    let hover = t.hover;
                    let mut keycap = div()
                        .id((id, row_index * 32 + col_index))
                        .relative()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .size_full()
                        .items_center()
                        .px(px(2.))
                        .pt(px(4.))
                        .pb(px(4.))
                        .rounded(px(6.))
                        .bg(t.keycap)
                        .border_1()
                        .border_color(if selected { t.text } else { t.keycap_border })
                        .text_size(px(9.5))
                        .text_color(if cap.key.is_some() { t.muted } else { t.faint })
                        .child(div().overflow_hidden().whitespace_nowrap().child(label))
                        .child(
                            div()
                                .h(px(3.))
                                .w(gpui::relative(0.7))
                                .rounded(px(2.))
                                .bg(led.unwrap_or(gpui::transparent_black())),
                        );
                    if let Some(led) = led {
                        keycap = keycap.bg(t.keycap.blend(led.opacity(led.a * 0.16)));
                    }
                    if let (Mode::Picker { on_select, .. }, Some(key), true) =
                        (&mode, cap.key, clickable)
                    {
                        let on_select = on_select.clone();
                        keycap = keycap
                            .cursor_pointer()
                            .hover(move |style| style.bg(hover))
                            .on_click(move |_, window, cx| on_select(key, window, cx));
                    }
                    div()
                        .w(width)
                        .h(px(40.))
                        .p(px(2.))
                        .child(keycap)
                        .into_any_element()
                }))
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(effect: &str) -> LightingState {
        LightingState {
            effect: effect.into(),
            brightness: 4,
            speed: 2,
            color: "#ff0000".into(),
            rainbow: false,
            direction: "right".into(),
            slot: 1,
        }
    }

    #[test]
    fn unlit_modes_have_no_preview_color() {
        assert!(preview_color(&light("off"), 0.5, 0.5, 0.0).is_none());
        assert!(preview_color(&light("screen_color"), 0.5, 0.5, 0.0).is_none());
        assert!(preview_color(&light("solid"), 0.5, 0.5, 0.0).is_some());
    }

    #[test]
    fn rows_fit_the_layout_width() {
        for row in model::LAYOUT {
            let units: f32 = row.iter().map(|cap| cap.width).sum();
            assert!(units <= ROW_UNITS + 0.01, "{units}");
        }
    }
}
