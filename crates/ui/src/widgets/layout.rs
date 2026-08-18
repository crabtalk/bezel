//! Organisation chrome — disclosure, collapsible header, split divider, tabs.
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.collapsible_header(..)`, `theme.split_handle(..)`, `theme.tab(..)`.

use gpui::{Div, SharedString, Svg, div, prelude::*, px};
use theme::{Theme, ThemeExt};

/// The drag payload of a [`Layout::split_handle`]. Shipped from here so every
/// split speaks the same type: `on_drag_move::<SplitDrag>` on one container
/// would otherwise fire for an unrelated split's gesture.
pub struct SplitDrag;

/// Width of the divider's grab strip: the 1px line plus zed's own 4px of slack
/// each side (`workspace::HANDLE_HITBOX_SIZE`) — a 1px target is unhittable.
const SPLIT_HANDLE_HIT: f32 = 9.0;

pub trait Layout: ThemeExt {
    /// The disclosure chevron: right when collapsed, down when expanded.
    ///
    /// Two assets rather than one rotated: gpui has no transform for `div`s at
    /// the pinned rev, and an SVG rotation would need a transform on the
    /// element.
    fn disclosure(&self, expanded: bool) -> Svg {
        let theme = self.theme();
        crate::icons::icon(if expanded {
            crate::icons::ALT_ARROW_DOWN
        } else {
            crate::icons::ALT_ARROW_RIGHT
        })
        .size(px(14.0))
        .text_color(theme.text_muted)
    }

    /// Header row of a collapsible section: chevron plus title. The caller owns
    /// `expanded` and renders the body itself — a container that swallowed its
    /// children would have to re-implement layout for them. Hover is
    /// caller-owned (gpui panics on a second hover); the default wash is
    /// [`collapsible_header_hover`](crate::widgets::collapsible_header_hover).
    fn collapsible_header(&self, label: impl Into<SharedString>, expanded: bool) -> Div {
        let theme = self.theme();
        div()
            .self_start()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(4.0))
            .py(px(5.0))
            .rounded(px(Theme::CONTROL_RADIUS))
            .cursor_pointer()
            .child(Layout::disclosure(self.theme(), expanded))
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(label.into()),
            )
    }

    /// The divider between two panes: a hairline centred in a grab strip, lit
    /// while dragged.
    ///
    /// The gesture stays with the caller, which owns the fraction — the handle
    /// is dragged with gpui's null-preview drag, and the container reads the
    /// pointer:
    ///
    /// ```ignore
    /// div()
    ///     .id("split")
    ///     .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SplitDrag>, _, cx| {
    ///         view.fraction = axis_fraction(event.event.position, event.bounds, Axis::Horizontal, 0.15);
    ///         cx.notify();
    ///     }))
    ///     .child(div().w(relative(self.fraction)).child(left))
    ///     .child(
    ///         theme.split_handle(Axis::Horizontal, self.dragging)
    ///             .id("split-handle")
    ///             .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| gpui::Empty)),
    ///     )
    ///     .child(div().flex_1().child(right))
    /// ```
    fn split_handle(&self, axis: gpui::Axis, dragging: bool) -> Div {
        let theme = self.theme();
        let line = if dragging { theme.caret } else { theme.border };
        let handle = div().flex_none().flex().items_center().justify_center();
        match axis {
            gpui::Axis::Horizontal => handle
                .w(px(SPLIT_HANDLE_HIT))
                .h_full()
                .cursor_col_resize()
                .child(div().w(px(1.0)).h_full().bg(line)),
            gpui::Axis::Vertical => handle
                .h(px(SPLIT_HANDLE_HIT))
                .w_full()
                .cursor_row_resize()
                .child(div().h(px(1.0)).w_full().bg(line)),
        }
    }

    /// Tab strip: a hairline-underlined row that tabs sit on.
    fn tab_bar(&self) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .border_b_1()
            .border_color(theme.border)
    }

    /// One tab. The active tab is marked by the text tone plus a 2px underline
    /// that overlaps the bar's hairline, so switching tabs never changes row
    /// height.
    fn tab(&self, label: impl Into<SharedString>, active: bool) -> Div {
        let theme = self.theme();
        div()
            .relative()
            .px(px(10.0))
            .pb(px(7.0))
            .pt(px(6.0))
            .rounded_t(px(Theme::CONTROL_RADIUS))
            .border_1()
            .border_color(crate::widgets::RING_SLOT)
            .text_size(px(13.0))
            .font_weight(if active {
                gpui::FontWeight::MEDIUM
            } else {
                gpui::FontWeight::NORMAL
            })
            .text_color(if active { theme.text } else { theme.text_muted })
            .cursor_pointer()
            .child(label.into())
            .when(active, |t| {
                t.child(
                    // Insets resolve against the padding box, so each one carries
                    // the ring slot's pixel: the underline still spans the tab's
                    // full width and still overlaps the bar's hairline.
                    div()
                        .absolute()
                        .bottom(px(-2.0))
                        .left(px(-1.0))
                        .right(px(-1.0))
                        .h(px(2.0))
                        .bg(theme.text),
                )
            })
    }
}

impl Layout for Theme {}
