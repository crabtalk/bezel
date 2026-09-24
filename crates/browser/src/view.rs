use gpui::{
    App, Bounds, Context, CursorStyle, Element, ElementId, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId, Pixels,
    Render, Style, Subscription, Task, Window, relative,
};
use serde::de::DeserializeOwned;
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
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
/// gpui elements behind the page are not hovered, and gpui's cursor over the
/// page is the arrow.
///
/// Paints nothing off macOS.
pub struct WebView {
    page: Rc<Page>,
    location: Option<String>,
    title: String,
    loading: bool,
    _focus: [Subscription; 2],
    _reports: Task<()>,
}

/// What the page reports about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebViewEvent {
    /// The page's URL changed: a load, a redirect, or the page's own history
    /// move (`pushState`, `replaceState`, a fragment).
    Location(String),
    /// The page's title changed.
    Title(String),
    Load(LoadState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadState {
    Started,
    Finished,
}

/// Why [`WebView::eval`] has no value.
#[derive(Debug)]
pub enum EvalError {
    /// The page is not built yet, or not on this platform.
    Unavailable,
    /// No answer within the timeout.
    Timeout,
    /// The script threw, or its value has no JSON form.
    Script,
    /// The JSON does not decode as the asked type.
    Decode(serde_json::Error),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("no page to evaluate in"),
            Self::Timeout => f.write_str("script timed out"),
            Self::Script => f.write_str("script threw or returned no JSON value"),
            Self::Decode(error) => write!(f, "script value: {error}"),
        }
    }
}

impl std::error::Error for EvalError {}

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
        let (reports, received) = async_channel::unbounded();
        // Ends when the page drops, which drops every sender.
        let task = cx.spawn_in(window, async move |this, cx| {
            while let Ok(report) = received.recv().await {
                if this
                    .update_in(cx, |this, window, cx| this.report(report, window, cx))
                    .is_err()
                {
                    return;
                }
            }
        });
        Self {
            page: Rc::new(Page::new(url.into(), focus, reports)),
            location: None,
            title: String::new(),
            loading: false,
            _focus: subscriptions,
            _reports: task,
        }
    }

    /// The URL the page last reported; `None` before its first report.
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Before the first paint, replaces the URL the page is built with.
    /// After it, navigates the page.
    pub fn load(&mut self, url: impl Into<String>) {
        self.page.load(url.into());
    }

    pub fn back(&mut self) {
        self.page.back();
    }

    pub fn forward(&mut self) {
        self.page.forward();
    }

    pub fn reload(&mut self) {
        self.page.reload();
    }

    /// Evaluates `script`, a JavaScript expression, in the page's main frame
    /// and decodes its value from JSON. `undefined` decodes as `null`; a
    /// promise is not awaited.
    pub fn eval<T: DeserializeOwned + 'static>(
        &self,
        script: &str,
        timeout: Duration,
        cx: &App,
    ) -> Task<Result<T, EvalError>> {
        let (answer, answered) = async_channel::bounded::<Option<String>>(2);
        let timer = answer.clone();
        // WebKit hands the value to `NSJSONSerialization`, which cannot encode
        // a DOM node or `undefined`, so the page encodes it and this decodes
        // twice.
        let script = format!("JSON.stringify((\n{script}\n) ?? null)");
        if !self.page.eval(&script, move |json| {
            let _ = answer.try_send(Some(json));
        }) {
            return Task::ready(Err(EvalError::Unavailable));
        }
        let expired = cx.background_executor().timer(timeout);
        cx.foreground_executor()
            .spawn(async move {
                expired.await;
                let _ = timer.try_send(None);
            })
            .detach();
        cx.foreground_executor().spawn(async move {
            let json = answered
                .recv()
                .await
                .ok()
                .flatten()
                .ok_or(EvalError::Timeout)?;
            let json = serde_json::from_str::<Option<String>>(&json)
                .ok()
                .flatten()
                .ok_or(EvalError::Script)?;
            serde_json::from_str(&json).map_err(EvalError::Decode)
        })
    }

    fn report(&mut self, report: Report, window: &mut Window, cx: &mut Context<Self>) {
        match report {
            Report::Pressed => {
                if self.page.holds_keys() {
                    window.focus(&self.page.focus, cx);
                }
            }
            Report::Moved => {
                if let Some(url) = self.page.location() {
                    self.locate(url, cx);
                }
            }
            Report::Load(state, url) => {
                self.locate(url, cx);
                self.loading = state == LoadState::Started;
                cx.emit(WebViewEvent::Load(state));
            }
            // Live pages rewrite their title, some every second.
            Report::Title(title) => {
                if title != self.title {
                    self.title = title.clone();
                    cx.emit(WebViewEvent::Title(title));
                }
            }
        }
    }

    fn locate(&mut self, url: String, cx: &mut Context<Self>) {
        if self.location.as_ref() != Some(&url) {
            self.location = Some(url.clone());
            cx.emit(WebViewEvent::Location(url));
        }
    }
}

impl EventEmitter<WebViewEvent> for WebView {}

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

/// Sent from the page's callbacks to the view.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum Report {
    Pressed,
    /// The page moved its own history; its URL is read back from the page.
    Moved,
    Load(LoadState, String),
    Title(String),
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
        window.set_focus_handle(&self.page.focus, cx);
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
    /// What the page is built with; unread once it is built.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    url: RefCell<String>,
    #[cfg(target_os = "macos")]
    view: std::cell::OnceCell<Option<wry::WebView>>,
    /// Where the page last sat; `None` before the first paint and while parked.
    placed: Cell<Option<Bounds<Pixels>>>,
    /// The token of the [`Shown`] that placed the page last.
    owner: Cell<u64>,
    focus: FocusHandle,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    reports: async_channel::Sender<Report>,
}

