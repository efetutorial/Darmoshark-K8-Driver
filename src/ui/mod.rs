//! GPUI front end.

mod display_page;
mod icons;
mod input;
mod keyboard;
mod keys_page;
mod lighting;
mod macros;
mod settings;
pub mod theme;
mod widgets;

use crate::{
    backend::{Backend, LiveKind, Progress},
    controller::{KeyboardOptions, LightingState, Status},
    presets,
    requests::{MacroAction, Modifier},
};
use gpui::{
    actions, canvas, div, point, prelude::*, px, size, App, Application, Bounds, Context,
    CursorStyle, Decorations, Entity, FocusHandle, Focusable, HitboxBehavior, KeyBinding,
    KeyDownEvent, KeyUpEvent, Modifiers, ModifiersChangedEvent, MouseButton, Pixels, Point,
    ResizeEdge, SharedString, Size, Subscription, TitlebarOptions, Window, WindowBounds,
    WindowControlArea, WindowDecorations, WindowOptions,
};
use icons::Icon;
use input::{InputEvent, TextInput};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use theme::{theme, Theme};
use widgets::{button, icon, icon_button, Kind};

actions!(
    studio,
    [
        OpenPalette,
        Refresh,
        Escape,
        PaletteUp,
        PaletteDown,
        GoLighting,
        GoDisplay,
        GoKeys,
        GoMacros,
        GoSettings,
        Quit
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Lighting,
    Display,
    Keys,
    Macros,
    Settings,
}

impl Page {
    const ALL: [Page; 5] = [
        Page::Lighting,
        Page::Display,
        Page::Keys,
        Page::Macros,
        Page::Settings,
    ];

    fn title(self) -> &'static str {
        match self {
            Page::Lighting => "Lighting",
            Page::Display => "Display",
            Page::Keys => "Keys",
            Page::Macros => "Macros",
            Page::Settings => "Settings",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Page::Lighting => "Effects, colors, presets and live sync",
            Page::Display => "Upload an image or GIF to the LCD",
            Page::Keys => "Remap keys and paint per-key colors",
            Page::Macros => "Record macros that run on the keyboard",
            Page::Settings => "Performance, options, sleep and tools",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::Lighting => Icon::Lighting,
            Page::Display => Icon::Display,
            Page::Keys => Icon::Keyboard,
            Page::Macros => Icon::Macro,
            Page::Settings => Icon::Settings,
        }
    }
}

struct Toast {
    id: u64,
    title: SharedString,
    detail: SharedString,
    error: bool,
}

struct Activity {
    title: SharedString,
    detail: SharedString,
    error: bool,
    time: SharedString,
}

struct Palette {
    input: Entity<TextInput>,
    index: usize,
}

#[derive(Clone, Copy)]
enum ConfirmAction {
    FactoryReset,
}

struct Confirm {
    title: SharedString,
    detail: SharedString,
    confirm_label: SharedString,
    action: ConfirmAction,
}

pub(crate) struct MacroRow {
    pub key: String,
    pub action: MacroAction,
    pub delay: Entity<TextInput>,
}

pub(crate) struct Recording {
    pub last: Instant,
    pub modifiers: Modifiers,
    pub pressed: Vec<String>,
    /// Direct HID capture from the K8 (Linux). `None` uses window key events.
    pub device: Option<crate::record::Recorder>,
}

pub struct Studio {
    backend: Arc<Backend>,
    focus: FocusHandle,
    page: Page,
    connected: bool,
    /// Set when the keyboard is present but could not be read (e.g. busy).
    device_error: Option<String>,
    status: Option<Status>,
    profile: u8,
    busy: HashSet<&'static str>,
    dirty_lighting: bool,
    dirty_settings: bool,
    open_menu: Option<SharedString>,
    toast: Option<Toast>,
    toast_seq: u64,
    activities: Vec<Activity>,
    drawer_open: bool,
    palette: Option<Palette>,
    confirm: Option<Confirm>,
    preset_name: Option<Entity<TextInput>>,
    // Lighting
    light: LightingState,
    color_input: Entity<TextInput>,
    live: Option<LiveKind>,
    presets: presets::Store,
    phase: f32,
    // Display
    display_file: Option<PathBuf>,
    display_slot: u8,
    display_delay: Entity<TextInput>,
    progress: Progress,
    // Keys
    selected_key: Option<&'static str>,
    fn_layer: bool,
    target: String,
    target_search: Entity<TextInput>,
    modifier: Modifier,
    second_key: Option<String>,
    key_color: String,
    key_color_input: Entity<TextInput>,
    key_slot: u8,
    painted: HashMap<&'static str, String>,
    // Macros
    macro_trigger: String,
    macro_slot: u8,
    macro_repeat: Entity<TextInput>,
    macro_mode: usize,
    macro_events: Vec<MacroRow>,
    recording: Option<Recording>,
    // Settings
    rate: u16,
    debounce: u8,
    options: KeyboardOptions,
    sleep: [Entity<TextInput>; 4],
    _subscriptions: Vec<Subscription>,
}

