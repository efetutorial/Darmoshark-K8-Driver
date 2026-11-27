use super::{
    icons::Icon,
    theme::{theme, Theme},
    widgets::{self, button, card, card_title, field, hint, icon, Kind},
    Confirm, ConfirmAction, Studio,
};
use crate::{controller::KeyboardOptions, requests::WriteRequest};
use gpui::{div, prelude::*, px, AnyElement, Context, PathPromptOptions, SharedString, Window};
use std::rc::Rc;

const RATES: [u16; 4] = [125, 250, 500, 1000];

type OptionField = (
    &'static str,
    &'static str,
    fn(&mut KeyboardOptions) -> &mut bool,
);

const OPTIONS: &[OptionField] = &[
    ("win_lock", "Lock Windows key", |o| &mut o.win_lock),
    ("mac_mode", "macOS mode", |o| &mut o.mac_mode),
    ("auto_os", "Detect operating system automatically", |o| {
        &mut o.auto_os
    }),
    ("swap_wasd_arrows", "Swap WASD and arrows", |o| {
        &mut o.swap_wasd_arrows
    }),
    ("gaming_mode", "Gaming mode", |o| &mut o.gaming_mode),
    ("fn_remap", "Fn remapping", |o| &mut o.fn_remap),
    ("power_save", "Power saving", |o| &mut o.power_save),
    ("led_off", "Turn off key lighting", |o| &mut o.led_off),
    ("side_led_off", "Turn off side lighting", |o| {
        &mut o.side_led_off
    }),
];

impl Studio {
    fn apply_performance(&mut self, cx: &mut Context<Self>) {
        let (hz, milliseconds, profile) = (self.rate, self.debounce, self.profile);
        self.job(
            "performance",
            cx,
            move |backend| {
                backend.execute(WriteRequest::Rate { hz, profile })?;
                backend.execute(WriteRequest::Debounce { milliseconds })
            },
            move |this, result, cx| {
                this.report(
                    result,
                    "Performance saved",
                    format!("{hz} Hz · {milliseconds} ms"),
                    "Could not save performance",
                    cx,
                );
                this.refresh(true, cx);
            },
        );
    }

    fn apply_options(&mut self, cx: &mut Context<Self>) {
        let (options, profile) = (self.options.clone(), self.profile);
        self.job(
            "options",
            cx,
            move |backend| backend.execute(WriteRequest::Options { profile, options }),
            move |this, result, cx| {
                if result.is_ok() {
                    this.dirty_settings = false;
                }
                this.report(
                    result,
                    "Keyboard options saved",
                    format!("Profile {}", profile + 1),
                    "Could not save options",
                    cx,
                );
            },
        );
    }

    fn apply_sleep(&mut self, cx: &mut Context<Self>) {
        let values: Vec<Option<u16>> = self
            .sleep
            .iter()
            .map(|input| input.read(cx).number())
            .collect();
        let Some(values) = values.into_iter().collect::<Option<Vec<u16>>>() else {
            self.notify_toast(
                "Could not save timers",
                "Enter a number of minutes in every field.",
                true,
                cx,
            );
            return;
        };
        let request = WriteRequest::Sleep {
            bluetooth: values[0],
            wireless: values[1],
            bluetooth_deep: values[2],
            wireless_deep: values[3],
        };
        self.job(
            "sleep",
            cx,
            move |backend| backend.execute(request),
            |this, result, cx| {
                if result.is_ok() {
                    this.dirty_settings = false;
                }
                this.report(
                    result,
                    "Sleep timers saved",
                    String::new(),
                    "Could not save timers",
                    cx,
                );
            },
        );
    }

    pub(super) fn factory_reset(&mut self, cx: &mut Context<Self>) {
        self.job(
            "reset",
            cx,
            |backend| backend.execute(WriteRequest::Reset {}),
            |this, result, cx| {
                this.report(
                    result,
                    "Factory settings restored",
                    String::new(),
                    "Reset failed",
                    cx,
                );
                this.refresh(true, cx);
            },
        );
    }

