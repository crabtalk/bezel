//! Shared scaffolding for the settings pages — the original's page rhythm
//! (`mx-auto max-w-3xl px-6 pb-16 pt-8`), section cards, row layout, badges
//! and small buttons, so every page reads as the same product surface
//! (the reference settings.devices.tsx / settings.agents.tsx / settings.archived.tsx).

use gpui::{AnyElement, SharedString, div, prelude::*, px};

use bezel_theme::{Theme, ink};

/// What a control paints in the 1px border it keeps for
/// [`crate::focus::focusable`]'s ring: nothing, until focus fills it.
///
/// Always present, never conditional. gpui sizes border-box, so a border that
/// appeared only on focus would shift the content under it by a pixel — a
/// checkbox whose tick jumps as you tab onto it.
pub(crate) const RING_SLOT: gpui::Hsla = gpui::transparent_black();

/// Centered page column: `mx-auto w-full max-w-3xl px-6 pb-16 pt-8`.
pub fn page_column() -> gpui::Div {
    div()
        .w_full()
        .max_w(px(768.0))
        .mx_auto()
        .px(px(24.0))
        .pt(px(32.0))
        .pb(px(64.0))
        .flex()
        .flex_col()
}

/// Page headline row: `flex items-baseline gap-2.5` — `text-base font-semibold`
/// title + `text-[13px]` count sharing a baseline (the reference settings.devices.tsx).
pub fn page_header(theme: &Theme, title: &str, count: Option<usize>) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text)
                .child(SharedString::from(title.to_string())),
        )
        .when_some(count, |el, count| {
            el.child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme.text_muted.opacity(0.7))
                    .child(SharedString::from(format!("{count}"))),
            )
        })
}

/// Subtitle under the headline: `mt-1 text-[13px] text-muted-foreground`.
pub fn page_subtitle(theme: &Theme, copy: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(px(4.0))
        .text_size(px(13.0))
        .text_color(theme.text_muted)
        .child(copy.into())
}

/// Small label above a group of controls (`text-[13px] font-medium`) — the
/// "Theme" caption over a picker, not a page headline.
pub fn field_label(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(13.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.text)
        .child(label.into())
}

/// A row of equally-sized preview cards for picking one of N *visual* options.
///
/// Deliberately knows nothing about themes: the caller supplies each preview as
/// an arbitrary element and picks however many cards it wants, so the same
/// control works for a density picker, a layout picker or anything else where
/// the choice is easier to show than to describe. Pair with [`option_card`].
pub fn option_card_row() -> gpui::Div {
    div().flex().flex_row().items_start().gap(px(16.0)).w_full()
}

/// Default height of an [`option_card`] preview frame.
pub const OPTION_CARD_HEIGHT: f32 = 148.0;
/// Corner radius of the preview frame.
///
/// Public because the preview has to round *itself* to this. gpui content masks
/// are axis-aligned rectangles, so `overflow_hidden` on the frame clips to its
/// bounding box and not to its corner radius — a preview that paints its own
/// background will square off the corners and cover the frame's border with it.
pub const OPTION_CARD_RADIUS: f32 = 10.0;
/// Clear space between the frame and the selection ring.
const RING_GAP: f32 = 2.0;
/// Thickness of the selection ring.
const RING_WIDTH: f32 = 2.0;

