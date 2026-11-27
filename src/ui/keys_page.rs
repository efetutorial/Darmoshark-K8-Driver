use super::{
    keyboard::{keyboard, Mode},
    theme::{hex_color, theme},
    widgets::{self, button, card, card_title, field, hint, Kind},
    Studio,
};
use crate::{
    keys, model,
    requests::{Modifier, WriteRequest},
};
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use std::rc::Rc;

const MODIFIERS: [(Modifier, &str); 5] = [
    (Modifier::None, "None"),
    (Modifier::Ctrl, "Ctrl"),
    (Modifier::Shift, "Shift"),
    (Modifier::Alt, "Alt"),
    (Modifier::Meta, "Meta"),
];

/// Keys sorted by HID usage so the list reads A–Z, 1–0, then the rest.
pub(super) fn key_names() -> Vec<String> {
    let mut list: Vec<(String, u8)> = keys::keys().into_iter().collect();
    list.sort_by_key(|(_, usage)| *usage);
    list.into_iter().map(|(name, _)| name).collect()
}

fn all_targets() -> Vec<String> {
    let mut targets = vec!["disabled".to_string()];
    targets.extend(key_names());
    targets.extend(keys::target_names());
    targets
}

impl Studio {
    fn apply_remap(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.selected_key else {
            self.notify_toast("Select a key first", "", true, cx);
            return;
        };
        let second_usage = match self.second_key.as_deref() {
            None => 0,
            Some(name) => match keys::parse_key(name) {
                Ok(usage) => usage,
                Err(error) => {
                    self.notify_toast("Could not assign key", error, true, cx);
                    return;
                }
            },
        };
        let target = self.target.clone();
        let detail = format!(
            "{} → {}",
            model::key_label(source),
            model::target_label(&target)
        );
        let request = WriteRequest::Remap {
            source: source.into(),
            target,
            modifier: self.modifier,
            second_usage,
            profile: self.profile,
            fn_layer: self.fn_layer.then_some(0),
        };
        self.job(
            "remap",
            cx,
            move |backend| backend.execute(request),
            move |this, result, cx| {
                this.report(
                    result,
                    "Key assignment saved",
                    detail,
                    "Could not assign key",
                    cx,
                )
            },
        );
    }

    fn apply_key_color(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.selected_key else {
            self.notify_toast("Select a key first", "", true, cx);
            return;
        };
        let (color, slot) = (self.key_color.clone(), self.key_slot);
        let request = WriteRequest::KeyColor {
            key: key.into(),
            color: color.clone(),
            slot,
        };
        self.job(
            "key-color",
            cx,
            move |backend| backend.execute(request),
            move |this, result, cx| {
                if result.is_ok() {
                    this.painted.insert(key, color);
                }
                this.report(
                    result,
                    "Key color saved",
                    format!("{} · Slot {}", model::key_label(key), slot + 1),
                    "Could not save key color",
                    cx,
                );
            },
        );
    }

    fn set_key_color(&mut self, color: String, cx: &mut Context<Self>) {
        self.key_color = color.clone();
        self.key_color_input
            .update(cx, |input, cx| input.set_text(color, cx));
        cx.notify();
    }

