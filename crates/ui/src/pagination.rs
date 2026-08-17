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

use bezel_theme::{Theme, ink};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(slots: &[Slot]) -> String {
        slots
            .iter()
            .map(|slot| match slot {
                Slot::Page(page) => page.to_string(),
                Slot::Gap => "…".to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_short_run_shows_every_page() {
        assert_eq!(pages(&window(3, 5, 2)), "1 2 3 4 5");
        assert_eq!(pages(&window(1, 3, 2)), "1 2 3");
    }

    #[test]
    fn a_long_run_gaps_both_sides() {
        assert_eq!(pages(&window(6, 20, 2)), "1 … 4 5 6 7 8 … 20");
    }

    #[test]
    fn a_gap_hiding_one_page_shows_the_page_instead() {
        // The window is 3..=7, so page 2 alone sits outside it on the left —
        // "…" would take the same room and say less.
        assert_eq!(pages(&window(5, 20, 2)), "1 2 3 4 5 6 7 … 20");
        // And the mirror, at the other end.
        assert_eq!(pages(&window(16, 20, 2)), "1 … 14 15 16 17 18 19 20");
    }

    #[test]
    fn the_window_slides_at_the_ends_rather_than_shrinking() {
        // Five pages of window at both extremes, not three.
        let first = window(1, 20, 2);
        assert_eq!(pages(&first), "1 2 3 4 5 … 20");
        let last = window(20, 20, 2);
        assert_eq!(pages(&last), "1 … 16 17 18 19 20");
        let middle = window(10, 20, 2);
        assert_eq!(
            window_width(&first),
            window_width(&middle),
            "the run of pages around the current one keeps its width"
        );
        assert_eq!(window_width(&last), window_width(&middle));
    }

    /// Longest run of consecutive pages — the window, whichever end it is at.
    fn window_width(slots: &[Slot]) -> usize {
        let mut best = 0;
        let mut run = 0;
        let mut previous = None;
        for slot in slots {
            match slot {
                Slot::Page(page) if previous == Some(page.saturating_sub(1)) => run += 1,
                Slot::Page(_) => run = 1,
                Slot::Gap => run = 0,
            }
            previous = match slot {
                Slot::Page(page) => Some(*page),
                Slot::Gap => None,
            };
            best = best.max(run);
        }
        best
    }

    #[test]
    fn the_degenerate_runs_answer_something_sane() {
        assert!(window(1, 0, 2).is_empty(), "nothing to page through");
        assert_eq!(pages(&window(1, 1, 2)), "1");
        // `around` of zero still shows where you are and both ends.
        assert_eq!(pages(&window(6, 20, 0)), "1 … 6 … 20");
    }

    #[test]
    fn a_current_page_out_of_range_is_clamped_not_trusted() {
        assert_eq!(pages(&window(0, 20, 2)), pages(&window(1, 20, 2)));
        assert_eq!(pages(&window(999, 20, 2)), pages(&window(20, 20, 2)));
    }
}