    fn export_presets(&mut self, cx: &mut Context<Self>) {
        let directory = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some("darmoshark-k8-studio.json"));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = receiver.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let result = this.presets.export_to(&path);
                this.report(
                    result,
                    "Presets exported",
                    path.display().to_string(),
                    "Could not export presets",
                    cx,
                );
            })
            .ok();
        })
        .detach();
    }

    fn import_presets(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import presets".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| match this.presets.import_from(&path) {
                Ok(count) => {
                    this.notify_toast("Presets imported", format!("{count} presets"), false, cx)
                }
                Err(error) => this.notify_toast("Could not import file", error, true, cx),
            })
            .ok();
        })
        .detach();
    }

    fn tool_row(
        t: &Theme,
        id: &'static str,
        glyph: super::icons::Icon,
        title: &'static str,
        detail: &'static str,
        danger: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let hover = t.hover;
        let color = if danger { t.danger } else { t.text };
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(10.))
            .py(px(9.))
            .mx(px(-10.))
            .rounded(px(8.))
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child(icon(glyph, 16., color))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(div().text_size(px(13.)).text_color(color).child(title))
                    .child(div().text_size(px(12.)).text_color(t.faint).child(detail)),
            )
    }

    pub(super) fn render_settings(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme(cx);
        let view = cx.entity().downgrade();
        let rate_index = RATES
            .iter()
            .position(|rate| *rate == self.rate)
            .unwrap_or(3);
        let sleep_labels = [
            "Bluetooth sleep",
            "2.4 GHz sleep",
            "Bluetooth deep sleep",
            "2.4 GHz deep sleep",
        ];

        let performance = card(&t)
            .child(card_title(&t, "Performance"))
            .child(field(
                &t,
                "USB report rate",
                widgets::segmented(
                    &t,
                    "rate",
                    RATES.iter().map(|rate| SharedString::from(format!("{rate} Hz"))).collect(),
                    rate_index,
                    {
                        let view = view.clone();
                        Rc::new(move |index, _, cx| {
                            view.update(cx, |this, cx| {
                                this.rate = RATES[index];
                                this.dirty_settings = true;
                                cx.notify();
                            })
                            .ok();
                        })
                    },
                ),
            ))
            .child(hint(&t, "1000 Hz gives the shortest report interval. It is not a measurement of total key latency."))
            .child(field(
                &t,
                format!("Debounce · {} ms", self.debounce),
                widgets::steps(&t, "debounce", self.debounce, 10, {
                    let view = view.clone();
                    Rc::new(move |value, _, cx| {
                        view.update(cx, |this, cx| {
                            this.debounce = value;
                            this.dirty_settings = true;
                            cx.notify();
                        })
                        .ok();
                    })
                }),
            ))
            .child(
                button(&t, "apply-performance", "Apply performance", Kind::Primary, None, self.is_busy("performance"))
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| this.apply_performance(cx))),
            );

        let options = card(&t)
            .child(card_title(
                &t,
                format!("Keyboard options · Profile {}", self.profile + 1),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(OPTIONS.iter().map(|(id, label, get)| {
                        let get = *get;
                        let mut copy = self.options.clone();
                        let on = *get(&mut copy);
                        widgets::switch_row(&t, *id, *label, on)
                            .mx(px(-10.))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let value = get(&mut this.options);
                                *value = !*value;
                                this.dirty_settings = true;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                button(
                    &t,
                    "apply-options",
                    "Save options",
                    Kind::Secondary,
                    None,
                    self.is_busy("options"),
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| this.apply_options(cx))),
            );

        let sleep =
            card(&t)
                .child(card_title(&t, "Sleep timers"))
                .child(hint(
                    &t,
                    "Minutes. Sleep 0–60; deep sleep 0 (off) or 10–60.",
                ))
                .child(div().flex().flex_wrap().gap(px(12.)).children(
                    self.sleep.iter().zip(sleep_labels).map(|(input, label)| {
                        div().w(px(150.)).child(field(&t, label, input.clone()))
                    }),
                ))
                .child(
                    button(
                        &t,
                        "apply-sleep",
                        "Save timers",
                        Kind::Secondary,
                        None,
                        self.is_busy("sleep"),
                    )
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| this.apply_sleep(cx))),
                );

        let tools = card(&t)
            .child(card_title(&t, "Tools"))
            .child(
                Self::tool_row(&t, "export", Icon::Download, "Export presets", "Save presets to a file", false)
                    .on_click(cx.listener(|this, _, _, cx| this.export_presets(cx))),
            )
            .child(
                Self::tool_row(&t, "import", Icon::Upload, "Import presets", "Load a saved preset file", false)
                    .on_click(cx.listener(|this, _, _, cx| this.import_presets(cx))),
            )
            .child(
                Self::tool_row(&t, "reset", Icon::Reset, "Factory reset", "Erase all custom keyboard settings", true)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm = Some(Confirm {
                            title: "Factory reset".into(),
                            detail: "Erase all custom keyboard settings and restore factory defaults? This cannot be undone.".into(),
                            confirm_label: "Reset keyboard".into(),
                            action: ConfirmAction::FactoryReset,
                        });
                        cx.notify();
                    })),
            );

        div()
            .flex()
            .gap(px(16.))
            .items_start()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .flex_1()
                    .min_w(px(340.))
                    .child(performance)
                    .child(sleep)
                    .child(tools),
            )
            .child(div().flex_1().min_w(px(340.)).child(options))
            .into_any_element()
    }
}
