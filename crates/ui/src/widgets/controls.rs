//! Display-only controls — toggle, checkbox, radio, progress, slider, color
//! well, select face, segmented control. State is always the caller's; each
//! control is the paint plus its gesture contract, and the caller adds
//! `.id(..)` / handlers — but for [`Controls::segmented`], which takes them.
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.toggle(..)`, `theme.slider(..)`, `theme.toggle_group()`,
//! `theme.segmented(..)`.

use crate::stack;
use std::rc::Rc;

use gpui::{
    App, Axis, Div, DragMoveEvent, ElementId, SharedString, Stateful, Window, div, prelude::*, px,
};
use icons::Icon;
use theme::{TextStyle, Theme, ThemeExt, Typeset};

/// The drag payload of a [`Controls::slider`], carrying the id of the slider
/// the gesture started on.
///
/// The id is what keeps sliders apart: gpui delivers a drag move to *every*
/// listener of the payload's type, not just the element under the pointer, so
/// a page of five sliders would move all five at once.
pub struct SliderDrag(pub ElementId);

/// Where a slider drag lands on the track it is asked about, or `None` when the
/// gesture belongs to another slider. `id` is the one that element carries.
pub fn slider_fraction(
    event: &DragMoveEvent<SliderDrag>,
    id: impl Into<ElementId>,
    cx: &App,
) -> Option<f32> {
    (event.drag(cx).0 == id.into()).then(|| {
        crate::widgets::axis_fraction(event.event.position, event.bounds, Axis::Horizontal, 0.0)
    })
}

pub trait Controls: ThemeExt {
    /// Display-only toggle switch (the reference branch-picker.tsx `Toggle`):
    /// an 18×32 pill whose knob slides right and track flips white when on.
    /// State is owned by the parent row — the caller adds `.id(..)` and
    /// `.on_click(..)`.
    fn toggle(&self, on: bool) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .w(px(32.0))
            .h(px(18.0))
            .rounded_full()
            .bg(if on { theme.text } else { theme.ink(0.15) })
            .border_1()
            .border_color(crate::widgets::RING_SLOT)
            .relative()
            .child(
                // One less than the 2px inset it looks like: absolute insets
                // resolve against the padding box, which the ring slot has
                // already moved in by a pixel.
                div()
                    .absolute()
                    .top(px(1.0))
                    .left(px(if on { 15.0 } else { 1.0 }))
                    .size(px(14.0))
                    .rounded_full()
                    .bg(if on { theme.on_solid } else { theme.ink(0.7) }),
            )
    }

    /// Display-only checkbox: a 16px rounded square that fills with the text
    /// tone and shows a check when on. State is the caller's; add
    /// `.id(..)`/`.on_click(..)`.
    fn checkbox(&self, checked: bool) -> Div {
        let theme = self.theme();
        let mut box_ = div()
            .flex_none()
            .size(px(16.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center();
        box_ = if checked {
            box_.border_1()
                .border_color(crate::widgets::RING_SLOT)
                .bg(theme.solid)
        } else {
            box_.border_1()
                .border_color(theme.ink(0.25))
                .bg(theme.ink(0.03))
        };
        if checked {
            box_.child(
                crate::icons::icon(crate::icons::glyph::Check)
                    .size(px(11.0))
                    .text_color(theme.on_solid),
            )
        } else {
            box_
        }
    }

    /// Display-only radio button: a 16px ring with an inner dot when selected.
    /// Radios are a *set* — the caller owns which index is on.
    fn radio_button(&self, selected: bool) -> Div {
        let theme = self.theme();
        div()
            .flex_none()
            .size(px(16.0))
            .rounded_full()
            .border_1()
            .border_color(if selected {
                theme.text
            } else {
                theme.ink(0.25)
            })
            .bg(theme.ink(0.03))
            .flex()
            .items_center()
            .justify_center()
            .when(selected, |ring| {
                ring.child(div().size(px(8.0)).rounded_full().bg(theme.solid))
            })
    }

    /// Determinate progress bar. `fraction` is clamped to `0..=1`; the track
    /// keeps its full width so the row never reflows as the value moves.
    fn progress_bar(&self, fraction: f32) -> Div {
        let theme = self.theme();
        let fraction = fraction.clamp(0.0, 1.0);
        div()
            .w_full()
            .h(px(4.0))
            .rounded_full()
            .bg(theme.ink(0.12))
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(fraction))
                    .rounded_full()
                    .bg(theme.solid),
            )
    }

    /// Display-only slider: filled track behind a knob at `fraction` (clamped
    /// to `0..=1`). Dragging is the caller's — it owns the value and the
    /// mouse handlers; this is the paint.
    ///
    /// The element *is* the drag source, so the gesture is
    /// grab-anywhere-and-slide, and [`slider_fraction`] turns the pointer into
    /// the value — passing the element's own id, because every slider hears
    /// every slider's drag:
    ///
    /// ```ignore
    /// focus::focusable(&theme, &self.slider, theme.slider(self.level))
    ///     .id("slider")
    ///     .on_drag(SliderDrag("slider".into()), |_, _, _, cx| cx.new(|_| gpui::Empty))
    ///     .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SliderDrag>, _, cx| {
    ///         let Some(fraction) = widgets::slider_fraction(event, "slider", cx) else {
    ///             return;
    ///         };
    ///         view.level = fraction;
    ///         cx.notify();
    ///     }))
    /// ```
    fn slider(&self, fraction: f32) -> Div {
        let theme = self.theme();
        let fraction = fraction.clamp(0.0, 1.0);
        div()
            .w_full()
            .h(px(16.0))
            .border_1()
            .border_color(crate::widgets::RING_SLOT)
            .rounded(px(4.0))
            .flex()
            .items_center()
            .relative()
            .cursor_pointer()
            .child(
                div()
                    .w_full()
                    .h(px(4.0))
                    .rounded_full()
                    .bg(theme.ink(0.12))
                    .child(
                        div()
                            .h_full()
                            .w(gpui::relative(fraction))
                            .rounded_full()
                            .bg(theme.solid),
                    ),
            )
            .child(
                // Inset by the knob's own width so it never overhangs the track.
                div().absolute().left(gpui::relative(fraction)).child(
                    div()
                        .size(px(14.0))
                        .ml(px(-7.0))
                        .rounded_full()
                        .bg(theme.solid),
                ),
            )
    }

    /// Display-only color well: a 20px ring holding `color`. The caller adds
    /// `.id(..)` and `.on_click(..)`.
    fn color_well(&self, color: gpui::Hsla) -> Div {
        well(self.theme().ink(0.25), color)
    }

    /// Preset colors, wired: a click on one reports its index to `on_pick`.
    /// The ring marks `selected`; none is marked when it is out of range or
    /// `None`. One row when `columns` is `None`; otherwise rows of `columns`.
    ///
    /// ```ignore
    /// let swatches = cx.color_swatches();
    /// theme.swatch_picker("tint", &swatches, self.tint, Some(6),
    ///     cx.listener(|view, ix: &usize, _, cx| view.pick_tint(*ix, cx)))
    /// ```
    fn swatch_picker(
        &self,
        id: impl Into<ElementId>,
        swatches: &[crate::color::Swatch],
        selected: Option<usize>,
        columns: Option<usize>,
        on_pick: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let theme = self.theme();
        let on_pick = Rc::new(on_pick);
        let wells: Vec<_> = swatches
            .iter()
            .enumerate()
            .map(|(ix, swatch)| {
                let on_pick = on_pick.clone();
                let ring = match selected == Some(ix) {
                    true => theme.text,
                    false => crate::widgets::RING_SLOT,
                };
                well(ring, swatch.resolve(theme))
                    .id(ix)
                    .on_click(move |_, window, cx| on_pick(&ix, window, cx))
            })
            .collect();
        let width = columns.filter(|n| *n > 0).unwrap_or(wells.len().max(1));
        let mut wells = wells.into_iter();
        let rows = std::iter::from_fn(|| {
            let row: Vec<_> = wells.by_ref().take(width).collect();
            (!row.is_empty()).then(|| stack::row().gap(px(4.0)).children(row))
        });
        stack::column().id(id).gap(px(4.0)).children(rows)
    }

    /// The face of a select: current value plus a chevron, shaped and toned like
    /// [`crate::input::TextField`] so a form of fields and selects reads as one
    /// system. One look, open or shut — the menu hanging under it is what says
    /// which it is.
    ///
    /// There is no `Select` component, deliberately — a select IS this trigger
    /// plus [`crate::popover::anchored_menu_below`] over
    /// [`crate::popover::menu_row`]s, and the caller already owns the open
    /// state and the selection. Wrapping that in a struct would buy an
    /// abstraction and cost the caller its control over both.
    fn select_trigger(&self, label: impl Into<SharedString>) -> Div {
        self.select_trigger_with(None::<Div>, label)
    }

    /// [`Self::select_trigger`] with `leading` drawn before the label — a
    /// swatch or a sample of the value.
    fn select_trigger_with(
        &self,
        leading: Option<impl IntoElement>,
        label: impl Into<SharedString>,
    ) -> Div {
        let theme = self.theme();
        stack::row()
            .justify_between()
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(Theme::button_radius()))
            .bg(theme.input_bg)
            .border_1()
            .border_color(theme.border)
            .text_style(TextStyle::Body)
            .text_color(theme.text)
            .cursor_pointer()
            .child(
                stack::row()
                    .min_w_0()
                    .gap(px(8.0))
                    .children(leading.map(|leading| div().flex_none().child(leading)))
                    .child(div().min_w_0().truncate().child(label.into())),
            )
            .child(
                crate::icons::icon(crate::icons::glyph::ChevronDown)
                    .size(px(14.0))
                    .text_color(theme.text_muted),
            )
    }

    /// Segmented control: one pill holding mutually exclusive choices, for when
    /// there are few enough that a [`Self::select_trigger`] would be overkill.
    ///
    /// `self_start` because a segmented control must hug its segments: dropped
    /// into a `flex_col`, flexbox's default `align-items: stretch` would
    /// otherwise blow it out to the column's full width.
    fn toggle_group(&self) -> Div {
        let theme = self.theme();
        div()
            .self_start()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(TOGGLE_GROUP_PAD))
            .p(px(TOGGLE_GROUP_PAD))
            .rounded(px(TOGGLE_GROUP_RADIUS))
            .bg(theme.ink(0.06))
            .border_1()
            .border_color(theme.border)
    }

    /// One segment. The selected segment carries the active wash over the
    /// track's own — the two alphas stack, which is what makes it read — and
    /// the rest are bare, so exactly one is pressed.
    fn toggle_group_item(&self, label: impl Into<SharedString>, selected: bool) -> Div {
        segment(self.theme(), selected)
            .px(px(10.0))
            .py(px(4.0))
            .child(label.into())
    }

    /// A [`Self::toggle_group`] of labelled segments, wired: a click on one
    /// reports its index to `on_pick`, the selected one included. Nothing is
    /// selected when `selected` is out of range.
    ///
    /// ```ignore
    /// theme.segmented("search-engine", ["Google", "DuckDuckGo"], selected,
    ///     cx.listener(|view, ix: &usize, _, cx| view.pick_engine(*ix, cx)))
    /// ```
    fn segmented<Label: Into<SharedString>>(
        &self,
        id: impl Into<ElementId>,
        segments: impl IntoIterator<Item = Label>,
        selected: usize,
        on_pick: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let on_pick = Rc::new(on_pick);
        self.toggle_group()
            .id(id)
            .children(segments.into_iter().enumerate().map(|(ix, label)| {
                let on_pick = on_pick.clone();
                self.toggle_group_item(label, ix == selected)
                    .id(ix)
                    .on_click(move |_, window, cx| on_pick(&ix, window, cx))
            }))
    }

    /// A segment carrying a glyph rather than a word, for a control with no
    /// room for one — a view switcher on a board, a density picker in a
    /// toolbar. It comes out 24×24, the height a labelled segment takes.
    ///
    /// The tooltip is the caller's, and a glyph nobody recognises says nothing
    /// without one.
    fn toggle_group_icon(&self, icon: impl Into<Icon>, selected: bool) -> Div {
        let theme = self.theme();
        // Built here rather than taken, the way `control_bar::bar_button` does
        // it: gpui reads an svg's colour off that element's own style, so a
        // colour set on the segment would not reach the glyph.
        segment(theme, selected)
            .p(px(5.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                crate::icons::icon(icon)
                    .size(px(14.0))
                    .text_color(match selected {
                        true => theme.text,
                        false => theme.text_muted,
                    }),
            )
    }
}