impl Page {
    fn new(url: String, focus: FocusHandle, reports: async_channel::Sender<Report>) -> Self {
        Self {
            url: RefCell::new(url),
            #[cfg(target_os = "macos")]
            view: std::cell::OnceCell::new(),
            placed: Cell::new(None),
            owner: Cell::new(0),
            focus,
            reports,
        }
    }
}

#[cfg(target_os = "macos")]
impl Page {
    /// Where a parked page waits. Hidden alone, a WKWebView stays registered as
    /// a drag destination over its last rect and takes every drag that crosses
    /// it.
    const PARKED: f64 = -20_000.0;

    /// Run in every frame. Any script in the page can post the same messages.
    const PRESSED: &str = "addEventListener('mousedown', () => \
        window.webkit.messageHandlers.ipc.postMessage('pressed'), true);";

    /// Run in the main frame.
    const MOVED: &str = "(() => {
        const moved = () => window.webkit.messageHandlers.ipc.postMessage('moved');
        for (const name of ['pushState', 'replaceState']) {
            const original = history[name];
            history[name] = function (...args) {
                const result = original.apply(this, args);
                moved();
                return result;
            };
        }
        addEventListener('popstate', moved);
        addEventListener('hashchange', moved);
    })();";

    fn place(&self, bounds: Bounds<Pixels>, window: &Window) {
        let view = self.view.get_or_init(|| self.build(bounds, window));
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

    fn build(&self, bounds: Bounds<Pixels>, window: &Window) -> Option<wry::WebView> {
        let url = self.url.borrow();
        let (ipc, loads, titles) = (
            self.reports.clone(),
            self.reports.clone(),
            self.reports.clone(),
        );
        wry::WebViewBuilder::new()
            .with_url(url.as_str())
            .with_bounds(rect(bounds))
            .with_initialization_script_for_main_only(Self::PRESSED, false)
            .with_initialization_script(Self::MOVED)
            .with_ipc_handler(move |request| {
                let report = match request.body().as_str() {
                    "pressed" => Report::Pressed,
                    "moved" => Report::Moved,
                    _ => return,
                };
                let _ = ipc.try_send(report);
            })
            .with_on_page_load_handler(move |event, url| {
                let state = match event {
                    wry::PageLoadEvent::Started => LoadState::Started,
                    wry::PageLoadEvent::Finished => LoadState::Finished,
                };
                let _ = loads.try_send(Report::Load(state, url));
            })
            .with_document_title_changed_handler(move |title| {
                let _ = titles.try_send(Report::Title(title));
            })
            .build_as_child(window)
            .inspect_err(|error| tracing::warn!(%error, url = %url, "webview: build"))
            .ok()
    }

    fn built(&self) -> Option<&wry::WebView> {
        self.view.get()?.as_ref()
    }

    fn load(&self, url: String) {
        match self.built() {
            Some(view) => {
                let _ = view.load_url(&url);
            }
            None => *self.url.borrow_mut() = url,
        }
    }

    fn back(&self) {
        use wry::WebViewExtMacOS;

        if let Some(view) = self.built() {
            // SAFETY: called on the main thread.
            unsafe { view.webview().goBack() };
        }
    }

    fn forward(&self) {
        use wry::WebViewExtMacOS;

        if let Some(view) = self.built() {
            // SAFETY: called on the main thread.
            unsafe { view.webview().goForward() };
        }
    }

    fn reload(&self) {
        if let Some(view) = self.built() {
            let _ = view.reload();
        }
    }

    fn location(&self) -> Option<String> {
        self.built()?.url().ok()
    }

    /// Whether the script was handed to the page.
    fn eval(&self, script: &str, done: impl Fn(String) + Send + 'static) -> bool {
        self.built()
            .is_some_and(|view| view.evaluate_script_with_callback(script, done).is_ok())
    }

    /// Whether the page, or a view inside it, is its window's first responder.
    fn holds_keys(&self) -> bool {
        use objc2_app_kit::NSView;
        use wry::WebViewExtMacOS;

        let Some(view) = self.built() else {
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

        let Some(view) = self.built() else {
            return;
        };
        // `focus` unwraps the page's window.
        if self.placed.get().is_some() && view.webview().window().is_some() {
            let _ = view.focus();
        }
    }

    /// Hands first responder back to gpui's view if the page holds it.
    fn give_keys(&self) {
        if let Some(view) = self.built()
            && self.holds_keys()
        {
            let _ = view.focus_parent();
        }
    }

    fn park(&self) {
        use wry::WebViewExtMacOS;

        let Some(view) = self.built() else {
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
    fn place(&self, bounds: Bounds<Pixels>, _window: &Window) {
        self.placed.set(Some(bounds));
    }

    fn load(&self, url: String) {
        *self.url.borrow_mut() = url;
    }

    fn back(&self) {}

    fn forward(&self) {}

    fn reload(&self) {}

    fn location(&self) -> Option<String> {
        None
    }

    fn eval(&self, _script: &str, _done: impl Fn(String) + Send + 'static) -> bool {
        false
    }

    fn holds_keys(&self) -> bool {
        false
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
