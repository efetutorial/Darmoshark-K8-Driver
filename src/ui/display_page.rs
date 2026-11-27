use super::{
    icons::Icon,
    theme::theme,
    widgets::{self, button, card, card_title, field, hint, icon, Kind},
    Studio,
};
use crate::backend::Progress;
use gpui::{
    div, img, prelude::*, px, relative, AnyElement, Context, ObjectFit, PathPromptOptions,
    SharedString, Window,
};
use std::{path::PathBuf, rc::Rc};

const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

fn is_gif(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
}

impl Studio {
    pub(super) fn choose_display_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.is_busy("upload") {
            self.notify_toast(
                "Upload in progress",
                "Wait before selecting another image.",
                true,
                cx,
            );
            return;
        }
        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_FILE_BYTES => {
                self.notify_toast("Image exceeds 100 MB", "", true, cx);
            }
            Ok(_) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.display_file = Some(path);
                self.progress = Progress::default();
                self.notify_toast("Image selected", name, false, cx);
            }
            Err(error) => self.notify_toast("Could not open image", error.to_string(), true, cx),
        }
        cx.notify();
    }

    fn browse_display_file(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose an image or GIF".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            if let Some(path) = paths.into_iter().next() {
                this.update(cx, |this, cx| this.choose_display_file(path, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn upload_display(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.display_file.clone() else {
            self.notify_toast("Choose an image or GIF first", "", true, cx);
            return;
        };
        let gif = is_gif(&path);
        let delay = if gif {
            let text = self.display_delay.read(cx).text().trim().to_string();
            if text.is_empty() {
                None
            } else {
                match text.parse::<u8>() {
                    Ok(value) if value >= 1 => Some(value),
                    _ => {
                        self.notify_toast(
                            "Invalid GIF delay",
                            "Use 1–255 ms, or leave it empty for automatic timing.",
                            true,
                            cx,
                        );
                        return;
                    }
                }
            }
        } else {
            None
        };
        let slot = if gif { 0 } else { self.display_slot };
        self.live = None;
        self.progress = Progress {
            active: true,
            value: 0.0,
            label: "Preparing image".into(),
        };
        self.job(
            "upload",
            cx,
            move |backend| backend.upload_display(&path, slot, delay),
            move |this, result, cx| {
                this.progress = this.backend.progress();
                match result {
                    Ok(done) => {
                        let detail = if done.frames > 1 {
                            format!("GIF · {} frames · {} ms", done.frames, done.delay)
                        } else {
                            format!("Slot {}", slot + 1)
                        };
                        this.notify_toast("Display updated", detail, false, cx);
                    }
                    Err(error) => this.notify_toast("Upload failed", error, true, cx),
                }
            },
        );
    }

    pub(super) fn render_display(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme(cx);
        let view = cx.entity().downgrade();
        let uploading = self.is_busy("upload");
        let gif = self.display_file.as_deref().is_some_and(is_gif);
        let (name, size) = match &self.display_file {
            Some(path) => (
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                std::fs::metadata(path)
                    .map(|m| format!("{:.1} KB", m.len() as f64 / 1024.0))
                    .unwrap_or_default(),
            ),
            None => ("No image selected".into(), "—".into()),
        };
        let hover = t.hover;

        let preview = card(&t)
            .w(px(340.))
            .flex_none()
            .items_center()
            .child(
                div()
                    .p(px(14.))
                    .rounded(px(18.))
                    .bg(if t.dark { t.frame } else { t.control })
                    .border_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .size(px(256.))
                            .rounded(px(6.))
                            .overflow_hidden()
                            .bg(gpui::black())
                            .flex()
                            .items_center()
                            .justify_center()
                            .map(|this| match &self.display_file {
                                Some(path) => this.child(
                                    img(path.clone()).size_full().object_fit(ObjectFit::Contain),
                                ),
                                None => this.child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(gpui::rgb(0x5a5a5a))
                                        .child("128 × 128"),
                                ),
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .w_full()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.muted)
                            .truncate()
                            .child(name),
                    )
                    .child(div().text_size(px(12.)).text_color(t.faint).child(size)),
            );

        let progress = self.progress.clone();
        let upload = card(&t)
            .flex_1()
            .min_w(px(360.))
            .child(card_title(&t, "Image or GIF"))
            .child(hint(&t, "PNG, JPEG, WebP or GIF. Images are scaled to 128×128 with black letterboxing."))
            .child(
                div()
                    .id("drop-zone")
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .h(px(130.))
                    .rounded(px(10.))
                    .border_1()
                    .border_dashed()
                    .border_color(t.border_strong)
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(|this, _, _, cx| this.browse_display_file(cx)))
                    .child(icon(Icon::Image, 22., t.muted))
                    .child(div().text_size(px(13.)).text_color(t.text).child("Drop an image here, or browse"))
                    .child(div().text_size(px(12.)).text_color(t.faint).child("Up to 100 MB")),
            )
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .when(!gif, |this| {
                        let view = view.clone();
                        this.child(div().flex_1().child(field(
                            &t,
                            "Display slot",
                            widgets::segmented(
                                &t,
                                "display-slot",
                                (1..=5).map(|n| SharedString::from(n.to_string())).collect(),
                                self.display_slot as usize,
                                Rc::new(move |index, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.display_slot = index as u8;
                                        cx.notify();
                                    })
                                    .ok();
                                }),
                            ),
                        )))
                    })
                    .when(gif, |this| {
                        this.child(div().w(px(160.)).child(field(&t, "Frame delay (ms)", self.display_delay.clone())))
                    }),
            )
            .child(hint(
                &t,
                if gif {
                    "GIFs keep up to 222 frames. The keyboard uses one delay for the whole animation; leave it empty to use the GIF's average."
                } else {
                    "Only the selected photo slot is replaced."
                },
            ))
            .child(
                button(
                    &t,
                    "upload-display",
                    if uploading { "Uploading…" } else { "Upload to display" },
                    Kind::Primary,
                    Some(Icon::Upload),
                    uploading || self.display_file.is_none(),
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    if !this.is_busy("upload") && this.display_file.is_some() {
                        this.upload_display(cx)
                    }
                })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .h(px(4.))
                            .w_full()
                            .rounded(px(2.))
                            .bg(t.active)
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(progress.value.clamp(0.0, 1.0) as f32))
                                    .rounded(px(2.))
                                    .bg(t.text),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.faint)
                            .child(if progress.label.is_empty() { "Ready".to_string() } else { progress.label }),
                    ),
            );

        div()
            .flex()
            .gap(px(16.))
            .items_start()
            .child(preview)
            .child(upload)
            .into_any_element()
    }
}
