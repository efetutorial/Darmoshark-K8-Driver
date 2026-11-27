//! Reusable, theme-aware building blocks.

use super::{icons::Icon, theme::Theme};
use gpui::{
    anchored, deferred, div, prelude::*, px, svg, AnyElement, App, ClickEvent, Corner, Div,
    ElementId, Hsla, MouseButton, SharedString, Stateful, Svg, Window,
};
use std::rc::Rc;

/// A child id derived from a parent id.
pub fn child_id(id: &ElementId, part: impl ToString) -> ElementId {
    ElementId::NamedChild(Box::new(id.clone()), part.to_string().into())
}

pub type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
pub type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

pub fn icon(icon: Icon, size: f32, color: Hsla) -> Svg {
    svg()
        .path(icon.path())
        .size(px(size))
        .flex_none()
        .text_color(color)
}

pub fn card(t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .p(px(18.))
        .rounded(px(12.))
        .bg(t.card)
        .border_1()
        .border_color(t.border)
}

pub fn card_title(t: &Theme, title: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(t.text)
        .child(title.into())
}

pub fn hint(t: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(17.))
        .text_color(t.muted)
        .child(text.into())
}

/// A label above a control.
pub fn field(t: &Theme, label: impl Into<SharedString>, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .w_full()
        .min_w_0()
        .child(
            div()
                .text_size(px(12.))
                .text_color(t.muted)
                .whitespace_nowrap()
                .child(label.into()),
        )
        .child(control)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Danger,
}

pub fn button(
    t: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: Kind,
    leading: Option<Icon>,
    disabled: bool,
) -> Stateful<Div> {
    let (bg, fg, border, hover) = match kind {
        Kind::Primary => (
            t.primary,
            t.primary_text,
            t.primary,
            t.primary.opacity(0.88),
        ),
        Kind::Secondary => (t.control, t.text, t.border, t.hover),
        Kind::Danger => (
            gpui::transparent_black(),
            t.danger,
            t.border,
            t.danger.opacity(0.1),
        ),
    };
    let hover_bg = if kind == Kind::Secondary {
        t.active
    } else {
        hover
    };
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(32.))
        .px(px(12.))
        .rounded(px(8.))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(px(13.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(fg)
        .when_some(leading, |this, leading| this.child(icon(leading, 15., fg)))
        .child(label.into())
        .when(disabled, |this| this.opacity(0.45))
        .when(!disabled, |this| {
            this.cursor_pointer().hover(move |style| style.bg(hover_bg))
        })
}

pub fn icon_button(t: &Theme, id: impl Into<ElementId>, glyph: Icon) -> Stateful<Div> {
    let hover = t.hover;
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(30.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(icon(glyph, 16., t.muted))
}

pub fn switch(t: &Theme, id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .w(px(34.))
        .h(px(20.))
        .p(px(2.))
        .rounded(px(10.))
        .cursor_pointer()
        .bg(if on { t.primary } else { t.active })
        .when(on, |this| this.justify_end())
        .child(
            div()
                .size(px(16.))
                .rounded(px(8.))
                .bg(if on { t.primary_text } else { t.surface }),
        )
}

/// A switch with a label, the whole row clickable.
pub fn switch_row(
    t: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    let id: ElementId = id.into();
    let hover = t.hover;
    div()
        .id(id.clone())
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .text_size(px(13.))
                .text_color(t.text)
                .child(label.into()),
        )
        .child(switch(t, child_id(&id, "switch"), on))
}

/// Pill-style segmented control.
pub fn segmented(
    t: &Theme,
    id: impl Into<ElementId>,
    items: Vec<SharedString>,
    selected: usize,
    on_select: IndexHandler,
) -> Div {
    let id: ElementId = id.into();
    div()
        .flex()
        .p(px(3.))
        .gap(px(2.))
        .rounded(px(9.))
        .bg(t.control)
        .border_1()
        .border_color(t.border)
        .children(items.into_iter().enumerate().map(|(index, label)| {
            let on_select = on_select.clone();
            let active = index == selected;
            let hover = t.hover;
            div()
                .id(child_id(&id, index))
                .flex_1()
                .flex()
                .justify_center()
                .px(px(10.))
                .py(px(4.))
                .rounded(px(6.))
                .text_size(px(12.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .when(active, |this| {
                    this.bg(t.surface)
                        .text_color(t.text)
                        .border_1()
                        .border_color(t.border)
                })
                .when(!active, |this| {
                    this.text_color(t.muted).hover(move |style| style.bg(hover))
                })
                .on_click(move |_, window, cx| on_select(index, window, cx))
                .child(label)
        }))
}

/// A discrete slider: one segment per value, click or drag to set.
pub fn steps(
    t: &Theme,
    id: impl Into<ElementId>,
    value: u8,
    max: u8,
    on_set: Rc<dyn Fn(u8, &mut Window, &mut App)>,
) -> Div {
    let id: ElementId = id.into();
    div()
        .flex()
        .gap(px(3.))
        .h(px(22.))
        .items_center()
        .children((0..=max).map(|step| {
            let on_click = on_set.clone();
            let on_drag = on_set.clone();
            let filled = step <= value;
            div()
                .id(child_id(&id, step))
                .flex_1()
                .h(px(if step == value { 14. } else { 8. }))
                .rounded(px(4.))
                .cursor_pointer()
                .bg(if filled { t.primary } else { t.active })
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    on_click(step, window, cx)
                })
                .on_mouse_move(move |event, window, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        on_drag(step, window, cx)
                    }
                })
        }))
}

