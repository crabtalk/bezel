//! One scroll position, whichever element owns it.

use gpui::{Bounds, ListState, Pixels, Point, ScrollHandle, point, px};

/// What a bar, a follow or a drag region reads and moves: a scrolling div's
/// [`ScrollHandle`] or a [`gpui::list`]'s [`ListState`], in the handle's terms
/// — `offset` negative going down, `max_offset` the overflow.
///
/// A list counts only the items it has measured: one never laid out adds
/// nothing to `max_offset`, so both readings grow as the list is scrolled
/// through. A list's offset is vertical only.
///
/// `offset` never reads past `max_offset`. A div's wheel handler moves its
/// handle past the end and the div clamps it when next laid out, so a raw
/// read before that layout is where the content will not be drawn.
#[derive(Clone)]
pub enum Scroller {
    Pane(ScrollHandle),
    List(ListState),
}

impl Scroller {
    /// The viewport as last laid out, in window coordinates.
    pub fn bounds(&self) -> Bounds<Pixels> {
        match self {
            Self::Pane(handle) => handle.bounds(),
            Self::List(state) => state.viewport_bounds(),
        }
    }

    pub fn offset(&self) -> Point<Pixels> {
        match self {
            Self::Pane(handle) => {
                let (offset, max) = (handle.offset(), handle.max_offset());
                point(
                    offset.x.clamp(-max.x, px(0.)),
                    offset.y.clamp(-max.y, px(0.)),
                )
            }
            Self::List(state) => {
                let offset = state.scroll_px_offset_for_scrollbar();
                point(offset.x, offset.y.max(-state.max_offset_for_scrollbar().y))
            }
        }
    }

    pub fn max_offset(&self) -> Point<Pixels> {
        match self {
            Self::Pane(handle) => handle.max_offset(),
            Self::List(state) => state.max_offset_for_scrollbar(),
        }
    }

    /// Takes effect at the next layout. A list clamps to what it has measured.
    pub fn set_offset(&self, offset: Point<Pixels>) {
        match self {
            Self::Pane(handle) => handle.set_offset(offset),
            Self::List(state) => state.set_offset_from_scrollbar(offset),
        }
    }

    /// Past the last of the content. A list anchors on its last item and lays
    /// out backwards from it, so the end is reached in one frame however much
    /// of it is unmeasured.
    pub fn scroll_to_end(&self) {
        match self {
            Self::Pane(handle) => {
                handle.set_offset(point(handle.offset().x, -handle.max_offset().y))
            }
            Self::List(state) => state.scroll_to_end(),
        }
    }
}

impl From<ScrollHandle> for Scroller {
    fn from(handle: ScrollHandle) -> Self {
        Self::Pane(handle)
    }
}

impl From<&ScrollHandle> for Scroller {
    fn from(handle: &ScrollHandle) -> Self {
        Self::Pane(handle.clone())
    }
}

impl From<ListState> for Scroller {
    fn from(state: ListState) -> Self {
        Self::List(state)
    }
}

impl From<&ListState> for Scroller {
    fn from(state: &ListState) -> Self {
        Self::List(state.clone())
    }
}