const DEFAULT_LIGHT: fn() -> LightingState = || LightingState {
    effect: "solid".into(),
    brightness: 4,
    speed: 2,
    color: "#ff6432".into(),
    rainbow: false,
    direction: "right".into(),
    slot: 1,
};

impl Studio {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let color_input = cx.new(|cx| TextInput::new(cx, "#rrggbb").hex().max_len(7).with_text("#ff6432"));
        let key_color_input =
            cx.new(|cx| TextInput::new(cx, "#rrggbb").hex().max_len(7).with_text("#ff6432"));
        let display_delay = cx.new(|cx| TextInput::new(cx, "Auto").digits().max_len(3));
        let target_search = cx.new(|cx| TextInput::new(cx, "Search keys and actions"));
        let macro_repeat = cx.new(|cx| TextInput::new(cx, "1").digits().max_len(5).with_text("1"));
        let sleep = std::array::from_fn(|_| cx.new(|cx| TextInput::new(cx, "0").digits().max_len(2)));

        let mut subscriptions = vec![
            cx.subscribe(&color_input, |this, input, event, cx| {
                if matches!(event, InputEvent::Changed | InputEvent::Submit) {
                    let text = input.read(cx).text().to_string();
                    if theme::hex_color(&text).is_some() {
                        this.light.color = text.to_lowercase();
                        this.light.rainbow = false;
                        this.dirty_lighting = true;
                        cx.notify();
                    }
                }
            }),
            cx.subscribe(&key_color_input, |this, input, event, cx| {
                if matches!(event, InputEvent::Changed | InputEvent::Submit) {
                    let text = input.read(cx).text().to_string();
                    if theme::hex_color(&text).is_some() {
                        this.key_color = text.to_lowercase();
                        cx.notify();
                    }
                }
            }),
            cx.subscribe(&target_search, |_, _, _, cx| cx.notify()),
        ];
        for input in &sleep {
            subscriptions.push(cx.subscribe(input, |this, _, event, _| {
                if matches!(event, InputEvent::Changed) {
                    this.dirty_settings = true;
                }
            }));
        }
        subscriptions.push(cx.observe_window_appearance(window, |_, window, cx| {
            cx.set_global(Theme::for_appearance(window.appearance()));
            cx.notify();
        }));