/// One card in an [`option_card_row`]: a fixed-height preview frame that carries
/// the selection ring, with a caption underneath.
///
/// `preview` fills the frame and **must round its own corners** to
/// [`OPTION_CARD_RADIUS`] if it paints a background — see that constant.
///
/// Returns a plain `Div` like the rest of this module — the caller adds `.id(..)`
/// and `.on_click(..)`, so selection behaviour stays with the page that owns the
/// state.
pub fn option_card(
    theme: &Theme,
    label: impl Into<SharedString>,
    selected: bool,
    preview: AnyElement,
) -> gpui::Div {
    let frame = div()
        .h(px(OPTION_CARD_HEIGHT))
        .w_full()
        .rounded(px(OPTION_CARD_RADIUS))
        .overflow_hidden()
        .border_1()
        .border_color(theme.border)
        .child(preview);

    // The ring is a *wrapper border*, not a spread shadow. A shadow's spread
    // grows the rectangle without growing its corner radius, so the halo's
    // corners tighten relative to the frame's and the two visibly drift apart by
    // a pixel at each rounded corner. Concentric borders can't do that: each
    // element rounds itself, and the outer radius is the inner one plus the gap
    // it sits behind. Always present, transparent when unselected, so selecting a
    // card never reflows the row.
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .cursor_pointer()
        .child(
            div()
                .w_full()
                .rounded(px(OPTION_CARD_RADIUS + RING_GAP + RING_WIDTH))
                .p(px(RING_GAP))
                .border_2()
                .border_color(if selected {
                    theme.accent
                } else {
                    gpui::transparent_black()
                })
                .child(frame),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(if selected {
                    theme.text
                } else {
                    theme.text_muted
                })
                .child(label.into()),
        )
}

