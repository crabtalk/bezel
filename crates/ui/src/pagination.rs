//! Pagination — for data that arrives in pages, not for lists that are long.
//!
//! Worth saying plainly, because the distinction is the whole reason this
//! module is small and late: a long list is answered by [`crate::scroll`] and
//! [`crate::list`], which will show ten thousand rows and build nine of them. A
//! paginator earns its place only when the *data* is paged and the client
//! cannot hold the whole set — an API that answers "page 4 of 87", a report with
//! a fixed page size, a backend that will not stream. There the page number is
//! not a scrolling affordance, it is the query.
//!
//! What it contributes is one function. [`window`] turns `(current, total)` into
//! the row you see, and it is the part that is fiddly rather than obvious:
//!
//! ```text
//! current = 6, total = 20   →   1 … 4 5 [6] 7 8 … 20
//! current = 2, total = 20   →   1 [2] 3 4 5 … 20
//! current = 3, total = 5    →   1 2 [3] 4 5
//! ```
//!
//! Which page you are on, how many there are, and how to fetch one are all the
//! caller's — as with [`crate::table`]'s sort, this module reports and paints.

use gpui::{SharedString, div, prelude::*, px};

use theme::{Theme, ink};

use crate::{icons, widgets};

/// A place in the row: a page you can go to, or the mark for pages skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Page(usize),
    Gap,
}

/// The pages to show for `current` of `total`, keeping `around` either side.
///
/// Pages are **1-based** here, unlike the indices everywhere else in this
/// crate: a page number is a label a person reads, not an offset into a slice,
/// and a paginator that can say "page 0" is a bug waiting to be filed. `current`
/// out of range is clamped rather than trusted — it arrives from a caller's
/// state, and a paint is no place to panic.
///
/// Two rules earn their tests. A gap that hides exactly **one** page is worse
/// than the page, so that page is shown instead; an ellipsis standing for a
/// single number tells you less while taking the same room. And the window
/// **slides** at the ends rather than shrinking, so walking to the last page
/// does not narrow the control under the pointer — the same refusal to reflow as
/// the focus ring's reserved border and the calendar's six fixed rows.
pub fn window(current: usize, total: usize, around: usize) -> Vec<Slot> {
    if total == 0 {
        return Vec::new();
    }
    let current = current.clamp(1, total);
    let width = (2 * around + 1).min(total);
    // The furthest left the window can start and still hold its width.
    let last_start = total - width + 1;
    let start = current.saturating_sub(around).clamp(1, last_start);
    let end = start + width - 1;

    let mut slots = Vec::with_capacity(width + 4);
    if start > 1 {
        slots.push(Slot::Page(1));
        match start - 1 {
            // Page 1 is the window's left neighbour: nothing is skipped.
            1 => {}
            // Exactly one page between: show it rather than hide it.
            2 => slots.push(Slot::Page(2)),
            _ => slots.push(Slot::Gap),
        }
    }
    slots.extend((start..=end).map(Slot::Page));
    if end < total {
        match total - end {
            1 => {}
            2 => slots.push(Slot::Page(total - 1)),
            _ => slots.push(Slot::Gap),
        }
        slots.push(Slot::Page(total));
    }
    slots
}

/// Side of a page button, and of the steps either side of the row.
const BUTTON: f32 = 28.0;

/// The row. Fill it with [`page_button`]s, [`ellipsis`]es and [`step`]s.
pub fn pagination() -> gpui::Div {
    div()
        .self_start()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
}

/// One page. The caller adds `.id`/`.on_click`: a paginator that owned its
/// clicks would have to own which page you are on, which is the caller's whole
/// reason for having one.
pub fn page_button(theme: &Theme, page: usize, current: bool) -> gpui::Div {
    let button = div()
        .min_w(px(BUTTON))
        .h(px(BUTTON))
        .px(px(6.0))
        .rounded(px(7.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.5))
        // The ring slot, like every other control: focus has somewhere to land
        // and nothing moves when it does.
        .border_1()
        .border_color(widgets::RING_SLOT)
        .cursor_pointer()
        .child(SharedString::from(page.to_string()));
    if current {
        button
            .bg(theme.accent)
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.on_accent)
    } else {
        button
            .text_color(theme.text_muted)
            .hover(|s| s.bg(ink(0.06)).text_color(theme.text))
    }
}

/// The mark for skipped pages. Inert on purpose — it is a statement about the
/// row, not somewhere to go, so it takes no hover and no pointer cursor.
pub fn ellipsis(theme: &Theme) -> gpui::Div {
    div()
        .w(px(BUTTON))
        .h(px(BUTTON))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.5))
        .text_color(theme.text_faint)
        .child(SharedString::from("…"))
}

/// Previous or next, with [`icons::ALT_ARROW_LEFT`]/[`icons::ALT_ARROW_RIGHT`]
/// — the pair the calendar's month header already uses.
///
/// A disabled step stays in place rather than disappearing at the ends, so the
/// row does not shuffle sideways on the first and last pages.
pub fn step(theme: &Theme, icon: &'static str, enabled: bool) -> gpui::Div {
    let step = div()
        .size(px(BUTTON))
        .rounded(px(7.0))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(widgets::RING_SLOT);
    if enabled {
        step.cursor_pointer()
            .hover(|s| s.bg(ink(0.06)))
            .child(icons::icon(icon).size(px(14.0)).text_color(theme.text))
    } else {
        step.child(
            icons::icon(icon)
                .size(px(14.0))
                .text_color(theme.text_faint),
        )
    }
}
