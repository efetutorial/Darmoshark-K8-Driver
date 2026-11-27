use super::{
    icons::Icon,
    input::TextInput,
    theme::{theme, Theme},
    widgets::{self, button, card, card_title, field, hint, Kind},
    MacroRow, Recording, Studio,
};
use crate::{
    model,
    requests::{MacroAction, MacroEvent, WriteRequest},
};
use gpui::{div, prelude::*, px, AnyElement, Context, Modifiers, SharedString, Window};
use std::{rc::Rc, time::Instant};

const MODES: [(&str, &str); 3] = [
    ("repeat", "Repeat"),
    ("toggle", "Toggle on / off"),
    ("while_held", "While held"),
];

impl Studio {
    fn new_row(
        &mut self,
        key: String,
        action: MacroAction,
        delay: u16,
        cx: &mut Context<Self>,
    ) -> MacroRow {
        let input = cx.new(|cx| {
            TextInput::new(cx, "0")
                .digits()
                .max_len(5)
                .with_text(delay.to_string())
        });
        MacroRow {
            key,
            action,
            delay: input,
        }
    }

    pub(super) fn record_event(
        &mut self,
        key: String,
        action: MacroAction,
        at: Instant,
        cx: &mut Context<Self>,
    ) {
        let Some(recording) = self.recording.as_mut() else {
            return;
        };
        let elapsed = at
            .saturating_duration_since(recording.last)
            .as_millis()
            .min(65535) as u16;
        recording.last = at;
        match action {
            MacroAction::Down => {
                if recording.pressed.contains(&key) {
                    return;
                }
                recording.pressed.push(key.clone());
            }
            MacroAction::Up => {
                if !recording.pressed.contains(&key) {
                    return;
                }
                recording.pressed.retain(|pressed| *pressed != key);
            }
        }
        // The delay of an event is the wait after it, so set it on the previous row.
        if let Some(last) = self.macro_events.last() {
            last.delay
                .update(cx, |input, cx| input.set_text(elapsed.to_string(), cx));
        }
        let row = self.new_row(key, action, 30, cx);
        self.macro_events.push(row);
        cx.notify();
    }

