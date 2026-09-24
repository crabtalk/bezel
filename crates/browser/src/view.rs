use gpui::{
    App, Bounds, Context, Element, ElementId, FocusHandle, Focusable, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Render, Style, Subscription, Window,
    relative,
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
/// The page takes keys while the view's focus handle is focused, and a press
/// in the page focuses the handle. Key equivalents (cmd or ctrl held) reach
/// gpui's key dispatch before the page sees them.
///
/// Paints nothing off macOS.
pub struct WebView {
    page: Rc<Page>,
    _focus: [Subscription; 2],
}

impl WebView {
    pub fn new(url: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let subscriptions = [
            cx.on_focus(&focus, window, |this: &mut Self, _, _| {
                this.page.take_keys()
            }),
            cx.on_blur(&focus, window, |this: &mut Self, _, _| {
                this.page.give_keys()
            }),
        ];
        Self {
            page: Rc::new(Page::new(url.into(), focus)),
            _focus: subscriptions,
        }
    }
}

impl Focusable for WebView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.page.focus.clone()
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
        window: &mut Window,
        cx: &mut App,
    ) {
        window.set_focus_handle(&self.page.focus, cx);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(id) = id else { return };
        let page = self.page.clone();
        window.with_element_state::<Shown, _>(id, |shown, window| {
            let shown = shown.unwrap_or_else(|| Shown::new(page.clone()));
            page.owner.set(shown.token);
            page.place(bounds, window, cx);
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
    focus: FocusHandle,
}

impl Page {
    fn new(url: String, focus: FocusHandle) -> Self {
        Self {
            url,
            #[cfg(target_os = "macos")]
            view: std::cell::OnceCell::new(),
            placed: Cell::new(None),
            owner: Cell::new(0),
            focus,
        }
    }
}

#[cfg(target_os = "macos")]
impl Page {
    /// Where a parked page waits. Hidden alone, a WKWebView stays registered as
    /// a drag destination over its last rect and takes every drag that crosses
    /// it.
    const PARKED: f64 = -20_000.0;

    /// Posted by every frame of the page on a press. Any script in the page
    /// can post it too.
    const PRESSED: &str = "pressed";

    fn place(self: &Rc<Self>, bounds: Bounds<Pixels>, window: &Window, cx: &App) {
        let view = self.view.get_or_init(|| self.build(bounds, window, cx));
        let Some(view) = view else { return };
        let placed = self.placed.get();
        if placed == Some(bounds) {
            return;
        }
        let _ = view.set_bounds(rect(bounds));
        let _ = view.set_visible(true);
        self.placed.set(Some(bounds));
        if placed.is_none() && self.focus.is_focused(window) {
            self.take_keys();
        }
    }

    fn build(
        self: &Rc<Self>,
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &App,
    ) -> Option<wry::WebView> {
        let (pressed, presses) = async_channel::unbounded::<()>();
        let view = wry::WebViewBuilder::new()
            .with_url(&self.url)
            .with_bounds(rect(bounds))
            .with_initialization_script_for_main_only(
                format!(
                    "addEventListener('mousedown', () => \
                     window.webkit.messageHandlers.ipc.postMessage('{}'), true);",
                    Self::PRESSED
                ),
                false,
            )
            .with_ipc_handler(move |request| {
                if request.body() == Self::PRESSED {
                    let _ = pressed.try_send(());
                }
            })
            .build_as_child(window)
            .inspect_err(|error| tracing::warn!(%error, url = %self.url, "webview: build"))
            .ok()?;
        // Ends when the page drops, which drops the sender held by the handler.
        let page = Rc::downgrade(self);
        window
            .spawn(cx, async move |cx| {
                while presses.recv().await.is_ok() {
                    let Some(page) = page.upgrade() else { return };
                    if page.holds_keys() {
                        let _ = cx.update(|window, cx| window.focus(&page.focus, cx));
                    }
                }
            })
            .detach();
        Some(view)
    }

    /// Whether the page, or a view inside it, is its window's first responder.
    fn holds_keys(&self) -> bool {
        use objc2_app_kit::NSView;
        use wry::WebViewExtMacOS;

        let Some(Some(view)) = self.view.get() else {
            return false;
        };
        let page = view.webview();
        let Some(window) = page.window() else {
            return false;
        };
        window
            .firstResponder()
            .and_then(|responder| responder.downcast::<NSView>().ok())
            .is_some_and(|responder| responder.isDescendantOf(&page))
    }

    fn take_keys(&self) {
        use wry::WebViewExtMacOS;

        let Some(Some(view)) = self.view.get() else {
            return;
        };
        // `focus` unwraps the page's window.
        if self.placed.get().is_some() && view.webview().window().is_some() {
            let _ = view.focus();
        }
    }

    /// Hands first responder back to gpui's view if the page holds it.
    fn give_keys(&self) {
        if let Some(Some(view)) = self.view.get()
            && self.holds_keys()
        {
            let _ = view.focus_parent();
        }
    }

    fn park(&self) {
        use wry::WebViewExtMacOS;

        let Some(Some(view)) = self.view.get() else {
            return;
        };
        let Some(bounds) = self.placed.take() else {
            return;
        };
        self.give_keys();
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
    fn place(self: &Rc<Self>, bounds: Bounds<Pixels>, _window: &Window, _cx: &App) {
        self.placed.set(Some(bounds));
    }

    fn park(&self) {
        self.placed.set(None);
    }

    fn take_keys(&self) {}

    fn give_keys(&self) {}
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
