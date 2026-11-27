use super::{
    icons::Icon,
    input::{InputEvent, TextInput},
    keyboard::{keyboard, Mode},
    theme::{hex_color, hex_hsv, hsv_hex, theme, Theme},
    widgets::{self, button, card, card_title, field, hint, Kind},
    Studio,
};
use crate::{backend::LiveKind, model, presets::Preset, requests::WriteRequest};
use gpui::{div, hsla, Focusable, prelude::*, px, AnyElement, Context, MouseButton, SharedString, Window};
use std::rc::Rc;

impl Studio {
    fn set_color(&mut self, color: String, cx: &mut Context<Self>) {
        self.light.color = color.clone();
        self.light.rainbow = false;
        self.dirty_lighting = true;
        self.color_input.update(cx, |input, cx| input.set_text(color, cx));
        cx.notify();
    }

    fn apply_lighting(&mut self, quiet_name: Option<String>, cx: &mut Context<Self>) {
        let light = self.light.clone();
        let label = model::effect_label(&light.effect);
        self.job(
            "lighting",
            cx,
            move |backend| backend.execute(WriteRequest::Lighting(light)),
            move |this, result, cx| {
                if result.is_ok() {
                    this.dirty_lighting = false;
                    this.live = None;
                }
                match quiet_name {
                    Some(name) => this.report(result, "Preset applied", name, "Could not apply preset", cx),
                    None => this.report(result, "Lighting applied", label, "Could not apply lighting", cx),
                }
            },
        );
    }

    fn start_live(&mut self, kind: LiveKind, cx: &mut Context<Self>) {
        self.light.effect = match kind {
            LiveKind::Screen => "screen_color".into(),
            LiveKind::Audio if self.light.effect == "music_2" => "music_2".into(),
            LiveKind::Audio => "music_3".into(),
        };
        let light = self.light.clone();
        self.job(
            "live",
            cx,
            move |backend| backend.start_live(kind, light),
            move |this, result, cx| match result {
                Ok(()) => {
                    this.live = Some(kind);
                    this.dirty_lighting = false;
                    let title = match kind {
                        LiveKind::Screen => "Screen sync started",
                        LiveKind::Audio => "Audio sync started",
                    };
                    this.notify_toast(title, "", false, cx);
                }
                Err(error) => {
                    this.live = None;
                    this.notify_toast("Could not start live sync", error, true, cx);
                }
            },
        );
        cx.notify();
    }

    fn stop_live(&mut self, cx: &mut Context<Self>) {
        self.job(
            "live",
            cx,
            |backend| {
                backend.stop_live();
                Ok(())
            },
            |this, _: Result<(), String>, cx| {
                this.live = None;
                this.notify_toast("Live sync stopped", "", false, cx);
            },
        );
    }