    pub(super) fn render_keys(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme(cx);
        let view = cx.entity().downgrade();
        let selected = self.selected_key;
        let query = self.target_search.read(cx).text().to_lowercase();
        let hover = t.hover;

        let picker = card(&t)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(card_title(
                        &t,
                        selected
                            .map(|key| format!("Selected · {}", model::key_label(key)))
                            .unwrap_or_else(|| "Select a key".into()),
                    ))
                    .child(div().w(px(180.)).child(widgets::segmented(
                        &t,
                        "layer",
                        vec!["Normal".into(), "Fn layer".into()],
                        self.fn_layer as usize,
                        {
                            let view = view.clone();
                            Rc::new(move |index, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.fn_layer = index == 1;
                                    cx.notify();
                                })
                                .ok();
                            })
                        },
                    ))),
            )
            .child(keyboard(
                &t,
                "keymap",
                Mode::Picker {
                    selected,
                    painted: &self.painted,
                    on_select: {
                        let view = view.clone();
                        Rc::new(move |key, _, cx| {
                            view.update(cx, |this, cx| {
                                this.selected_key = Some(key);
                                cx.notify();
                            })
                            .ok();
                        })
                    },
                },
            ))
            .child(hint(&t, "Fn, the LCD and the logo key cannot be remapped."));

        let targets: Vec<String> = all_targets()
            .into_iter()
            .filter(|target| {
                query.is_empty()
                    || target.to_lowercase().contains(&query)
                    || model::target_label(target).to_lowercase().contains(&query)
            })
            .collect();

        let second_open = self.menu_open("second-key");
        let second_names = key_names();
        let second_label = self
            .second_key
            .as_deref()
            .map(|name| model::target_label(name))
            .unwrap_or_else(|| "None".into());
        let second_select = div()
            .relative()
            .child(
                widgets::select_button(&t, "second-key", second_label, second_open)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_menu("second-key", cx))),
            )
            .when(second_open, |this| {
                let mut items: Vec<(SharedString, bool)> =
                    vec![("None".into(), self.second_key.is_none())];
                items.extend(second_names.iter().map(|name| {
                    (
                        SharedString::from(model::target_label(name)),
                        self.second_key.as_deref() == Some(name),
                    )
                }));
                let names = second_names.clone();
                let pick = view.clone();
                let dismiss = view.clone();
                this.child(widgets::menu(
                    &t,
                    "second-key-menu",
                    items,
                    Rc::new(move |index, _, cx| {
                        let value = (index > 0).then(|| names[index - 1].clone());
                        pick.update(cx, |this, cx| {
                            this.second_key = value;
                            this.open_menu = None;
                            cx.notify();
                        })
                        .ok();
                    }),
                    Rc::new(move |_, cx| {
                        dismiss
                            .update(cx, |this, cx| {
                                this.open_menu = None;
                                cx.notify();
                            })
                            .ok();
                    }),
                ))
            });

        let assignment = card(&t)
            .flex_1()
            .min_w(px(360.))
            .child(card_title(
                &t,
                if self.fn_layer {
                    "Fn layer assignment"
                } else {
                    "Key assignment"
                },
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(widgets::badge(
                        &t,
                        selected.map(model::key_label).unwrap_or("—"),
                    ))
                    .child(div().text_size(px(13.)).text_color(t.faint).child("→"))
                    .child(widgets::badge(&t, model::target_label(&self.target))),
            )
            .child(self.target_search.clone())
            .child(
                div()
                    .id("target-list")
                    .h(px(220.))
                    .overflow_y_scroll()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(t.border)
                    .p(px(4.))
                    .when(targets.is_empty(), |this| {
                        this.child(hint(&t, "No matching keys or actions.").p(px(8.)))
                    })
                    .children(targets.into_iter().map(|target| {
                        let active = target == self.target;
                        let label = model::target_label(&target);
                        let group = model::target_group(&target);
                        div()
                            .id(SharedString::from(format!("target-{target}")))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(8.))
                            .py(px(5.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(active, |this| this.bg(t.active))
                            .hover(move |style| style.bg(hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.target = target.clone();
                                cx.notify();
                            }))
                            .child(div().text_size(px(13.)).text_color(t.text).child(label))
                            .child(div().text_size(px(11.)).text_color(t.faint).child(group))
                    })),
            )
            .child(field(
                &t,
                "Modifier",
                widgets::segmented(
                    &t,
                    "modifier",
                    MODIFIERS
                        .iter()
                        .map(|(_, label)| SharedString::from(*label))
                        .collect(),
                    MODIFIERS
                        .iter()
                        .position(|(m, _)| *m == self.modifier)
                        .unwrap_or(0),
                    {
                        let view = view.clone();
                        Rc::new(move |index, _, cx| {
                            view.update(cx, |this, cx| {
                                this.modifier = MODIFIERS[index].0;
                                cx.notify();
                            })
                            .ok();
                        })
                    },
                ),
            ))
            .child(field(&t, "Second key (optional)", second_select))
            .child(
                button(
                    &t,
                    "apply-remap",
                    "Apply assignment",
                    Kind::Primary,
                    None,
                    self.is_busy("remap"),
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| this.apply_remap(cx))),
            );

        let color = card(&t)
            .w(px(320.))
            .flex_none()
            .child(card_title(&t, "Key color"))
            .child(hint(
                &t,
                "Used by the Custom keys lighting mode. Pick a slot, then paint keys one by one.",
            ))
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
                            .bg(hex_color(&self.key_color).unwrap_or(t.control)),
                    )
                    .child(self.key_color_input.clone()),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .children(model::SWATCHES.iter().map(|swatch| {
                        let swatch = *swatch;
                        let active = self.key_color.eq_ignore_ascii_case(swatch);
                        div()
                            .id(SharedString::from(format!("key-swatch-{swatch}")))
                            .size(px(22.))
                            .rounded(px(11.))
                            .cursor_pointer()
                            .bg(hex_color(swatch).unwrap())
                            .border_2()
                            .border_color(if active { t.text } else { t.border })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_key_color(swatch.into(), cx)
                            }))
                    })),
            )
            .child(field(
                &t,
                "Lighting slot",
                widgets::segmented(
                    &t,
                    "key-slot",
                    (1..=4).map(|n| SharedString::from(n.to_string())).collect(),
                    self.key_slot as usize,
                    {
                        let view = view.clone();
                        Rc::new(move |index, _, cx| {
                            view.update(cx, |this, cx| {
                                this.key_slot = index as u8;
                                cx.notify();
                            })
                            .ok();
                        })
                    },
                ),
            ))
            .child(
                button(
                    &t,
                    "apply-key-color",
                    "Apply key color",
                    Kind::Secondary,
                    None,
                    self.is_busy("key-color"),
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| this.apply_key_color(cx))),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(picker)
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .items_start()
                    .child(assignment)
                    .child(color),
            )
            .into_any_element()
    }
}
