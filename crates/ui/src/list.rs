//! Virtualized list — a thin binding over gpui's `uniform_list`, and a bridge
//! that lets [`crate::scroll::scrollbar`] report on one.
//!
//! Thin on purpose: gpui already does the hard part, and a wrapper that only
//! re-exported it with extra steps would be worse than none. This module exists
//! for two things it can guarantee that a caller otherwise has to know.
//!
//! **The row height.** `uniform_list` measures the *first* row it renders and
//! lays every other one out at that height. Hand it rows of different heights
//! and nothing errors — the content simply overlaps, at a size nobody chose.
//! [`virtual_list`] takes the height and applies it to every row it hands back,
//! so that cannot happen by accident.
//!
//! **The scroll handle.** A `UniformListScrollHandle` wraps a real
//! [`ScrollHandle`] and gpui registers it as the list's tracked handle, so the
//! bar's geometry is all there — behind `handle.0.borrow().base_handle`, which
//! is not something a consumer should have to find by reading gpui's source.
//! [`scroll_handle`] is that reach, named.
//!
//! ## Why not gpui's `list()`
//!
//! gpui has a second virtualizer for rows of *varying* height, and it cannot
//! carry a proportional scrollbar: `ListState` speaks in `ListOffset { item_ix,
//! offset_in_item }` — logical position, not pixels — with no maximum offset
//! and no viewport. A thumb's length is the visible share of a total height, and
//! a variable-height list cannot know its total without measuring every row,
//! which is the work virtualization exists to skip. A list of thousands of rows
//! wants a bar; a list that needs varying heights is a different component, and
//! nothing has asked for one yet.
//!
//! ```ignore
//! div().relative().h(px(240.0))
//!     .child(virtual_list("rows", rows.len(), px(28.0), &self.rows_scroll, {
//!         let rows = rows.clone();
//!         move |range, _, _| range.map(|ix| row(&rows[ix])).collect()
//!     }))
//!     .child(scroll::scrollbar("rows-bar", &list::scroll_handle(&self.rows_scroll), &self.rows_bar))
//! ```

use std::ops::Range;

use gpui::{
    App, ElementId, IntoElement, Pixels, ScrollHandle, UniformList, UniformListScrollHandle,
    Window, prelude::*, uniform_list,
};

/// The pixel-space scroll handle inside a `UniformListScrollHandle`.
///
/// `uniform_list` tracks its scrolling through this one, so it carries the
/// offset, the maximum offset and the viewport that [`crate::scroll::thumb`]
/// needs — a virtualized list takes the same bar as any other scroller, with no
/// second implementation behind a trait.
///
/// The clone shares state rather than copying it: the returned handle *is* the
/// list's, and moving one moves the other.
pub fn scroll_handle(handle: &UniformListScrollHandle) -> ScrollHandle {
    handle.0.borrow().base_handle.clone()
}

/// A list that builds only the rows on screen.
///
/// `render` is handed the visible range and returns one element per index in
/// it; each comes back sized to `row_height`, which is what keeps the list
/// uniform and therefore virtualizable at all.
///
/// Fills its parent, which is the other thing that has to be true for any of
/// this to work: a list with no height of its own collapses, and a collapsed
/// list builds a single row to measure and then nothing — an empty box, no
/// error, no clue. A virtualized list is bounded by definition, so filling is
/// the only sane default; a caller wanting otherwise sets its own size after,
/// and the later call wins.
pub fn virtual_list<R>(
    id: impl Into<ElementId>,
    count: usize,
    row_height: Pixels,
    handle: &UniformListScrollHandle,
    render: impl 'static + Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>,
) -> UniformList
where
    R: IntoElement,
{
    uniform_list(id, count, move |range, window, cx| {
        render(range, window, cx)
            .into_iter()
            .map(|row| gpui::div().h(row_height).w_full().child(row))
            .collect::<Vec<_>>()
    })
    .size_full()
    .track_scroll(handle)
}
