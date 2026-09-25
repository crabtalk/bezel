use gpui::{
    App, Bounds, CursorStyle, Element, ElementId, FocusHandle, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window, relative,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

/// A native surface over the window that a [`Host`] places.
pub(crate) trait Surface: 'static {
    fn focus(&self) -> Option<&FocusHandle>;
    /// The token of the [`Shown`] that placed the surface last.
    fn owner(&self) -> &Cell<u64>;
    fn place(&self, bounds: Bounds<Pixels>, window: &Window);
    fn park(&self);
}

/// Fills its parent and places a surface over itself.
pub(crate) struct Host<S: Surface> {
    pub(crate) surface: Rc<S>,
}

impl<S: Surface> IntoElement for Host<S> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<S: Surface> Element for Host<S> {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some("webview".into())
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
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        if let Some(focus) = self.surface.focus() {
            window.set_focus_handle(focus, cx);
        }
        window.insert_hitbox(bounds, HitboxBehavior::BlockMouse)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        _cx: &mut App,
    ) {
        // gpui's view tracks the pointer across the page's pixels too, and
        // resets the platform cursor whenever its own style under the pointer
        // changes, over whatever WebKit set.
        window.set_cursor_style(CursorStyle::Arrow, hitbox);
        let Some(id) = id else { return };
        let surface = self.surface.clone();
        window.with_element_state::<Shown<S>, _>(id, |shown, window| {
            let shown = shown.unwrap_or_else(|| Shown::new(surface.clone()));
            surface.owner().set(shown.token);
            surface.place(bounds, window);
            ((), shown)
        });
    }
}

/// Held in the host's element state. gpui drops the state of an element a
/// frame did not paint, which is what parks the surface.
struct Shown<S: Surface> {
    surface: Rc<S>,
    token: u64,
}

impl<S: Surface> Shown<S> {
    fn new(surface: Rc<S>) -> Self {
        static TOKENS: AtomicU64 = AtomicU64::new(1);
        Self {
            surface,
            token: TOKENS.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl<S: Surface> Drop for Shown<S> {
    // A host whose id path changed paints its new state before the old one is
    // dropped, so only the state that placed the surface last may park it.
    fn drop(&mut self) {
        if self.surface.owner().get() == self.token {
            self.surface.park();
        }
    }
}
