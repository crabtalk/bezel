//! Display-only controls — toggle, checkbox, radio, progress, slider, select
//! face, segmented control. State is always the caller's; each control is the
//! paint plus its gesture contract, and the caller adds `.id(..)` / handlers.
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.toggle(..)`, `theme.slider(..)`, `theme.toggle_group()`.

use bezel_theme::{Theme, ThemeExt, ink};
use gpui::{Div, SharedString, div, prelude::*, px};

/// The drag payload of a [`Controls::slider`], shipped from here for the same
/// reason [`crate::widgets::SplitDrag`] is: one type per gesture, so two
/// sliders in one window do not answer each other's `on_drag_move`.
pub struct SliderDrag;

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
            .bg(if on { theme.text } else { ink(0.15) })
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
                    .bg(if on { theme.on_solid } else { ink(0.7) }),
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
                .bg(theme.text)
        } else {
            box_.border_1().border_color(ink(0.25)).bg(ink(0.03))
        };
        if checked {
            box_.child(
                crate::icons::icon(crate::icons::CHECK)
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
            .border_color(if selected { theme.text } else { ink(0.25) })
            .bg(ink(0.03))
            .flex()
            .items_center()
            .justify_center()
            .when(selected, |ring| {
                ring.child(div().size(px(8.0)).rounded_full().bg(theme.text))
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
            .bg(ink(0.12))
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(fraction))
                    .rounded_full()
                    .bg(theme.text),
            )
    }

    /// Display-only slider: filled track behind a knob at `fraction` (clamped
    /// to `0..=1`). Dragging is the caller's — it owns the value and the
    /// mouse handlers; this is the paint.
    ///
    /// The element *is* the drag source, so the gesture is
    /// grab-anywhere-and-slide, and [`axis_fraction`](crate::widgets::axis_fraction)
    /// turns the pointer into the value:
    ///
    /// ```ignore
    /// focus::focusable(&theme, &self.slider, theme.slider(self.level))
    ///     .id("slider")
    ///     .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
    ///     .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SliderDrag>, _, cx| {
    ///         view.level = axis_fraction(event.event.position, event.bounds, Axis::Horizontal, 0.0);
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
            .child(
                div()
                    .w_full()
                    .h(px(4.0))
                    .rounded_full()
                    .bg(ink(0.12))
                    .child(
                        div()
                            .h_full()
                            .w(gpui::relative(fraction))
                            .rounded_full()
                            .bg(theme.text),
                    ),
            )
            .child(
                // Inset by the knob's own width so it never overhangs the track.
                div().absolute().left(gpui::relative(fraction)).child(
                    div()
                        .size(px(14.0))
                        .ml(px(-7.0))
                        .rounded_full()
                        .bg(theme.text),
                ),
            )
    }

    /// The closed face of a select: current value plus a chevron, shaped and
    /// toned like [`crate::input::TextField`] so a form of fields and selects
    /// reads as one system.
    ///
    /// There is no `Select` component, deliberately — a select IS this trigger
    /// plus [`crate::popover::anchored_menu_below`] over
    /// [`crate::popover::menu_row`]s, and the caller already owns the open
    /// state and the selection. Wrapping that in a struct would buy an
    /// abstraction and cost the caller its control over both.
    fn select_trigger(&self, label: impl Into<SharedString>, open: bool) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(Theme::BUTTON_RADIUS))
            .bg(theme.input_bg)
            .border_1()
            .border_color(if open { theme.caret } else { theme.border })
            .text_size(px(13.0))
            .text_color(theme.text)
            .cursor_pointer()
            .child(div().min_w_0().truncate().child(label.into()))
            .child(
                crate::icons::icon(crate::icons::ALT_ARROW_DOWN)
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
            .bg(ink(0.06))
            .border_1()
            .border_color(theme.border)
    }

    /// One segment. The selected segment carries a raised plate; the rest are
    /// bare, so exactly one reads as pressed.
    fn toggle_group_item(&self, label: impl Into<SharedString>, selected: bool) -> Div {
        let theme = self.theme();
        let mut item = div()
            .px(px(10.0))
            .py(px(4.0))
            // Concentric with the track: 9 - 2 = 7.
            .rounded(px(Theme::inset_radius(
                TOGGLE_GROUP_RADIUS,
                TOGGLE_GROUP_PAD,
            )))
            .border_1()
            .border_color(crate::widgets::RING_SLOT)
            .text_size(px(12.5))
            .cursor_pointer()
            .child(label.into());
        item = if selected {
            item.bg(theme.surface_raised)
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text)
        } else {
            item.text_color(theme.text_muted)
        };
        item
    }
}

impl Controls for Theme {}

/// The segmented track's radius, and the inset its segments come in by. Two
/// numbers read from both [`Controls::toggle_group`] and
/// [`Controls::toggle_group_item`], so a segment cannot stop being concentric
/// with the track it sits in.
const TOGGLE_GROUP_RADIUS: f32 = 9.0;
const TOGGLE_GROUP_PAD: f32 = 2.0;