        window.set_window_title("Darmoshark K8 Studio");
        let store = presets::Store::load();
        let recovered = store.recovered;
        let mut studio = Self {
            backend: Backend::new(),
            focus: cx.focus_handle(),
            page: Page::Lighting,
            connected: false,
            device_error: None,
            status: None,
            profile: 0,
            busy: HashSet::new(),
            dirty_lighting: false,
            dirty_settings: false,
            open_menu: None,
            toast: None,
            toast_seq: 0,
            activities: Vec::new(),
            drawer_open: false,
            palette: None,
            confirm: None,
            preset_name: None,
            light: DEFAULT_LIGHT(),
            color_input,
            live: None,
            presets: store,
            phase: 0.0,
            display_file: None,
            display_slot: 0,
            display_delay,
            progress: Progress::default(),
            selected_key: None,
            fn_layer: false,
            target: "disabled".into(),
            target_search,
            modifier: Modifier::None,
            second_key: None,
            key_color: "#ff6432".into(),
            key_color_input,
            key_slot: 0,
            painted: HashMap::new(),
            macro_trigger: "F12".into(),
            macro_slot: 0,
            macro_repeat,
            macro_mode: 0,
            macro_events: Vec::new(),
            recording: None,
            rate: 1000,
            debounce: 1,
            options: KeyboardOptions::default(),
            sleep,
            _subscriptions: subscriptions,
        };
        if recovered {
            studio.notify_toast(
                "Recovered preset storage",
                "Invalid presets were skipped; a copy was kept next to the file.",
                true,
                cx,
            );
        }
        studio.page = initial_page();
        studio.refresh(true, cx);
        studio.start_background_loops(cx);
        studio
    }

    // ----- background work -------------------------------------------------

    /// Run blocking device work off the UI thread, then apply the result.
    fn job<T: Send + 'static>(
        &mut self,
        key: &'static str,
        cx: &mut Context<Self>,
        work: impl FnOnce(&Backend) -> Result<T, String> + Send + 'static,
        done: impl FnOnce(&mut Self, Result<T, String>, &mut Context<Self>) + 'static,
    ) {
        if !self.busy.insert(key) {
            return;
        }
        cx.notify();
        let backend = self.backend.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { work(&backend) })
                .await;
            this.update(cx, |this, cx| {
                this.busy.remove(key);
                done(this, result, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn is_busy(&self, key: &str) -> bool {
        self.busy.contains(key)
    }

    fn start_background_loops(&mut self, cx: &mut Context<Self>) {
        // Periodic device check; never overwrites unsaved edits.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(15)).await;
            if this.update(cx, |this, cx| this.refresh(false, cx)).is_err() {
                break;
            }
        })
        .detach();
        // Animation tick: lighting preview, audio spectrum and upload progress.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(60)).await;
            let alive = this.update(cx, |this, cx| {
                let mut redraw = false;
                if this.page == Page::Lighting {
                    this.phase = (this.phase + 0.012 * (1.0 + this.light.speed as f32)) % 1.0;
                    redraw = true;
                }
                if this.recording.as_ref().is_some_and(|r| r.device.is_some()) {
                    this.drain_recorder(cx);
                }
                if this.is_busy("upload") {
                    this.progress = this.backend.progress();
                    redraw = true;
                }
                if redraw {
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        })
        .detach();
    }

    /// Re-read connection and settings. `force` discards unsaved edits.
    fn refresh(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.is_busy("refresh") || self.is_busy("upload") {
            return;
        }
        if force {
            self.dirty_lighting = false;
            self.dirty_settings = false;
        }
        self.job(
            "refresh",
            cx,
            |backend| {
                if !backend.connected() {
                    return Ok(None);
                }
                backend.status().map(Some)
            },
            move |this, result, cx| match result {
                Ok(Some(status)) => {
                    this.connected = true;
                    this.device_error = None;
                    this.apply_status(status, cx);
                }
                Ok(None) => {
                    this.connected = false;
                    this.device_error = None;
                    this.status = None;
                }
                Err(error) => {
                    this.connected = false;
                    this.device_error = Some(error.clone());
                    if force {
                        this.notify_toast("Cannot read keyboard settings", error, true, cx);
                    }
                }
            },
        );
    }

    fn apply_status(&mut self, status: Status, cx: &mut Context<Self>) {
        self.profile = status.profile;
        self.live = self.backend.live_kind();
        if !self.dirty_lighting {
            self.light = status.lighting.clone();
            let color = self.light.color.clone();
            self.color_input.update(cx, |input, cx| input.set_text(color, cx));
        }
        if !self.dirty_settings {
            self.rate = status.report_rate;
            self.debounce = status.debounce;
            self.options = status.options.clone();
            let values = [
                status.sleep.bluetooth,
                status.sleep.wireless,
                status.sleep.bluetooth_deep,
                status.sleep.wireless_deep,
            ];
            for (input, seconds) in self.sleep.iter().zip(values) {
                let minutes = ((seconds as f32) / 60.0).round() as u16;
                input.update(cx, |input, cx| input.set_text(minutes.to_string(), cx));
            }
        }
        self.status = Some(status);
    }

    // ----- notifications ----------------------------------------------------

    fn notify_toast(
        &mut self,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        error: bool,
        cx: &mut Context<Self>,
    ) {
        let title = title.into();
        let detail = detail.into();
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.activities.insert(
            0,
            Activity {
                title: title.clone(),
                detail: detail.clone(),
                error,
                time: time.into(),
            },
        );
        self.activities.truncate(80);
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toast = Some(Toast {
            id,
            title,
            detail,
            error,
        });
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(4300))
                .await;
            this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|toast| toast.id == id) {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn report(&mut self, result: Result<(), String>, ok: &str, detail: String, fail: &str, cx: &mut Context<Self>) {
        match result {
            Ok(()) => self.notify_toast(ok.to_string(), detail, false, cx),
            Err(error) => self.notify_toast(fail.to_string(), error, true, cx),
        }
    }

    // ----- shell actions ----------------------------------------------------

    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.recording.is_some() && page != Page::Macros {
            self.stop_recording(cx);
        }
        self.page = page;
        self.open_menu = None;
        cx.notify();
    }

    fn set_profile(&mut self, profile: u8, cx: &mut Context<Self>) {
        self.job(
            "profile",
            cx,
            move |backend| backend.execute(crate::requests::WriteRequest::Profile { profile }),
            move |this, result, cx| {
                match result {
                    Ok(()) => {
                        this.profile = profile;
                        this.notify_toast("Profile changed", format!("Profile {}", profile + 1), false, cx);
                    }
                    Err(error) => this.notify_toast("Could not change profile", error, true, cx),
                }
                this.refresh(true, cx);
            },
        );
    }

    fn toggle_menu(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        self.open_menu = if self.open_menu.as_ref() == Some(&id) {
            None
        } else {
            Some(id)
        };
        cx.notify();
    }

    fn menu_open(&self, id: &str) -> bool {
        self.open_menu.as_ref().is_some_and(|open| open.as_ref() == id)
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new(cx, "Jump to a page or action…"));
        let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            match event {
                InputEvent::Changed => {
                    if let Some(palette) = this.palette.as_mut() {
                        palette.index = 0;
                    }
                    cx.notify();
                }
                InputEvent::Submit => this.run_palette(window, cx),
            }
        });
        self._subscriptions.push(subscription);
        window.focus(&input.focus_handle(cx));
        self.palette = Some(Palette { input, index: 0 });
        cx.notify();
    }

    fn palette_items(&self, cx: &App) -> Vec<(&'static str, &'static str, Page)> {
        let query = self
            .palette
            .as_ref()
            .map(|palette| palette.input.read(cx).text().to_lowercase())
            .unwrap_or_default();
        Page::ALL
            .iter()
            .map(|page| (page.title(), page.subtitle(), *page))
            .filter(|(title, subtitle, _)| {
                format!("{title} {subtitle}").to_lowercase().contains(&query)
            })
            .collect()
    }

    fn run_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.palette_items(cx);
        let index = self.palette.as_ref().map(|p| p.index).unwrap_or(0);
        if let Some((_, _, page)) = items.get(index.min(items.len().saturating_sub(1))) {
            self.navigate(*page, cx);
        }
        self.close_overlays(window, cx);
    }

    fn close_overlays(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.confirm = None;
        self.preset_name = None;
        self.drawer_open = false;
        self.open_menu = None;
        window.focus(&self.focus);
        cx.notify();
    }

    fn run_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        match confirm.action {
            ConfirmAction::FactoryReset => self.factory_reset(cx),
        }
        self.close_overlays(window, cx);
    }

    // ----- macro recording (window-level key capture) -----------------------

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.recording.is_none() || event.is_held {
            return;
        }
        cx.stop_propagation();
        if event.keystroke.key == "escape" {
            self.stop_recording(cx);
            return;
        }
        if self.recording.as_ref().is_some_and(|r| r.device.is_some()) {
            return;
        }
        if let Some(name) = crate::model::hid_name_for_key(&event.keystroke.key) {
            self.record_event(name, MacroAction::Down, Instant::now(), cx);
        }
    }

    fn on_key_up(&mut self, event: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.recording.is_none() {
            return;
        }
        cx.stop_propagation();
        if self.recording.as_ref().is_some_and(|r| r.device.is_some()) {
            return;
        }
        if let Some(name) = crate::model::hid_name_for_key(&event.keystroke.key) {
            self.record_event(name, MacroAction::Up, Instant::now(), cx);
        }
    }

    fn on_modifiers(&mut self, event: &ModifiersChangedEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.as_ref() else {
            return;
        };
        if recording.device.is_some() {
            return;
        }
        let before = recording.modifiers;
        let after = event.modifiers;
        let changes = [
            (before.control, after.control, "LeftCtrl"),
            (before.shift, after.shift, "LeftShift"),
            (before.alt, after.alt, "LeftAlt"),
            (before.platform, after.platform, "LeftMeta"),
        ];
        for (was, is, name) in changes {
            if was != is {
                let action = if is { MacroAction::Down } else { MacroAction::Up };
                self.record_event(name.into(), action, Instant::now(), cx);
            }
        }
        if let Some(recording) = self.recording.as_mut() {
            recording.modifiers = after;
        }
    }

    fn render_window_controls(&self, t: &Theme, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let maximized = window.is_maximized();
        let control = |id: &'static str, glyph: Icon, area: WindowControlArea, danger: bool| {
            let hover = if danger { t.danger } else { t.hover };
            let fg = t.muted;
            let hover_fg = if danger { gpui::white() } else { t.text };
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .rounded(px(7.))
                .cursor_pointer()
                .window_control_area(area)
                .hover(move |style| style.bg(hover).text_color(hover_fg))
                .text_color(fg)
                .child(
                    gpui::svg()
                        .path(glyph.path())
                        .size(px(14.))
                        .text_color(fg),
                )
        };
        div()
            .flex()
            .gap(px(2.))
            .child(
                control("win-min", Icon::Minimize, WindowControlArea::Min, false)
                    .on_click(cx.listener(|_, _, window, _| window.minimize_window())),
            )
            .child(
                control(
                    "win-max",
                    if maximized { Icon::Restore } else { Icon::Maximize },
                    WindowControlArea::Max,
                    false,
                )
                .on_click(cx.listener(|_, _, window, _| window.zoom_window())),
            )
            .child(
                control("win-close", Icon::Close, WindowControlArea::Close, true)
                    .on_click(cx.listener(|_, _, window, _| window.remove_window())),
            )
    }

    fn render_sidebar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let hover = t.hover;
        let connected = self.connected;
        let firmware = self
            .status
            .as_ref()
            .map(|status| format!("Firmware {}", status.firmware))
            .unwrap_or_else(|| "Wired mode required".into());
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(220.))
            .h_full()
            .px(px(10.))
            .pb(px(10.))
            .child(
                // Brand row doubles as a drag handle.
                div()
                    .id("brand")
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(52.))
                    .px(px(6.))
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(26.))
                            .rounded(px(7.))
                            .bg(t.primary)
                            .text_color(t.primary_text)
                            .text_size(px(11.))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("K8"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(t.text)
                                    .child("K8 Studio"),
                            )
                            .child(div().text_size(px(11.)).text_color(t.faint).child("Darmoshark")),
                    ),
            )
            .child(
                div()
                    .id("search")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(32.))
                    .px(px(10.))
                    .mb(px(10.))
                    .rounded(px(8.))
                    .bg(t.control)
                    .border_1()
                    .border_color(t.border)
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(|this, _, window, cx| this.open_palette(window, cx)))
                    .child(icon(Icon::Search, 14., t.faint))
                    .child(div().flex_1().text_size(px(12.)).text_color(t.faint).child("Search"))
                    .child(div().text_size(px(11.)).text_color(t.faint).child("Ctrl K")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .children(Page::ALL.iter().map(|page| {
                        let page = *page;
                        let active = self.page == page;
                        div()
                            .id(page.title())
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .h(px(34.))
                            .px(px(10.))
                            .rounded(px(8.))
                            .cursor_pointer()
                            .text_size(px(13.))
                            .when(active, |this| {
                                this.bg(t.surface)
                                    .border_1()
                                    .border_color(t.border)
                                    .text_color(t.text)
                                    .font_weight(gpui::FontWeight::MEDIUM)
                            })
                            .when(!active, |this| {
                                this.text_color(t.muted).hover(move |style| style.bg(hover))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
                            .child(icon(page.icon(), 16., if active { t.text } else { t.muted }))
                            .child(page.title())
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.faint)
                            .px(px(4.))
                            .child("PROFILE"),
                    )
                    .child(widgets::segmented(
                        t,
                        "profile",
                        vec!["1".into(), "2".into(), "3".into()],
                        self.profile as usize,
                        {
                            let view = cx.entity().downgrade();
                            std::rc::Rc::new(move |index, _, cx| {
                                view.update(cx, |this, cx| {
                                    if index as u8 != this.profile {
                                        this.set_profile(index as u8, cx)
                                    }
                                })
                                .ok();
                            })
                        },
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .p(px(10.))
                            .rounded(px(10.))
                            .bg(t.surface)
                            .border_1()
                            .border_color(t.border)
                            .child(
                                div()
                                    .size(px(8.))
                                    .rounded(px(4.))
                                    .bg(if connected { t.success } else { t.faint }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(t.text)
                                            .child(if connected { "Darmoshark K8" } else { "Disconnected" }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(t.faint)
                                            .truncate()
                                            .child(if connected { firmware } else { "Connect by USB cable".into() }),
                                    ),
                            ),
                    ),
            )
    }

    fn render_topbar(&self, t: &Theme, show_controls: bool, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let refreshing = self.is_busy("refresh");
        div()
            .id("topbar")
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(52.))
            .pl(px(4.))
            .pr(px(10.))
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count == 1 {
                    window.start_window_move()
                }
            })
            .on_click(cx.listener(|_, event: &gpui::ClickEvent, window, _| {
                if event.click_count() == 2 {
                    window.zoom_window();
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(t.text)
                            .child(self.page.title()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.faint)
                            .truncate()
                            .child(self.page.subtitle()),
                    ),
            )
            .child(
                icon_button(t, "refresh", Icon::Refresh)
                    .when(refreshing, |this| this.opacity(0.5))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(true, cx))),
            )
            .child(
                icon_button(t, "activity", Icon::Activity)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.drawer_open = !this.drawer_open;
                        cx.notify();
                    })),
            )
            .when(show_controls, |this| {
                this.child(div().w(px(1.)).h(px(18.)).mx(px(4.)).bg(t.border))
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(self.render_window_controls(t, window, cx)),
                    )
            })
    }

    fn render_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.page {
            Page::Lighting => self.render_lighting(window, cx),
            Page::Display => self.render_display(window, cx),
            Page::Keys => self.render_keys(window, cx),
            Page::Macros => self.render_macros(window, cx),
            Page::Settings => self.render_settings(window, cx),
        }
    }

    fn render_toast(&self, t: &Theme) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        Some(
            div()
                .absolute()
                .bottom(px(20.))
                .right(px(20.))
                .flex()
                .items_start()
                .gap(px(10.))
                .max_w(px(380.))
                .px(px(14.))
                .py(px(10.))
                .rounded(px(10.))
                .bg(t.card)
                .border_1()
                .border_color(t.border_strong)
                .shadow_lg()
                .child(div().mt(px(5.)).size(px(7.)).rounded(px(4.)).flex_none().bg(if toast.error {
                    t.danger
                } else {
                    t.success
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(toast.title.clone()),
                        )
                        .when(!toast.detail.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.muted)
                                    .child(toast.detail.clone()),
                            )
                        }),
                ),
        )
    }

    fn render_drawer(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_end()
            .child(
                div()
                    .id("drawer-scrim")
                    .absolute()
                    .inset_0()
                    .bg(t.overlay)
                    .on_click(cx.listener(|this, _, window, cx| this.close_overlays(window, cx))),
            )
            .child(
                div()
                    .id("drawer")
                    .occlude()
                    .flex()
                    .flex_col()
                    .w(px(360.))
                    .h_full()
                    .bg(t.surface)
                    .border_l_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .h(px(52.))
                            .px(px(16.))
                            .child(widgets::card_title(t, "Activity"))
                            .child(
                                icon_button(t, "drawer-close", Icon::Close)
                                    .on_click(cx.listener(|this, _, window, cx| this.close_overlays(window, cx))),
                            ),
                    )
                    .child(
                        div()
                            .id("activity-list")
                            .flex_1()
                            .overflow_y_scroll()
                            .px(px(12.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .when(self.activities.is_empty(), |this| {
                                this.child(widgets::hint(t, "No activity yet.").p(px(8.)))
                            })
                            .children(self.activities.iter().map(|entry| {
                                div()
                                    .flex()
                                    .gap(px(10.))
                                    .p(px(8.))
                                    .rounded(px(8.))
                                    .child(
                                        div()
                                            .mt(px(5.))
                                            .size(px(6.))
                                            .flex_none()
                                            .rounded(px(3.))
                                            .bg(if entry.error { t.danger } else { t.faint }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .text_color(t.text)
                                                    .child(entry.title.clone()),
                                            )
                                            .child(
                                                div().text_size(px(11.)).text_color(t.faint).child(
                                                    if entry.detail.is_empty() {
                                                        entry.time.to_string()
                                                    } else {
                                                        format!("{} · {}", entry.time, entry.detail)
                                                    },
                                                ),
                                            ),
                                    )
                            })),
                    )
                    .child(
                        div().p(px(12.)).child(
                            button(t, "activity-clear", "Clear activity", Kind::Secondary, None, false)
                                .w_full()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.activities.clear();
                                    cx.notify();
                                })),
                        ),
                    ),
            )
    }

    fn modal(t: &Theme, id: &'static str, cx: &mut Context<Self>, body: impl IntoElement) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .pt(px(120.))
            .child(
                div()
                    .id((id, 0usize))
                    .absolute()
                    .inset_0()
                    .bg(t.overlay)
                    .on_click(cx.listener(|this, _, window, cx| this.close_overlays(window, cx))),
            )
            .child(
                div()
                    .id(id)
                    .occlude()
                    .w(px(460.))
                    .h_auto()
                    .max_h(px(440.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .p(px(14.))
                    .rounded(px(14.))
                    .bg(t.card)
                    .border_1()
                    .border_color(t.border_strong)
                    .shadow_lg()
                    .child(body),
            )
    }

    fn render_palette(&self, t: &Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let palette = self.palette.as_ref()?;
        let items = self.palette_items(cx);
        let selected = palette.index.min(items.len().saturating_sub(1));
        let hover = t.hover;
        let body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(palette.input.clone())
            .child(
                div().flex().flex_col().gap(px(2.)).children(items.into_iter().enumerate().map(
                    |(index, (title, subtitle, page))| {
                        div()
                            .id(("palette-item", index))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(8.))
                            .cursor_pointer()
                            .when(index == selected, |this| this.bg(t.active))
                            .hover(move |style| style.bg(hover))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.navigate(page, cx);
                                this.close_overlays(window, cx);
                            }))
                            .child(icon(page.icon(), 16., t.muted))
                            .child(div().text_size(px(13.)).text_color(t.text).child(title))
                            .child(div().text_size(px(12.)).text_color(t.faint).child(subtitle))
                    },
                )),
            )
            .child(widgets::hint(t, "↑↓ select · Enter open · Esc close"));
        Some(Self::modal(t, "palette", cx, body))
    }

    fn render_confirm(&self, t: &Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let confirm = self.confirm.as_ref()?;
        let body = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(icon(Icon::Warning, 18., t.danger))
                    .child(widgets::card_title(t, confirm.title.clone())),
            )
            .child(widgets::hint(t, confirm.detail.clone()))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button(t, "confirm-cancel", "Cancel", Kind::Secondary, None, false)
                            .on_click(cx.listener(|this, _, window, cx| this.close_overlays(window, cx))),
                    )
                    .child(
                        button(t, "confirm-ok", confirm.confirm_label.clone(), Kind::Danger, None, false)
                            .on_click(cx.listener(|this, _, window, cx| this.run_confirm(window, cx))),
                    ),
            );
        Some(Self::modal(t, "confirm", cx, body))
    }
}