/// A 20px ring in `ring` around a disc of `color`.
fn well(ring: gpui::Hsla, color: gpui::Hsla) -> Div {
    div()
        .flex_none()
        .size(px(20.0))
        .p(px(2.0))
        .rounded_full()
        .border_1()
        .border_color(ring)
        .cursor_pointer()
        .child(div().size_full().rounded_full().bg(color))
}

/// What a segment looks like in each of its two states, before whatever it
/// carries. Padding is the caller's: a word and a glyph reach the same height
/// by different insets.
///
/// An unselected segment takes its own `hover`, and gpui panics on a second
/// one — reach for a `group_hover` rather than chaining `.hover(..)` on.
fn segment(theme: &Theme, selected: bool) -> Div {
    let wash = theme.element_hover;
    let item = div()
        // Concentric with the track: 9 - 2 = 7.
        .rounded(px(Theme::inset_radius(
            TOGGLE_GROUP_RADIUS,
            TOGGLE_GROUP_PAD,
        )))
        .border_1()
        .border_color(crate::widgets::RING_SLOT)
        .text_style(TextStyle::Callout)
        .cursor_pointer();
    if selected {
        item.bg(theme.element_active)
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text)
    } else {
        item.text_color(theme.text_muted)
            .hover(move |el| el.bg(wash))
    }
}

impl Controls for Theme {}

/// The segmented track's radius, and the inset its segments come in by. Two
/// numbers read from both [`Controls::toggle_group`] and
/// [`Controls::toggle_group_item`], so a segment cannot stop being concentric
/// with the track it sits in.
const TOGGLE_GROUP_RADIUS: f32 = 9.0;
const TOGGLE_GROUP_PAD: f32 = 2.0;