/// Section card: `mt-6 overflow-hidden rounded-xl border border-border bg-card`
/// — the card tone, thinned to a translucent tint over glass so the card
/// reads as frost instead of a solid slab ([`Theme::card_glass_bg`]).
pub fn group_box(theme: &Theme) -> gpui::Div {
    div()
        .mt(px(24.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.card_glass_bg())
        .overflow_hidden()
        .flex()
        .flex_col()
}

/// One card row: `border-t border-border px-5 py-3.5 first:border-t-0` with the
/// quiet hover wash.
pub fn card_row(theme: &Theme, first: bool) -> gpui::Div {
    div()
        .px(px(20.0))
        .py(px(14.0))
        .when(!first, |el| el.border_t_1().border_color(theme.border))
        .hover(|s| s.bg(ink(0.015)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(14.0))
}

/// The identity tile on a row: `size-9 rounded-[10px] border bg-white/[0.03]`
/// around a 16px icon.
pub fn row_tile(theme: &Theme, icon_path: &'static str) -> gpui::Div {
    div()
        .flex_none()
        .size(px(36.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(theme.border)
        .bg(ink(0.03))
        .flex()
        .items_center()
        .justify_center()
        .child(
            crate::icons::icon(icon_path)
                .size(px(16.0))
                .text_color(theme.text_muted),
        )
}

/// Row title: `text-[13.5px] font-medium leading-tight`.
pub fn row_title(theme: &Theme, title: impl Into<SharedString>) -> gpui::Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(px(13.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.text)
        .child(title.into())
}

/// The quiet meta line under a row title: `text-[11.5px]
/// text-muted-foreground/65` fragments joined by dots.
pub fn meta_line(theme: &Theme, fragments: Vec<AnyElement>) -> gpui::Div {
    let mut line = div()
        .mt(px(4.0))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(px(8.0))
        .gap_y(px(2.0))
        .text_size(px(11.5))
        .text_color(theme.text_muted.opacity(0.65));
    let mut first = true;
    for fragment in fragments {
        if !first {
            line = line.child(
                div()
                    .text_color(theme.text_muted.opacity(0.3))
                    .child(SharedString::from("·")),
            );
        }
        line = line.child(fragment);
        first = false;
    }
    line
}

/// Right-anchored badge pill: `rounded-full border px-2 py-0.5 text-[10.5px]`.
pub fn badge(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex_none()
        .px(px(8.0))
        .py(px(2.0))
        .rounded_full()
        .border_1()
        .border_color(theme.border)
        .text_size(px(10.5))
        .text_color(theme.text_muted)
        .child(label.into())
}

/// Emerald status pill (the Accounts "Active" badge:
/// `bg-emerald-400/[0.12] text-emerald-300/90`).
pub fn badge_active(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    let emerald = theme.success;
    let emerald_text = theme.success_muted; // emerald-300
    div()
        .flex_none()
        .px(px(8.0))
        .py(px(2.0))
        .rounded_full()
        .bg(emerald.opacity(0.12))
        .text_size(px(10.5))
        .text_color(emerald_text.opacity(0.9))
        .child(label.into())
}

/// Display-only toggle switch (the reference branch-picker.tsx `Toggle`): an 18×32
/// pill whose knob slides right and track flips white when on. State is owned
/// by the parent row — the caller adds `.id(..)` and `.on_click(..)`.
pub fn toggle(theme: &Theme, on: bool) -> gpui::Div {
    div()
        .flex_none()
        .w(px(32.0))
        .h(px(18.0))
        .rounded_full()
        .bg(if on { theme.text } else { ink(0.15) })
        .border_1()
        .border_color(RING_SLOT)
        .relative()
        .child(
            // One less than the 2px inset it looks like: absolute insets resolve
            // against the padding box, which the ring slot has already moved in
            // by a pixel.
            div()
                .absolute()
                .top(px(1.0))
                .left(px(if on { 15.0 } else { 1.0 }))
                .size(px(14.0))
                .rounded_full()
                .bg(if on { theme.on_solid } else { ink(0.7) }),
        )
}

/// Display-only checkbox: a 16px rounded square that fills with the text tone
/// and shows a check when on. State is the caller's; add `.id(..)`/`.on_click(..)`.
pub fn checkbox(theme: &Theme, checked: bool) -> gpui::Div {
    let mut box_ = div()
        .flex_none()
        .size(px(16.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center();
    box_ = if checked {
        box_.border_1().border_color(RING_SLOT).bg(theme.text)
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
pub fn radio_button(theme: &Theme, selected: bool) -> gpui::Div {
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

/// Determinate progress bar. `fraction` is clamped to `0..=1`; the track keeps
/// its full width so the row never reflows as the value moves.
pub fn progress_bar(theme: &Theme, fraction: f32) -> gpui::Div {
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

/// The drag payload of a [`slider`], shipped from here for the same reason
/// [`SplitDrag`] is: one type per gesture, so two sliders in one window do not
/// answer each other's `on_drag_move`.
pub struct SliderDrag;

/// Display-only slider: filled track behind a knob at `fraction` (clamped to
/// `0..=1`). Dragging is the caller's — it owns the value and the mouse
/// handlers; this is the paint.
///
/// The element *is* the drag source, so the gesture is grab-anywhere-and-slide,
/// and [`axis_fraction`] turns the pointer into the value:
///
/// ```ignore
/// focus::focusable(&theme, &self.slider, slider(&theme, self.level))
///     .id("slider")
///     .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
///     .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SliderDrag>, _, cx| {
///         view.level = axis_fraction(event.event.position, event.bounds, Axis::Horizontal, 0.0);
///         cx.notify();
///     }))
/// ```
pub fn slider(theme: &Theme, fraction: f32) -> gpui::Div {
    let fraction = fraction.clamp(0.0, 1.0);
    div()
        .w_full()
        .h(px(16.0))
        .border_1()
        .border_color(RING_SLOT)
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

/// Circular avatar holding one or two initials — the fallback every avatar
/// needs, and often the whole thing on a monochrome surface.
pub fn avatar(theme: &Theme, initials: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex_none()
        .size(px(28.0))
        .rounded_full()
        .bg(ink(0.12))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.text_muted)
        .child(initials.into())
}

/// The closed face of a select: current value plus a chevron, shaped and
/// toned like [`crate::input::TextField`] so a form of fields and selects
/// reads as one system.
///
/// There is no `Select` component, deliberately — a select IS this trigger
/// plus [`crate::popover::anchored_menu_below`] over [`crate::popover::menu_row`]s,
/// and the caller already owns the open state and the selection. Wrapping
/// that in a struct would buy an abstraction and cost the caller its control
/// over both.
pub fn select_trigger(theme: &Theme, label: impl Into<SharedString>, open: bool) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded(px(8.0))
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
/// there are few enough that a [`select_trigger`] would be overkill.
///
/// `self_start` because a segmented control must hug its segments: dropped into
/// a `flex_col`, flexbox's default `align-items: stretch` would otherwise blow
/// it out to the column's full width.
pub fn toggle_group(theme: &Theme) -> gpui::Div {
    div()
        .self_start()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded(px(9.0))
        .bg(ink(0.06))
        .border_1()
        .border_color(theme.border)
}

/// One segment. The selected segment carries a raised plate; the rest are
/// bare, so exactly one reads as pressed.
pub fn toggle_group_item(
    theme: &Theme,
    label: impl Into<SharedString>,
    selected: bool,
) -> gpui::Div {
    let mut item = div()
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(RING_SLOT)
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

/// The disclosure chevron: right when collapsed, down when expanded.
///
/// Two assets rather than one rotated: gpui has no transform for `div`s at the
/// pinned rev, and an SVG rotation would need a transform on the element.
pub fn disclosure(theme: &Theme, expanded: bool) -> gpui::Svg {
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
/// children would have to re-implement layout for them.
pub fn collapsible_header(
    theme: &Theme,
    label: impl Into<SharedString>,
    expanded: bool,
) -> gpui::Div {
    div()
        .self_start()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(4.0))
        .py(px(5.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(ink(0.03)))
        .child(disclosure(theme, expanded))
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(label.into()),
        )
}

// ---------------------------------------------------------------------------
// Resizable split
// ---------------------------------------------------------------------------

/// The drag payload of a [`split_handle`]. Shipped from here so every split
/// speaks the same type: `on_drag_move::<SplitDrag>` on one container would
/// otherwise fire for an unrelated split's gesture.
pub struct SplitDrag;

/// Width of the divider's grab strip: the 1px line plus zed's own 4px of slack
/// each side (`workspace::HANDLE_HITBOX_SIZE`) — a 1px target is unhittable.
const SPLIT_HANDLE_HIT: f32 = 9.0;

/// Where `pointer` falls along `axis` as a fraction of `bounds` — what a
/// divider dragged there makes the split, and what a slider dragged there makes
/// the value. `Axis::Horizontal` travels in x.
///
/// Clamped to `min..=1-min` — the dead zone a split passes so neither pane can
/// be squeezed away, and the `0.0` a slider passes because it has none. On a
/// zero-extent container the answer is `min`: the frame before layout has run
/// would otherwise divide by zero.
pub fn axis_fraction(
    pointer: gpui::Point<gpui::Pixels>,
    bounds: gpui::Bounds<gpui::Pixels>,
    axis: gpui::Axis,
    min: f32,
) -> f32 {
    let min = min.clamp(0.0, 0.5);
    let (offset, extent) = match axis {
        gpui::Axis::Horizontal => (pointer.x - bounds.left(), bounds.size.width),
        gpui::Axis::Vertical => (pointer.y - bounds.top(), bounds.size.height),
    };
    if extent <= px(0.0) {
        return min;
    }
    (offset / extent).clamp(min, 1.0 - min)
}

/// The divider between two panes: a hairline centred in a grab strip, lit while
/// dragged.
///
/// The gesture stays with the caller, which owns the fraction — the handle is
/// dragged with gpui's null-preview drag, and the container reads the pointer:
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
///         split_handle(&theme, Axis::Horizontal, self.dragging)
///             .id("split-handle")
///             .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| gpui::Empty)),
///     )
///     .child(div().flex_1().child(right))
/// ```
pub fn split_handle(theme: &Theme, axis: gpui::Axis, dragging: bool) -> gpui::Div {
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

/// A removable chip — a token in a filter bar or a recipient field. The caller
/// adds the click handler for the ✕.
pub fn tag(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .self_start()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .pl(px(8.0))
        .pr(px(5.0))
        .py(px(3.0))
        .rounded(px(6.0))
        .bg(ink(0.07))
        .border_1()
        .border_color(theme.border)
        .text_size(px(12.0))
        .text_color(theme.text)
        .child(label.into())
        .child(
            crate::icons::icon(crate::icons::CLOSE)
                .size(px(10.0))
                .text_color(theme.text_faint),
        )
}

/// Breadcrumb trail. Items are added by the caller with [`breadcrumb_item`],
/// separated by [`breadcrumb_separator`].
pub fn breadcrumb() -> gpui::Div {
    div()
        .self_start()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .min_w_0()
}

/// One crumb. The last one is `current` and stops looking clickable.
pub fn breadcrumb_item(theme: &Theme, label: impl Into<SharedString>, current: bool) -> gpui::Div {
    let mut crumb = div()
        .min_w_0()
        .truncate()
        .text_size(px(12.5))
        .child(label.into());
    crumb = if current {
        crumb.text_color(theme.text)
    } else {
        crumb.text_color(theme.text_muted).cursor_pointer()
    };
    crumb
}

/// The chevron between crumbs.
pub fn breadcrumb_separator(theme: &Theme) -> gpui::Svg {
    crate::icons::icon(crate::icons::ALT_ARROW_RIGHT)
        .size(px(12.0))
        .text_color(theme.text_faint)
}

/// A small state dot — the "working / idle / failed" bead on a row. Takes the
/// tone from the caller so the meaning stays with the caller's domain.
pub fn status_dot(tone: gpui::Hsla) -> gpui::Div {
    div().flex_none().size(px(6.0)).rounded_full().bg(tone)
}

/// The centered "nothing here yet" panel: icon, headline, one line of hint.
pub fn empty_state(
    theme: &Theme,
    icon_path: &'static str,
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
) -> gpui::Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .py(px(40.0))
        .child(
            crate::icons::icon(icon_path)
                .size(px(24.0))
                .text_color(theme.text_faint),
        )
        .child(
            div()
                .text_size(px(13.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(title.into()),
        )
        .child(
            div()
                .text_size(px(12.5))
                .text_color(theme.text_muted)
                .child(hint.into()),
        )
}

/// Tab strip: a hairline-underlined row that tabs sit on.
pub fn tab_bar(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .border_b_1()
        .border_color(theme.border)
}

/// One tab. The active tab is marked by the text tone plus a 2px underline that
/// overlaps the bar's hairline, so switching tabs never changes row height.
pub fn tab(theme: &Theme, label: impl Into<SharedString>, active: bool) -> gpui::Div {
    div()
        .relative()
        .px(px(10.0))
        .pb(px(7.0))
        .pt(px(6.0))
        .rounded_t(px(6.0))
        .border_1()
        .border_color(RING_SLOT)
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

/// A small quiet ghost action (`rounded-lg px-2.5 py-1.5 text-[12px]
/// text-muted-foreground`). Caller adds id + click + leading icon child AND
/// its own `.hover(..)` — gpui panics on a second hover, and the pages vary
/// it (reveal opacity, 4% vs 6% washes).
pub fn ghost_action(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .rounded(px(8.0))
        .px(px(10.0))
        .py(px(6.0))
        .text_size(px(12.0))
        .text_color(theme.text_muted)
        .cursor_pointer()
}

/// The default ghost-action hover wash (`hover:bg-white/[0.06]
/// hover:text-foreground`).
pub fn ghost_hover(theme: &Theme, s: gpui::StyleRefinement) -> gpui::StyleRefinement {
    s.bg(ink(0.06)).text_color(theme.text)
}

/// The dismissible red error strip (`flex items-start gap-2 rounded-xl border
/// border-red-400/20 bg-red-400/[0.06] text-red-300/90` with a leading
/// `DangerTriangle mt-0.5 size-4`).
pub fn error_strip(theme: &Theme, message: impl Into<SharedString>) -> gpui::Div {
    let red = theme.danger; // red-400
    let red_text = theme.danger_muted; // red-300
    div()
        .mt(px(16.0))
        .px(px(16.0))
        .py(px(12.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(red.opacity(0.2))
        .bg(red.opacity(0.06))
        .text_size(px(12.5))
        .text_color(red_text.opacity(0.9))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.0))
        .child(
            div().flex_none().mt(px(2.0)).child(
                crate::icons::icon(crate::icons::DANGER_TRIANGLE)
                    .size(px(16.0))
                    .text_color(red_text.opacity(0.9)),
            ),
        )
        .child(div().min_w_0().child(message.into()))
}

/// The amber warning strip (`flex items-start gap-2 border-amber-400/20
/// bg-amber-400/[0.06] text-amber-200/90` with a leading `DangerTriangle
/// mt-0.5 size-3.5`).
pub fn warning_strip(theme: &Theme, message: impl Into<SharedString>) -> gpui::Div {
    let amber = theme.warning; // amber-400
    let amber_text = theme.warning_muted; // amber-200
    div()
        .mt(px(8.0))
        .px(px(16.0))
        .py(px(10.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(amber.opacity(0.2))
        .bg(amber.opacity(0.06))
        .text_size(px(12.0))
        .text_color(amber_text.opacity(0.9))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.0))
        .child(
            div().flex_none().mt(px(2.0)).child(
                crate::icons::icon(crate::icons::DANGER_TRIANGLE)
                    .size(px(14.0))
                    .text_color(amber_text.opacity(0.9)),
            ),
        )
        .child(div().min_w_0().child(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Axis, Bounds, point, size};

    fn box_at(left: f32, top: f32, width: f32, height: f32) -> Bounds<gpui::Pixels> {
        Bounds::new(point(px(left), px(top)), size(px(width), px(height)))
    }

    #[test]
    fn axis_fraction_measures_from_the_container_origin() {
        // A container that does not start at the window origin: the fraction is
        // of the container, not of the pointer's absolute position.
        let bounds = box_at(100.0, 40.0, 400.0, 200.0);
        assert_eq!(
            axis_fraction(point(px(300.0), px(60.0)), bounds, Axis::Horizontal, 0.0),
            0.5
        );
        assert_eq!(
            axis_fraction(point(px(200.0), px(60.0)), bounds, Axis::Horizontal, 0.0),
            0.25
        );
        // Vertical splits read y against the height.
        assert_eq!(
            axis_fraction(point(px(300.0), px(90.0)), bounds, Axis::Vertical, 0.0),
            0.25
        );
    }

    #[test]
    fn axis_fraction_never_squeezes_a_pane_away() {
        let bounds = box_at(0.0, 0.0, 400.0, 200.0);
        // Dragged past either end, and even outside the container entirely.
        assert_eq!(
            axis_fraction(point(px(-500.0), px(0.0)), bounds, Axis::Horizontal, 0.2),
            0.2
        );
        assert_eq!(
            axis_fraction(point(px(900.0), px(0.0)), bounds, Axis::Horizontal, 0.2),
            0.8
        );
        // A nonsense minimum still leaves both panes on screen.
        assert_eq!(
            axis_fraction(point(px(0.0), px(0.0)), bounds, Axis::Horizontal, 4.0),
            0.5
        );
    }

    #[test]
    fn axis_fraction_survives_a_container_with_no_extent() {
        // The frame before layout has run — no divide by zero, no NaN.
        let empty = box_at(0.0, 0.0, 0.0, 0.0);
        assert_eq!(
            axis_fraction(point(px(10.0), px(10.0)), empty, Axis::Horizontal, 0.15),
            0.15
        );
    }
}