impl Focusable for Studio {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// `--page <name>` opens a specific page (handy for screenshots and docs).
fn initial_page() -> Page {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--page" {
            if let Some(name) = args.next() {
                if let Some(page) = Page::ALL.iter().find(|page| page.title().eq_ignore_ascii_case(&name)) {
                    return *page;
                }
            }
        }
    }
    Page::Lighting
}

const RESIZE_EDGE: Pixels = px(6.);

fn resize_edge(pos: Point<Pixels>, size: Size<Pixels>) -> Option<ResizeEdge> {
    let e = RESIZE_EDGE;
    let left = pos.x < e;
    let right = pos.x > size.width - e;
    let top = pos.y < e;
    let bottom = pos.y > size.height - e;
    Some(match (top, bottom, left, right) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (true, _, _, true) => ResizeEdge::TopRight,
        (_, true, true, _) => ResizeEdge::BottomLeft,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (true, ..) => ResizeEdge::Top,
        (_, true, ..) => ResizeEdge::Bottom,
        (_, _, true, _) => ResizeEdge::Left,
        (_, _, _, true) => ResizeEdge::Right,
        _ => return None,
    })
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let decorations = window.window_decorations();
        let client_side = matches!(decorations, Decorations::Client { .. });
        let show_controls = client_side || cfg!(target_os = "windows");
        let page = self.render_page(window, cx);
        let offline = !self.connected;

        div()
            .id("studio")
            .key_context("Studio")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .flex()
            .bg(t.frame)
            .text_color(t.text)
            .text_size(px(13.))
            .font_family(".SystemUIFont")
            .when(client_side, |this| {
                this.border_1().border_color(t.border_strong).rounded(px(10.))
            })
            .on_action(cx.listener(|this, _: &OpenPalette, window, cx| this.open_palette(window, cx)))
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.refresh(true, cx)))
            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                if this.recording.is_some() {
                    this.stop_recording(cx);
                } else {
                    this.close_overlays(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PaletteUp, _, cx| {
                if let Some(palette) = this.palette.as_mut() {
                    palette.index = palette.index.saturating_sub(1);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &PaletteDown, _, cx| {
                let count = this.palette_items(cx).len();
                if let Some(palette) = this.palette.as_mut() {
                    palette.index = (palette.index + 1).min(count.saturating_sub(1));
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &GoLighting, _, cx| this.navigate(Page::Lighting, cx)))
            .on_action(cx.listener(|this, _: &GoDisplay, _, cx| this.navigate(Page::Display, cx)))
            .on_action(cx.listener(|this, _: &GoKeys, _, cx| this.navigate(Page::Keys, cx)))
            .on_action(cx.listener(|this, _: &GoMacros, _, cx| this.navigate(Page::Macros, cx)))
            .on_action(cx.listener(|this, _: &GoSettings, _, cx| this.navigate(Page::Settings, cx)))
            .capture_key_down(cx.listener(Self::on_key_down))
            .capture_key_up(cx.listener(Self::on_key_up))
            .on_modifiers_changed(cx.listener(Self::on_modifiers))
            .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.choose_display_file(path.clone(), cx);
                    this.navigate(Page::Display, cx);
                }
            }))
            .when(client_side, |this| {
                this.child(
                    canvas(
                        |_bounds, window, _| {
                            window.insert_hitbox(
                                Bounds::new(point(px(0.), px(0.)), window.window_bounds().get_bounds().size),
                                HitboxBehavior::Normal,
                            )
                        },
                        |_bounds, hitbox, window, _| {
                            let size = window.window_bounds().get_bounds().size;
                            if let Some(edge) = resize_edge(window.mouse_position(), size) {
                                window.set_cursor_style(
                                    match edge {
                                        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                                        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
                                        ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                            CursorStyle::ResizeUpLeftDownRight
                                        }
                                        ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                            CursorStyle::ResizeUpRightDownLeft
                                        }
                                    },
                                    &hitbox,
                                );
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                )
                .on_mouse_move(|_, window, _| window.refresh())
                .on_mouse_down(MouseButton::Left, |event, window, cx| {
                    let size = window.window_bounds().get_bounds().size;
                    if let Some(edge) = resize_edge(event.position, size) {
                        window.start_window_resize(edge);
                        cx.stop_propagation();
                    }
                })
            })
            .child(self.render_sidebar(&t, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_topbar(&t, show_controls, window, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .mr(px(8.))
                            .mb(px(8.))
                            .rounded(px(12.))
                            .bg(t.surface)
                            .border_1()
                            .border_color(t.border)
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .when(offline, |this| {
                                this.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(10.))
                                        .px(px(18.))
                                        .py(px(10.))
                                        .border_b_1()
                                        .border_color(t.border)
                                        .child(icon(Icon::Warning, 15., t.muted))
                                        .child(widgets::hint(
                                            &t,
                                            self.device_error.clone().unwrap_or_else(|| {
                                                "Keyboard not detected. Connect the K8 with a USB cable and switch it to wired mode.".into()
                                            }),
                                        )),
                                )
                            })
                            .child(
                                div()
                                    .id("page-scroll")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .child(div().p(px(20.)).child(page)),
                            ),
                    ),
            )
            .when(self.drawer_open, |this| this.child(self.render_drawer(&t, cx)))
            .children(self.render_palette(&t, cx))
            .children(self.render_confirm(&t, cx))
            .children(self.render_preset_modal(&t, cx))
            .children(self.render_toast(&t))
    }
}

