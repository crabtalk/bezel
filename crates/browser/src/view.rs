use gpui::{
    App, Bounds, Context, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Render, Style, Window, relative,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

/// A webview showing one page.
///
/// The page is built the first time the view is painted, in the window it is
/// painted in, and stays in that window. It sits at the element's bounds in
/// every frame the element is painted and is parked in every frame it is not.
/// A parked page stays loaded.
///
/// Paints nothing off macOS.
pub struct WebView {
    page: Rc<Page>,
}

impl WebView {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            page: Rc::new(Page::new(url.into())),
        }
    }
}

impl Render for WebView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Host {
            page: self.page.clone(),
        }
    }
}

/// Fills its parent and places the page over itself.
struct Host {
    page: Rc<Page>,
}

impl IntoElement for Host {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Host {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let Some(id) = id else { return };
        let page = self.page.clone();
        window.with_element_state::<Shown, _>(id, |shown, window| {
            let shown = shown.unwrap_or_else(|| Shown::new(page.clone()));
            page.owner.set(shown.token);
            page.place(bounds, window);
            ((), shown)
        });
    }
}

/// Held in the host's element state. gpui drops the state of an element a
/// frame did not paint, which is what parks the page.
struct Shown {
    page: Rc<Page>,
    token: u64,
}

impl Shown {
    fn new(page: Rc<Page>) -> Self {
        static TOKENS: AtomicU64 = AtomicU64::new(1);
        Self {
            page,
            token: TOKENS.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl Drop for Shown {
    // A host whose id path changed paints its new state before the old one is
    // dropped, so only the state that placed the page last may park it.
    fn drop(&mut self) {
        if self.page.owner.get() == self.token {
            self.page.park();
        }
    }
}

struct Page {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    url: String,
    #[cfg(target_os = "macos")]
    view: std::cell::OnceCell<Option<wry::WebView>>,
    /// Where the page last sat; `None` before the first paint and while parked.
    placed: Cell<Option<Bounds<Pixels>>>,
    /// The token of the [`Shown`] that placed the page last.
    owner: Cell<u64>,
}

impl Page {
    fn new(url: String) -> Self {
        Self {
            url,
            #[cfg(target_os = "macos")]
            view: std::cell::OnceCell::new(),
            placed: Cell::new(None),
            owner: Cell::new(0),
        }
    }
}

#[cfg(target_os = "macos")]
impl Page {
    /// Where a parked page waits. Hidden alone, a WKWebView stays registered as
    /// a drag destination over its last rect and takes every drag that crosses
    /// it.
    const PARKED: f64 = -20_000.0;

    fn place(&self, bounds: Bounds<Pixels>, window: &Window) {
        let view = self.view.get_or_init(|| {
            wry::WebViewBuilder::new()
                .with_url(&self.url)
                .with_bounds(rect(bounds))
                .build_as_child(window)
                .inspect_err(|error| tracing::warn!(%error, url = %self.url, "webview: build"))
                .ok()
        });
        let Some(view) = view else { return };
        if self.placed.get() == Some(bounds) {
            return;
        }
        let _ = view.set_bounds(rect(bounds));
        let _ = view.set_visible(true);
        self.placed.set(Some(bounds));
    }

    fn park(&self) {
        use wry::WebViewExtMacOS;

        let Some(Some(view)) = self.view.get() else {
            return;
        };
        let Some(bounds) = self.placed.take() else {
            return;
        };
        let _ = view.set_visible(false);
        // `set_bounds` unwraps the page's window, which is gone once the window
        // has closed.
        if view.webview().window().is_none() {
            return;
        }
        // At its own size, so the page does not lay out again for a viewport
        // nobody sees.
        let _ = view.set_bounds(wry::Rect {
            position: wry::dpi::LogicalPosition::new(Self::PARKED, Self::PARKED).into(),
            ..rect(bounds)
        });
    }
}

#[cfg(not(target_os = "macos"))]
impl Page {
    fn place(&self, bounds: Bounds<Pixels>, _window: &Window) {
        self.placed.set(Some(bounds));
    }

    fn park(&self) {
        self.placed.set(None);
    }
}

#[cfg(target_os = "macos")]
fn rect(bounds: Bounds<Pixels>) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(
            f64::from(f32::from(bounds.origin.x)),
            f64::from(f32::from(bounds.origin.y)),
        )
        .into(),
        size: wry::dpi::LogicalSize::new(
            f64::from(f32::from(bounds.size.width)),
            f64::from(f32::from(bounds.size.height)),
        )
        .into(),
    }
}