    fn open_preset_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = format!("Preset {}", chrono::Local::now().format("%H:%M"));
        let input = cx.new(|cx| TextInput::new(cx, "Preset name").max_len(100).with_text(name));
        let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::Submit) {
                this.save_preset(window, cx);
            }
        });
        self._subscriptions.push(subscription);
        window.focus(&input.focus_handle(cx));
        self.preset_name = Some(input);
        cx.notify();
    }

    fn save_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.preset_name.as_ref() else {
            return;
        };
        let name = input.read(cx).text().trim().to_string();
        let preset = Preset::from_lighting(name.clone(), &self.light);
        let result = self.presets.add(preset);
        self.report(result, "Preset saved", name, "Could not save preset", cx);
        self.close_overlays(window, cx);
    }

    pub(super) fn render_preset_modal(&self, t: &Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let input = self.preset_name.as_ref()?;
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(6.))
            .child(card_title(t, "Save preset"))
            .child(hint(t, "Stores the current effect, color, brightness, speed and direction."))
            .child(input.clone())
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button(t, "preset-cancel", "Cancel", Kind::Secondary, None, false)
                            .on_click(cx.listener(|this, _, window, cx| this.close_overlays(window, cx))),
                    )
                    .child(
                        button(t, "preset-save", "Save", Kind::Primary, None, false)
                            .on_click(cx.listener(|this, _, window, cx| this.save_preset(window, cx))),
                    ),
            );
        Some(Self::modal(t, "preset-modal", cx, body))
    }

    fn render_color_picker(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.light.color.clone();
        let (hue, _, _) = hex_hsv(&current).unwrap_or((0.0, 1.0, 1.0));
        let segments = 36;
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .size(px(32.))
                            .flex_none()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(t.border_strong)
                            .bg(hex_color(&current).unwrap_or(t.control)),
                    )
                    .child(div().w(px(110.)).child(self.color_input.clone()))
                    .child(
                        div().flex().gap(px(6.)).children(model::SWATCHES.iter().map(|swatch| {
                            let swatch = *swatch;
                            let selected = current.eq_ignore_ascii_case(swatch);
                            div()
                                .id(SharedString::from(format!("swatch-{swatch}")))
                                .size(px(20.))
                                .rounded(px(10.))
                                .cursor_pointer()
                                .bg(hex_color(swatch).unwrap())
                                .border_2()
                                .border_color(if selected { t.text } else { t.border })
                                .on_click(cx.listener(move |this, _, _, cx| this.set_color(swatch.into(), cx)))
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .h(px(14.))
                    .rounded(px(7.))
                    .overflow_hidden()
                    .border_1()
                    .border_color(t.border)
                    .children((0..segments).map(|index| {
                        let h = index as f32 * 360.0 / segments as f32;
                        let active = ((hue / (360.0 / segments as f32)).round() as usize) % segments == index;
                        let pick = cx.listener(move |this, _: &gpui::MouseDownEvent, _, cx| {
                            this.set_color(hsv_hex(h, 1.0, 1.0), cx)
                        });
                        let drag = cx.listener(move |this, event: &gpui::MouseMoveEvent, _, cx| {
                            if event.pressed_button == Some(MouseButton::Left) {
                                this.set_color(hsv_hex(h, 1.0, 1.0), cx)
                            }
                        });
                        div()
                            .id(("hue", index))
                            .flex_1()
                            .h_full()
                            .cursor_pointer()
                            .bg(hsla(h / 360.0, 1.0, 0.5, 1.0))
                            .when(active, |this| this.border_x_2().border_color(gpui::white()))
                            .on_mouse_down(MouseButton::Left, pick)
                            .on_mouse_move(drag)
                    })),
            )
    }

    fn render_controls(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let caps = model::capabilities(&self.light.effect);
        let options = model::effect_options(&self.light.effect);
        let effect_label = model::effect_label(&self.light.effect);
        let open = self.menu_open("effect");
        let busy = self.is_busy("lighting");
        let view = cx.entity().downgrade();

        let effect_select = div()
            .relative()
            .child(
                widgets::select_button(t, "effect-select", effect_label, open)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_menu("effect", cx))),
            )
            .when(open, |this| {
                let picks: Vec<String> = options.iter().map(|(id, _)| id.clone()).collect();
                let items = options
                    .iter()
                    .map(|(id, label)| (SharedString::from(label.clone()), *id == self.light.effect))
                    .collect();
                let pick_view = view.clone();
                let dismiss_view = view.clone();
                this.child(widgets::menu(
                    t,
                    "effect-menu",
                    items,
                    Rc::new(move |index, _, cx| {
                        let effect = picks[index].clone();
                        pick_view
                            .update(cx, |this, cx| {
                                this.light.effect = effect;
                                let directions = model::capabilities(&this.light.effect).directions;
                                if !directions.iter().any(|(value, _)| *value == this.light.direction) {
                                    this.light.direction = "right".into();
                                }
                                this.dirty_lighting = true;
                                this.open_menu = None;
                                cx.notify();
                            })
                            .ok();
                    }),
                    Rc::new(move |_, cx| {
                        dismiss_view
                            .update(cx, |this, cx| {
                                this.open_menu = None;
                                cx.notify();
                            })
                            .ok();
                    }),
                ))
            });

        let step_handler = |set: fn(&mut Studio, u8)| {
            let view = view.clone();
            Rc::new(move |value: u8, _: &mut Window, cx: &mut gpui::App| {
                view.update(cx, |this, cx| {
                    set(this, value);
                    this.dirty_lighting = true;
                    cx.notify();
                })
                .ok();
            }) as Rc<dyn Fn(u8, &mut Window, &mut gpui::App)>
        };

        card(t)
            .w(px(360.))
            .flex_none()
            .child(field(t, "Effect", effect_select))
            .when(caps.color, |this| {
                this.child(field(t, "Color", self.render_color_picker(t, cx)))
            })
            .when(caps.rainbow, |this| {
                this.child(
                    widgets::switch_row(t, "rainbow", "Rainbow colors", self.light.rainbow)
                        .mx(px(-10.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.light.rainbow = !this.light.rainbow;
                            this.dirty_lighting = true;
                            cx.notify();
                        })),
                )
            })
            .when(!caps.directions.is_empty(), |this| {
                let selected = caps
                    .directions
                    .iter()
                    .position(|(value, _)| *value == self.light.direction)
                    .unwrap_or(0);
                let values: Vec<&'static str> = caps.directions.iter().map(|(value, _)| *value).collect();
                let labels = caps.directions.iter().map(|(_, label)| SharedString::from(*label)).collect();
                let view = view.clone();
                this.child(field(
                    t,
                    "Direction",
                    widgets::segmented(
                        t,
                        "direction",
                        labels,
                        selected,
                        Rc::new(move |index, _, cx| {
                            let value = values[index];
                            view.update(cx, |this, cx| {
                                this.light.direction = value.into();
                                this.dirty_lighting = true;
                                cx.notify();
                            })
                            .ok();
                        }),
                    ),
                ))
            })
            .when(caps.brightness, |this| {
                this.child(field(
                    t,
                    format!("Brightness · {} / 4", self.light.brightness),
                    widgets::steps(t, "brightness", self.light.brightness, 4, step_handler(|s, v| s.light.brightness = v)),
                ))
            })
            .when(caps.speed, |this| {
                this.child(field(
                    t,
                    format!("Speed · {} / 4", self.light.speed),
                    widgets::steps(t, "speed", self.light.speed, 4, step_handler(|s, v| s.light.speed = v)),
                ))
            })
            .when(caps.slot, |this| {
                let view = view.clone();
                this.child(field(
                    t,
                    "Custom key slot",
                    widgets::segmented(
                        t,
                        "rgb-slot",
                        (1..=4).map(|n| SharedString::from(n.to_string())).collect(),
                        self.light.slot.saturating_sub(1) as usize,
                        Rc::new(move |index, _, cx| {
                            view.update(cx, |this, cx| {
                                this.light.slot = index as u8 + 1;
                                this.dirty_lighting = true;
                                cx.notify();
                            })
                            .ok();
                        }),
                    ),
                ))
            })
            .when_some(caps.note, |this, note| this.child(hint(t, note)))
            .child(
                button(t, "apply-lighting", if busy { "Applying…" } else { "Apply lighting" }, Kind::Primary, None, busy)
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| this.apply_lighting(None, cx))),
            )
    }

    fn render_live(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let levels = crate::live::latest_levels();
        let audio = self.live == Some(LiveKind::Audio);
        let starting = self.is_busy("live");
        card(t)
            .flex_1()
            .min_w(px(320.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(card_title(t, "Live sync"))
                    .child(widgets::badge(
                        t,
                        match self.live {
                            Some(LiveKind::Screen) => "Screen",
                            Some(LiveKind::Audio) => "Audio",
                            None => "Idle",
                        },
                    )),
            )
            .child(hint(t, "Mirror your screen's average color, or drive the audio modes from what is playing."))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(3.))
                    .h(px(46.))
                    .children(levels.iter().map(|level| {
                        let level = if audio { *level } else { 0 };
                        div()
                            .flex_1()
                            .h(px(4. + level as f32 * 7.))
                            .rounded(px(2.))
                            .bg(if audio { t.text } else { t.active })
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        button(t, "live-screen", "Screen color", Kind::Secondary, Some(Icon::Screen), starting)
                            .on_click(cx.listener(|this, _, _, cx| this.start_live(LiveKind::Screen, cx))),
                    )
                    .child(
                        button(t, "live-audio", "Audio spectrum", Kind::Secondary, Some(Icon::Music), starting)
                            .on_click(cx.listener(|this, _, _, cx| this.start_live(LiveKind::Audio, cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        button(t, "live-stop", "Stop", Kind::Danger, Some(Icon::Stop), self.live.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.live.is_some() {
                                    this.stop_live(cx)
                                }
                            })),
                    ),
            )
    }

    fn render_presets(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let hover = t.hover;
        card(t)
            .flex_1()
            .min_w(px(320.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(card_title(t, "Presets"))
                    .child(
                        button(t, "save-preset", "Save current", Kind::Secondary, Some(Icon::Plus), false)
                            .on_click(cx.listener(|this, _, window, cx| this.open_preset_modal(window, cx))),
                    ),
            )
            .when(self.presets.presets.is_empty(), |this| {
                this.child(hint(t, "No presets yet. Save the current lighting to reuse it later."))
            })
            .child(
                div().flex().flex_wrap().gap(px(8.)).children(self.presets.presets.iter().enumerate().map(
                    |(index, preset)| {
                        let swatch = if preset.rainbow {
                            hsla(0.8, 0.7, 0.6, 1.0)
                        } else {
                            hex_color(&preset.color).unwrap_or(t.control)
                        };
                        div()
                            .id(("preset", index))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .pl(px(8.))
                            .pr(px(4.))
                            .py(px(4.))
                            .rounded(px(8.))
                            .bg(t.control)
                            .border_1()
                            .border_color(t.border)
                            .cursor_pointer()
                            .hover(move |style| style.bg(hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(preset) = this.presets.presets.get(index).cloned() {
                                    this.light = preset.lighting();
                                    let color = this.light.color.clone();
                                    this.color_input.update(cx, |input, cx| input.set_text(color, cx));
                                    this.apply_lighting(Some(preset.name), cx);
                                }
                            }))
                            .child(div().size(px(12.)).rounded(px(6.)).bg(swatch))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_size(px(12.)).text_color(t.text).child(preset.name.clone()))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(t.faint)
                                            .child(model::effect_label(&preset.effect)),
                                    ),
                            )
                            .child(
                                widgets::icon_button(t, ("preset-delete", index), Icon::Trash)
                                    .size(px(24.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        let result = this.presets.remove(index);
                                        if let Err(error) = result {
                                            this.notify_toast("Could not delete preset", error, true, cx);
                                        }
                                        cx.notify();
                                    })),
                            )
                    },
                )),
            )
    }

    pub(super) fn render_lighting(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let status = match self.live {
            Some(LiveKind::Screen) => "Screen sync",
            Some(LiveKind::Audio) => "Audio sync",
            None if self.dirty_lighting => "Unsaved preview",
            None => "Preview",
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .items_start()
                    .child(
                        card(&t)
                            .flex_1()
                            .min_w(px(420.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(card_title(&t, "Keyboard"))
                                    .child(widgets::badge(&t, status)),
                            )
                            .child(keyboard(
                                &t,
                                "preview",
                                Mode::Preview {
                                    light: &self.light,
                                    phase: self.phase,
                                },
                            ))
                            .child(hint(
                                &t,
                                "The preview approximates the effect. Changes reach the keyboard when you apply them.",
                            )),
                    )
                    .child(self.render_controls(&t, cx)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .items_start()
                    .child(self.render_live(&t, cx))
                    .child(self.render_presets(&t, cx)),
            )
            .into_any_element()
    }
}