pub fn run() {
    Application::new()
        .with_assets(icons::Assets)
        .run(|cx: &mut App| {
            cx.set_global(Theme::for_appearance(cx.window_appearance()));
            input::bind_keys(cx);
            cx.bind_keys([
                KeyBinding::new("secondary-k", OpenPalette, Some("Studio")),
                KeyBinding::new("secondary-r", Refresh, Some("Studio")),
                KeyBinding::new("f5", Refresh, Some("Studio")),
                KeyBinding::new("escape", Escape, Some("Studio")),
                KeyBinding::new("up", PaletteUp, Some("Studio")),
                KeyBinding::new("down", PaletteDown, Some("Studio")),
                KeyBinding::new("secondary-1", GoLighting, Some("Studio")),
                KeyBinding::new("secondary-2", GoDisplay, Some("Studio")),
                KeyBinding::new("secondary-3", GoKeys, Some("Studio")),
                KeyBinding::new("secondary-4", GoMacros, Some("Studio")),
                KeyBinding::new("secondary-5", GoSettings, Some("Studio")),
                KeyBinding::new("secondary-q", Quit, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            let bounds = Bounds::centered(None, size(px(1240.), px(820.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Darmoshark K8 Studio".into()),
                            appears_transparent: true,
                            traffic_light_position: None,
                        }),
                        app_id: Some("darmoshark-k8".into()),
                        window_min_size: Some(size(px(960.), px(640.))),
                        window_decorations: Some(WindowDecorations::Client),
                        ..Default::default()
                    },
                    |window, cx| {
                        let view = cx.new(|cx| Studio::new(window, cx));
                        window.focus(&view.focus_handle(cx));
                        view
                    },
                )
                .expect("Could not open the K8 Studio window");
            window
                .update(cx, |_, _, cx| cx.activate(true))
                .ok();
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
        });
}
