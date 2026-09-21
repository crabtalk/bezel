//! Where the panel a [`crate::menu::Item::Submenu`] row drops goes: beside the
//! card that dropped it, on whichever side the window has room for.
//!
//! gpui's `anchored` fits a layer to the window around a *point*. Both of its
//! modes are wrong for a submenu, because the point it would work from is the
//! parent card's own right edge: snapping slides the panel left until it fits,
//! straight over the card, and `SwitchAnchor` mirrors it about that edge, which
//! lands it on the card almost exactly. A submenu has to clear a rectangle, so
//! [`place`] works from one.
//!
//! The rectangle is not known when the element tree is built — the panel's
//! width is a measurement and the card's edges move with whatever fitting the
//! card itself went through. A canvas on the row reports it during prepaint,
//! one deferred round before the panel is placed.

use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, Display, Element, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Point, Position, Size, Style, Window, canvas, div, point, prelude::*, px,
};

use super::{MENU_PAD, SNAP};

/// Which way a panel opens from the card it hangs on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Right,
    Left,
}

impl Side {
    fn other(self) -> Self {
        match self {
            Side::Right => Side::Left,
            Side::Left => Side::Right,
        }
    }
}

/// The side every panel of one open menu opens to.
///
/// Shared down the chain rather than settled per panel: a panel that had to
/// flip is one whose own children have no room to the right either, and a
/// deeper panel deciding on its own would open back across its parent.
///
/// A frame's worth of state, not a menu's. [`crate::menu::card`] makes one per
/// render and every panel settles the side again from where the cards actually
/// landed, so nothing here outlives the geometry it was read from.
#[derive(Clone, Default)]
pub struct Chain(Rc<Cell<Side>>);

/// Where a panel of `size` goes, and which side it took.
///
/// `anchor` is the parent card's horizontal span at the top the panel's first
/// row should line up with. A panel's edge sits on the card's with nothing
/// between: a strip of nothing between the two is a strip the pointer crosses
/// on its way in, and it would land on a sibling row and close what it was
/// reaching for.
///
/// `prefer` is the side the rest of the chain took, kept unless the window has
/// no room for it. `margin` is the gap held at the window edge when neither
/// side fits and the panel is snapped instead.
pub fn place(
    anchor: Bounds<Pixels>,
    size: Size<Pixels>,
    viewport: Size<Pixels>,
    margin: Pixels,
    prefer: Side,
) -> (Point<Pixels>, Side) {
    let x = |side: Side| match side {
        Side::Right => anchor.right(),
        Side::Left => anchor.left() - size.width,
    };
    let fits = |side: Side| {
        let left = x(side);
        left >= margin && left + size.width + margin <= viewport.width
    };
    let side = match (fits(prefer), fits(prefer.other())) {
        (false, true) => prefer.other(),
        // Neither side fits: the preferred one is snapped below, which is what
        // a window narrower than two panels can do.
        _ => prefer,
    };

    let mut origin = point(x(side), anchor.origin.y);
    if origin.x + size.width > viewport.width {
        origin.x -= origin.x + size.width - viewport.width + margin;
    }
    if origin.x < Pixels::ZERO {
        origin.x = margin;
    }
    // Vertically a panel slides rather than flipping, the way every menu does:
    // its first row lines up with the row it hangs on, and a panel too tall for
    // the room below that row rides up until it fits.
    if origin.y + size.height > viewport.height {
        origin.y -= origin.y + size.height - viewport.height + margin;
    }
    if origin.y < Pixels::ZERO {
        origin.y = margin;
    }
    (origin, side)
}

/// The panel a submenu row drops, ready to be mounted on that row — which must
/// be `relative()`, since everything here is placed against it.
///
/// `content` arrives already wrapped in its surface and its entrance: this
/// positions it and nothing else.
pub(super) fn layer(content: AnyElement, chain: &Chain) -> AnyElement {
    let anchor = Rc::new(Cell::new(None));
    let measure = {
        let anchor = anchor.clone();
        canvas(
            move |bounds, _, _| anchor.set(Some(bounds)),
            |_, _, _, _| {},
        )
    };
    div()
        .absolute()
        .inset_0()
        // The card's span at the row's top: the row is a block child of a card
        // inset by `MENU_PAD` on every side, so its own box pushed out by that
        // much on three sides is the card's, and the top is where the panel's
        // first row has to start to line up with this one.
        .child(
            measure
                .absolute()
                .top(px(-MENU_PAD))
                .left(px(-MENU_PAD))
                .right(px(-MENU_PAD))
                .h_0(),
        )
        .child(
            // The panel's own layout context: a zero-size absolute box, which
            // is what it is measured inside today. Nothing is read off this
            // box's position but the placement it falls back to.
            div()
                .absolute()
                .top(px(-MENU_PAD))
                .right(px(-MENU_PAD))
                .size_0()
                .child(
                    gpui::deferred(Panel {
                        child: content,
                        anchor,
                        chain: chain.clone(),
                    })
                    .priority(1),
                ),
        )
        .into_any_element()
}

/// The placed panel: gpui's `anchored` with [`place`]'s fitting instead of its
/// own, and its layout — absolute, so the panel sizes to itself rather than to
/// the row it hangs on.
struct Panel {
    child: AnyElement,
    /// The rectangle the canvas measured this frame. `None` only if the panel
    /// is prepainted before the row it hangs on, which the deferred rounds
    /// rule out.
    anchor: Rc<Cell<Option<Bounds<Pixels>>>>,
    chain: Chain,
}

impl Element for Panel {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        let style = Style {
            position: Position::Absolute,
            display: Display::Flex,
            ..Style::default()
        };
        (window.request_layout(style, [child], cx), child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = window.layout_bounds(*child).size;
        let origin = match self.anchor.get() {
            Some(anchor) => {
                let margin = px(SNAP) + window.client_inset().unwrap_or(Pixels::ZERO);
                let (origin, side) =
                    place(anchor, size, window.viewport_size(), margin, self.chain.0.get());
                self.chain.0.set(side);
                origin
            }
            None => bounds.origin,
        };
        let offset = origin - bounds.origin;
        window.with_element_offset(point(offset.x.round(), offset.y.round()), |window| {
            self.child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for Panel {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