/// The closed state of a dropdown.
pub fn select_button(
    t: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    open: bool,
) -> Stateful<Div> {
    let hover = t.hover;
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .w_full()
        .h(px(32.))
        .px(px(10.))
        .rounded(px(8.))
        .bg(t.control)
        .border_1()
        .border_color(if open { t.border_strong } else { t.border })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .text_size(px(13.))
        .text_color(t.text)
        .child(div().truncate().child(label.into()))
        .child(icon(Icon::ChevronDown, 14., t.muted))
}

/// A floating list anchored below its trigger. Render it as a child of the
/// trigger's container while open.
pub fn menu(
    t: &Theme,
    id: impl Into<ElementId>,
    items: Vec<(SharedString, bool)>,
    on_pick: IndexHandler,
    on_dismiss: Handler,
) -> AnyElement {
    let id: ElementId = id.into();
    let hover = t.hover;
    deferred(
        anchored().anchor(Corner::TopLeft).snap_to_window().child(
            div()
                .id(id.clone())
                .occlude()
                .mt(px(4.))
                .min_w(px(200.))
                .max_h(px(320.))
                .overflow_y_scroll()
                .p(px(4.))
                .rounded(px(10.))
                .bg(t.card)
                .border_1()
                .border_color(t.border_strong)
                .shadow_lg()
                .on_mouse_down_out(move |_, window, cx| on_dismiss(window, cx))
                .children(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(index, (label, selected))| {
                            let on_pick = on_pick.clone();
                            div()
                                .id(child_id(&id, index))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(12.))
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(6.))
                                .text_size(px(13.))
                                .text_color(t.text)
                                .cursor_pointer()
                                .hover(move |style| style.bg(hover))
                                .on_click(move |_: &ClickEvent, window, cx| {
                                    on_pick(index, window, cx)
                                })
                                .child(label)
                                .when(selected, |this| this.child(icon(Icon::Check, 14., t.muted)))
                        }),
                ),
        ),
    )
    .with_priority(1)
    .into_any_element()
}

pub fn badge(t: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(6.))
        .bg(t.control)
        .border_1()
        .border_color(t.border)
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(t.muted)
        .child(text.into())
}