    fn start_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.macro_events.clear();
        let device = crate::record::Recorder::start().ok();
        let detail = if device.is_some() {
            "Press keys on the K8, then Esc to finish"
        } else {
            "Press keys, then Esc to finish"
        };
        self.recording = Some(Recording {
            last: Instant::now(),
            modifiers: Modifiers::default(),
            pressed: Vec::new(),
            device,
        });
        window.focus(&self.focus);
        self.notify_toast("Recording started", detail, false, cx);
    }

    /// Apply events captured from the K8's own keyboard interface.
    pub(super) fn drain_recorder(&mut self, cx: &mut Context<Self>) {
        let events = match self.recording.as_ref().and_then(|r| r.device.as_ref()) {
            Some(device) => device.drain(),
            None => return,
        };
        for event in events {
            if event.usage == 41 {
                // Esc finishes recording and is never part of the macro.
                if event.down {
                    self.stop_recording(cx);
                    return;
                }
                continue;
            }
            if let Some(name) = model::usage_name(event.usage) {
                let action = if event.down {
                    MacroAction::Down
                } else {
                    MacroAction::Up
                };
                self.record_event(name, action, event.at, cx);
            }
        }
    }

    pub(super) fn stop_recording(&mut self, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.as_ref() else {
            return;
        };
        // Release anything still held so the macro never leaves keys down.
        let held = recording.pressed.clone();
        for key in held {
            self.record_event(key, MacroAction::Up, Instant::now(), cx);
        }
        self.recording = None;
        self.notify_toast(
            "Recording saved",
            format!("{} events", self.macro_events.len()),
            false,
            cx,
        );
    }

    fn save_macro(&mut self, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.stop_recording(cx);
        }
        if self.macro_events.is_empty() {
            self.notify_toast("Add or record events first", "", true, cx);
            return;
        }
        let Some(repeat_count) = self.macro_repeat.read(cx).number::<u16>() else {
            self.notify_toast("Invalid repeat count", "Use 0–65535.", true, cx);
            return;
        };
        let mut events = Vec::new();
        for row in &self.macro_events {
            let Some(delay) = row.delay.read(cx).number::<u16>() else {
                self.notify_toast("Invalid delay", "Delays must be 0–65535 ms.", true, cx);
                return;
            };
            events.push(MacroEvent {
                key: row.key.clone(),
                action: row.action,
                delay,
            });
        }
        let count = events.len();
        let slot = self.macro_slot;
        let request = WriteRequest::Macro {
            source: self.macro_trigger.clone(),
            events,
            index: slot,
            repeat_count,
            mode: MODES[self.macro_mode].0.into(),
            profile: self.profile,
        };
        self.job(
            "macro",
            cx,
            move |backend| backend.execute(request),
            move |this, result, cx| {
                this.report(
                    result,
                    "Macro saved",
                    format!("{count} events · Slot {slot}"),
                    "Could not save macro",
                    cx,
                )
            },
        );
    }

    fn key_menu(
        &self,
        t: &Theme,
        id: SharedString,
        current: &str,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Studio, String) + 'static,
    ) -> impl IntoElement {
        let open = self.menu_open(&id);
        let names: Vec<String> = model::LAYOUT
            .iter()
            .flat_map(|row| row.iter())
            .filter_map(|cap| cap.key.map(str::to_string))
            .collect();
        let view = cx.entity().downgrade();
        let toggle_id = id.clone();
        div()
            .relative()
            .child(
                widgets::select_button(t, id.clone(), model::key_label(current).to_string(), open)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_menu(toggle_id.clone(), cx)),
                    ),
            )
            .when(open, |this| {
                let items = names
                    .iter()
                    .map(|name| {
                        (
                            SharedString::from(model::key_label(name).to_string()),
                            name == current,
                        )
                    })
                    .collect();
                let pick = view.clone();
                let dismiss = view.clone();
                let apply = Rc::new(apply);
                this.child(widgets::menu(
                    t,
                    SharedString::from(format!("{id}-menu")),
                    items,
                    Rc::new(move |index, _, cx| {
                        let name = names[index].clone();
                        let apply = apply.clone();
                        pick.update(cx, |this, cx| {
                            apply(this, name);
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
            })
    }

    pub(super) fn render_macros(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme(cx);
        let view = cx.entity().downgrade();
        let recording = self.recording.is_some();

        let config = card(&t)
            .w(px(340.))
            .flex_none()
            .child(card_title(&t, "Macro"))
            .child(hint(
                &t,
                "Macros are stored on the keyboard and run even when this app is closed.",
            ))
            .child(field(
                &t,
                "Trigger key",
                self.key_menu(
                    &t,
                    "macro-trigger".into(),
                    &self.macro_trigger.clone(),
                    cx,
                    |this, name| this.macro_trigger = name,
                ),
            ))
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div().flex_1().child(field(
                            &t,
                            format!("Macro slot · {}", self.macro_slot),
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    widgets::icon_button(&t, "slot-down", Icon::Minimize).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.macro_slot = this.macro_slot.saturating_sub(1);
                                            cx.notify();
                                        }),
                                    ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_center()
                                        .text_size(px(13.))
                                        .text_color(t.text)
                                        .child(self.macro_slot.to_string()),
                                )
                                .child(widgets::icon_button(&t, "slot-up", Icon::Plus).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.macro_slot = (this.macro_slot + 1).min(49);
                                        cx.notify();
                                    }),
                                )),
                        )),
                    )
                    .child(div().flex_1().child(field(
                        &t,
                        "Repeat count",
                        self.macro_repeat.clone(),
                    ))),
            )
            .child(field(
                &t,
                "Playback",
                widgets::segmented(
                    &t,
                    "macro-mode",
                    MODES
                        .iter()
                        .map(|(_, label)| SharedString::from(*label))
                        .collect(),
                    self.macro_mode,
                    {
                        let view = view.clone();
                        Rc::new(move |index, _, cx| {
                            view.update(cx, |this, cx| {
                                this.macro_mode = index;
                                cx.notify();
                            })
                            .ok();
                        })
                    },
                ),
            ))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        button(
                            &t,
                            "macro-record",
                            if recording {
                                "Stop recording"
                            } else {
                                "Record"
                            },
                            if recording {
                                Kind::Danger
                            } else {
                                Kind::Secondary
                            },
                            Some(if recording { Icon::Stop } else { Icon::Record }),
                            false,
                        )
                        .flex_1()
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.recording.is_some() {
                                this.stop_recording(cx)
                            } else {
                                this.start_recording(window, cx)
                            }
                        })),
                    )
                    .child(
                        button(&t, "macro-clear", "Clear", Kind::Secondary, None, recording)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.macro_events.clear();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                button(
                    &t,
                    "save-macro",
                    "Save macro",
                    Kind::Primary,
                    None,
                    self.is_busy("macro"),
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| this.save_macro(cx))),
            );

        let rows: Vec<AnyElement> = (0..self.macro_events.len())
            .map(|index| {
                let row = &self.macro_events[index];
                let key = row.key.clone();
                let action = row.action;
                let delay = row.delay.clone();
                let action_view = view.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(28.))
                            .text_size(px(11.))
                            .text_color(t.faint)
                            .child(format!("{}", index + 1)),
                    )
                    .child(div().flex_1().child(self.key_menu(
                        &t,
                        SharedString::from(format!("event-key-{index}")),
                        &key,
                        cx,
                        move |this, name| {
                            if let Some(row) = this.macro_events.get_mut(index) {
                                row.key = name;
                            }
                        },
                    )))
                    .child(div().w(px(170.)).child(widgets::segmented(
                        &t,
                        SharedString::from(format!("event-action-{index}")),
                        vec!["Press".into(), "Release".into()],
                        (action == MacroAction::Up) as usize,
                        Rc::new(move |choice, _, cx| {
                            action_view
                                .update(cx, |this, cx| {
                                    if let Some(row) = this.macro_events.get_mut(index) {
                                        row.action = if choice == 0 {
                                            MacroAction::Down
                                        } else {
                                            MacroAction::Up
                                        };
                                    }
                                    cx.notify();
                                })
                                .ok();
                        }),
                    )))
                    .child(
                        div()
                            .w(px(110.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(delay)
                            .child(div().text_size(px(11.)).text_color(t.faint).child("ms")),
                    )
                    .child(
                        widgets::icon_button(&t, ("event-delete", index), Icon::Trash).on_click(
                            cx.listener(move |this, _, _, cx| {
                                if index < this.macro_events.len() {
                                    this.macro_events.remove(index);
                                }
                                cx.notify();
                            }),
                        ),
                    )
                    .into_any_element()
            })
            .collect();

        let timeline = card(&t)
            .flex_1()
            .min_w(px(460.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(card_title(&t, format!("{} events", self.macro_events.len())))
                    .child(
                        button(&t, "macro-add", "Add event", Kind::Secondary, Some(Icon::Plus), recording)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let row = this.new_row("A".into(), MacroAction::Down, 60, cx);
                                this.macro_events.push(row);
                                cx.notify();
                            })),
                    ),
            )
            .when(recording, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .bg(t.danger.opacity(0.1))
                        .child(div().size(px(8.)).rounded(px(4.)).bg(t.danger))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.text)
                                .child("Recording — press keys on your keyboard, then Esc to finish."),
                        ),
                )
            })
            .when(self.macro_events.is_empty() && !recording, |this| {
                this.child(hint(&t, "Record a sequence or add events by hand. Each delay is the wait after that event."))
            })
            .child(div().flex().flex_col().gap(px(6.)).children(rows));

        div()
            .flex()
            .gap(px(16.))
            .items_start()
            .child(config)
            .child(timeline)
            .into_any_element()
    }
}
