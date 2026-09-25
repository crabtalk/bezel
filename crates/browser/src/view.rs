use crate::host::{Host, Surface};
use gpui::{
    Action, App, Bounds, Context, EventEmitter, FocusHandle, Focusable, Global, IntoElement,
    KeyBinding, Pixels, Render, Subscription, Task, Window, actions, div, prelude::*,
};
use serde::de::DeserializeOwned;
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
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
/// in the page focuses the handle. On macOS, key equivalents (cmd or ctrl
/// held) reach gpui's key dispatch before the page sees them; on Windows the
/// page takes every key while it holds focus.
///
/// gpui elements behind the page are not hovered, and gpui's cursor over the
/// page is the arrow. On macOS the page sets the cursor over itself.
///
/// Linux needs gpui on X11 and paints nothing under Wayland. Paints nothing
/// off macOS, Windows and Linux.
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
        #[cfg(target_os = "linux")]
        gtk_loop::start(window, cx);
        bind_edits(cx);
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

    /// The user agent the page is built with, in place of the platform's.
    /// Read at the first paint. On macOS the default is Safari's, with the
    /// installed Safari's version.
    pub fn with_user_agent(self, user_agent: impl Into<String>) -> Self {
        *self.page.user_agent.borrow_mut() = Some(user_agent.into());
        self
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context(KEY_CONTEXT)
            .on_action(on_edit::<Copy>(Edit::Copy, cx))
            .on_action(on_edit::<Cut>(Edit::Cut, cx))
            .on_action(on_edit::<Paste>(Edit::Paste, cx))
            .on_action(on_edit::<SelectAll>(Edit::SelectAll, cx))
            .on_action(on_edit::<Undo>(Edit::Undo, cx))
            .on_action(on_edit::<Redo>(Edit::Redo, cx))
            .child(Host {
                surface: self.page.clone(),
            })
    }
}

actions!(webview, [Copy, Cut, Paste, SelectAll, Undo, Redo]);

const KEY_CONTEXT: &str = "WebView";

/// Edits a WKWebView takes as responder actions (`copy:`, `paste:`) and only
/// through them: without an Edit menu, cmd-c in the page copies nothing.
#[derive(Clone, Copy)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum Edit {
    Copy,
    Cut,
    Paste,
    SelectAll,
    Undo,
    Redo,
}

fn on_edit<A: Action>(
    edit: Edit,
    cx: &mut Context<WebView>,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |this, _: &A, _, _| this.page.edit(edit))
}

/// Binds the edit keys in the view's context, once per app.
fn bind_edits(cx: &mut App) {
    struct Bound;
    impl Global for Bound {}

    if !cfg!(target_os = "macos") || cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    let context = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new("cmd-x", Cut, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("cmd-z", Undo, context),
        KeyBinding::new("cmd-shift-z", Redo, context),
    ]);
}

/// Sent from the page's callbacks to the view.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
    allow(dead_code)
)]
enum Report {
    Pressed,
    /// The page moved its own history; its URL is read back from the page.
    Moved,
    Load(LoadState, String),
    Title(String),
}

struct Page {
    /// What the page is built with; unread once it is built.
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        allow(dead_code)
    )]
    url: RefCell<String>,
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        allow(dead_code)
    )]
    user_agent: RefCell<Option<String>>,
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    view: std::cell::OnceCell<Option<wry::WebView>>,
    #[cfg(target_os = "macos")]
    cursor: std::cell::OnceCell<cursor::Watch>,
    /// Where the page last sat; `None` before the first paint and while parked.
    placed: Cell<Option<Bounds<Pixels>>>,
    owner: Cell<u64>,
    focus: FocusHandle,
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        allow(dead_code)
    )]
    reports: async_channel::Sender<Report>,
    /// Whether the page holds keyboard focus, as WebView2 last reported.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    focused: Rc<Cell<bool>>,
}

impl Page {
    fn new(url: String, focus: FocusHandle, reports: async_channel::Sender<Report>) -> Self {
        Self {
            url: RefCell::new(url),
            user_agent: RefCell::new(None),
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            view: std::cell::OnceCell::new(),
            #[cfg(target_os = "macos")]
            cursor: std::cell::OnceCell::new(),
            placed: Cell::new(None),
            owner: Cell::new(0),
            focus,
            reports,
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            focused: Rc::new(Cell::new(false)),
        }
    }
}

impl Surface for Page {
    fn focus(&self) -> Option<&FocusHandle> {
        Some(&self.focus)
    }

    fn owner(&self) -> &Cell<u64> {
        &self.owner
    }

    fn place(&self, bounds: Bounds<Pixels>, window: &Window) {
        Page::place(self, bounds, window);
    }

    fn park(&self) {
        Page::park(self);
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl Page {
    /// Where a parked page waits. Hidden alone, a WKWebView stays registered as
    /// a drag destination over its last rect and takes every drag that crosses
    /// it.
    const PARKED: f64 = -20_000.0;

    /// Run in the main frame, where wry defines `window.ipc`.
    const MOVED: &str = "(() => {
        const moved = () => window.ipc.postMessage('moved');
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
        let builder = wry::WebViewBuilder::new()
            .with_url(url.as_str())
            .with_bounds(rect(bounds))
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
            });
        let user_agent = self.user_agent.borrow().clone().or_else(default_user_agent);
        let builder = match user_agent {
            Some(user_agent) => builder.with_user_agent(user_agent),
            None => builder,
        };
        #[cfg(target_os = "linux")]
        let window = &gtk_loop::Parent::of(window).filter(|_| gtk::is_initialized())?;
        let view = self
            .configure(builder)
            .build_as_child(window)
            .inspect_err(|error| tracing::warn!(%error, url = %url, "webview: build"))
            .ok()?;
        self.attach(&view);
        Some(view)
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

    fn take_keys(&self) {
        let Some(view) = self.built() else {
            return;
        };
        if self.placed.get().is_some() && !closed(view) {
            let _ = view.focus();
        }
    }

    /// Hands keyboard focus back to gpui's view if the page holds it.
    fn give_keys(&self) {
        if let Some(view) = self.built()
            && self.holds_keys()
        {
            let _ = view.focus_parent();
        }
    }

    fn park(&self) {
        let Some(view) = self.built() else {
            return;
        };
        let Some(bounds) = self.placed.take() else {
            return;
        };
        self.give_keys();
        #[cfg(target_os = "macos")]
        if let Some(cursor) = self.cursor.get() {
            cursor.release();
        }
        let _ = view.set_visible(false);
        if closed(view) {
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

#[cfg(target_os = "macos")]
impl Page {
    /// Run in every frame. Any script in the page can post the same message.
    const PRESSED: &str = "addEventListener('mousedown', () => \
        window.webkit.messageHandlers.ipc.postMessage('pressed'), true);";

    fn configure<'a>(&self, builder: wry::WebViewBuilder<'a>) -> wry::WebViewBuilder<'a> {
        builder.with_initialization_script_for_main_only(Self::PRESSED, false)
    }

    fn attach(&self, view: &wry::WebView) {
        use wry::WebViewExtMacOS;

        let _ = self.cursor.set(cursor::Watch::new(&view.webview()));
    }

    /// Sent down the key window's responder chain, where the page's view is
    /// first while it holds keys.
    fn edit(&self, edit: Edit) {
        use objc2::{MainThreadMarker, sel};
        use objc2_app_kit::NSApplication;

        if !self.holds_keys() {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let action = match edit {
            Edit::Copy => sel!(copy:),
            Edit::Cut => sel!(cut:),
            Edit::Paste => sel!(paste:),
            Edit::SelectAll => sel!(selectAll:),
            Edit::Undo => sel!(undo:),
            Edit::Redo => sel!(redo:),
        };
        // SAFETY: a nil target resolves along the responder chain, and every
        // action here takes a sender, which may be nil.
        unsafe { NSApplication::sharedApplication(mtm).sendAction_to_from(action, None, None) };
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
}

/// A WKWebView reports `AppleWebKit/605.1.15 (KHTML, like Gecko)` and no
/// browser, and some sites (Google) serve an unknown browser a basic page.
#[cfg(target_os = "macos")]
fn default_user_agent() -> Option<String> {
    use objc2_foundation::{NSBundle, NSString};

    let version = NSBundle::bundleWithPath(&NSString::from_str("/Applications/Safari.app"))
        .and_then(|safari| {
            safari.objectForInfoDictionaryKey(&NSString::from_str("CFBundleShortVersionString"))
        })
        .and_then(|version| version.downcast::<NSString>().ok())
        .map_or_else(|| "26.0".to_owned(), |version| version.to_string());
    Some(format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
         (KHTML, like Gecko) Version/{version} Safari/605.1.15"
    ))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn default_user_agent() -> Option<String> {
    None
}

/// Hands the cursor to WebKit while the pointer is over the page.
///
/// gpui's view registers a cursor rect over its whole visible rect, the page's
/// pixels included, and AppKit applies it over the cursor WebKit sets. While
/// the pointer is inside the page the window's cursor rects are disabled.
#[cfg(target_os = "macos")]
mod cursor {
    use objc2::{
        AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
        rc::Retained,
        runtime::{NSObject, NSObjectProtocol},
    };
    use objc2_app_kit::{NSEvent, NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindow};
    use objc2_foundation::NSRect;
    use std::cell::Cell;

    pub(super) struct Watch {
        page: Retained<NSView>,
        area: Retained<NSTrackingArea>,
        /// A tracking area does not retain its owner.
        watcher: Retained<Watcher>,
    }

    impl Watch {
        pub(super) fn new(page: &NSView) -> Self {
            let mtm = MainThreadMarker::new().expect("the page is built on the main thread");
            let watcher: Retained<Watcher> = {
                let watcher = Watcher::alloc(mtm).set_ivars(Inside::default());
                // SAFETY: `NSObject`'s `init` on a freshly allocated instance.
                unsafe { msg_send![super(watcher), init] }
            };
            // SAFETY: the watcher answers `mouseEntered:` and `mouseExited:`
            // and outlives the area, which `Drop` removes first.
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    NSRect::ZERO,
                    NSTrackingAreaOptions::MouseEnteredAndExited
                        | NSTrackingAreaOptions::ActiveAlways
                        | NSTrackingAreaOptions::InVisibleRect,
                    Some(&watcher),
                    None,
                )
            };
            page.addTrackingArea(&area);
            Self {
                page: page.retain(),
                area,
                watcher,
            }
        }

        /// Gives the cursor back to gpui if the pointer was inside the page.
        /// A hidden view reports no exit.
        pub(super) fn release(&self) {
            if self.watcher.ivars().0.replace(false)
                && let Some(window) = self.page.window()
            {
                enable(&window);
            }
        }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            self.page.removeTrackingArea(&self.area);
            self.release();
        }
    }

    #[derive(Default)]
    struct Inside(Cell<bool>);

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "BezelWebViewCursorWatcher"]
        #[ivars = Inside]
        struct Watcher;

        unsafe impl NSObjectProtocol for Watcher {}

        impl Watcher {
            #[unsafe(method(mouseEntered:))]
            fn entered(&self, event: &NSEvent) {
                self.ivars().0.set(true);
                if let Some(window) = event.window(self.mtm()) {
                    window.disableCursorRects();
                }
            }

            #[unsafe(method(mouseExited:))]
            fn exited(&self, event: &NSEvent) {
                if self.ivars().0.replace(false)
                    && let Some(window) = event.window(self.mtm())
                {
                    enable(&window);
                }
            }
        }
    );

    fn enable(window: &NSWindow) {
        window.enableCursorRects();
        if let Some(content) = window.contentView() {
            window.invalidateCursorRectsForView(&content);
        }
    }
}

/// Whether the page's window has closed. wry's `set_bounds` and `focus`
/// unwrap the window.
#[cfg(target_os = "macos")]
fn closed(view: &wry::WebView) -> bool {
    use wry::WebViewExtMacOS;

    view.webview().window().is_none()
}

#[cfg(target_os = "windows")]
impl Page {
    fn configure<'a>(&self, builder: wry::WebViewBuilder<'a>) -> wry::WebViewBuilder<'a> {
        builder
    }

    /// WebView2 reports focus itself, so a press needs no script.
    fn attach(&self, view: &wry::WebView) {
        use webview2_com::FocusChangedEventHandler;
        use wry::WebViewExtWindows;

        let controller = view.controller();
        let (got, lost) = (self.focused.clone(), self.focused.clone());
        let pressed = self.reports.clone();
        let mut token = 0;
        // SAFETY: called on the thread that owns the controller.
        unsafe {
            let _ = controller.add_GotFocus(
                &FocusChangedEventHandler::create(Box::new(move |_, _| {
                    got.set(true);
                    let _ = pressed.try_send(Report::Pressed);
                    Ok(())
                })),
                &mut token,
            );
            let _ = controller.add_LostFocus(
                &FocusChangedEventHandler::create(Box::new(move |_, _| {
                    lost.set(false);
                    Ok(())
                })),
                &mut token,
            );
        }
    }

    fn back(&self) {
        use wry::WebViewExtWindows;

        if let Some(view) = self.built() {
            // SAFETY: called on the thread that owns the webview.
            let _ = unsafe { view.webview().GoBack() };
        }
    }

    fn forward(&self) {
        use wry::WebViewExtWindows;

        if let Some(view) = self.built() {
            // SAFETY: called on the thread that owns the webview.
            let _ = unsafe { view.webview().GoForward() };
        }
    }

    fn holds_keys(&self) -> bool {
        self.focused.get()
    }

    fn edit(&self, _edit: Edit) {}
}

#[cfg(target_os = "windows")]
fn closed(_view: &wry::WebView) -> bool {
    false
}

#[cfg(target_os = "linux")]
impl Page {
    fn configure<'a>(&self, builder: wry::WebViewBuilder<'a>) -> wry::WebViewBuilder<'a> {
        builder
    }

    /// GTK reports focus itself, so a press needs no script.
    fn attach(&self, view: &wry::WebView) {
        use gtk::{glib::Propagation, prelude::WidgetExt};
        use wry::WebViewExtUnix;

        let page = view.webview();
        let (got, lost) = (self.focused.clone(), self.focused.clone());
        let pressed = self.reports.clone();
        page.connect_focus_in_event(move |_, _| {
            got.set(true);
            let _ = pressed.try_send(Report::Pressed);
            Propagation::Proceed
        });
        page.connect_focus_out_event(move |_, _| {
            lost.set(false);
            Propagation::Proceed
        });
    }

    fn back(&self) {
        use webkit2gtk::WebViewExt;
        use wry::WebViewExtUnix;

        if let Some(view) = self.built() {
            view.webview().go_back();
        }
    }

    fn forward(&self) {
        use webkit2gtk::WebViewExt;
        use wry::WebViewExtUnix;

        if let Some(view) = self.built() {
            view.webview().go_forward();
        }
    }

    fn holds_keys(&self) -> bool {
        self.focused.get()
    }

    fn edit(&self, _edit: Edit) {}
}

#[cfg(target_os = "linux")]
fn closed(_view: &wry::WebView) -> bool {
    false
}

/// wry's webview on Linux is webkit2gtk, driven by GTK's main loop, which gpui
/// does not run.
#[cfg(target_os = "linux")]
mod gtk_loop {
    use gpui::{App, Window};
    use raw_window_handle::{
        HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle,
    };
    use std::{os::raw::c_ulong, time::Duration};

    /// How often GTK's pending events are dispatched. Page input, painting
    /// and callbacks wait for it.
    const PUMP: Duration = Duration::from_millis(8);

    /// Initializes GTK on X11 and starts pumping its events, once per process.
    /// Does nothing under Wayland, or when GTK is already initialized: the
    /// host runs the loop then.
    pub(super) fn start(window: &Window, cx: &mut App) {
        if gtk::is_initialized() || Parent::of(window).is_none() {
            return;
        }
        gtk::gdk::set_allowed_backends("x11");
        if let Err(error) = gtk::init() {
            tracing::warn!(%error, "webview: gtk");
            return;
        }
        let timers = cx.background_executor().clone();
        cx.foreground_executor()
            .spawn(async move {
                loop {
                    timers.timer(PUMP).await;
                    while gtk::events_pending() {
                        gtk::main_iteration_do(false);
                    }
                }
            })
            .detach();
    }

    /// gpui's X11 window, as the Xlib handle wry takes. gpui hands out an XCB
    /// one for the same window.
    pub(super) struct Parent(XlibWindowHandle);

    impl Parent {
        /// `None` under Wayland.
        pub(super) fn of(window: &Window) -> Option<Self> {
            match HasWindowHandle::window_handle(window).ok()?.as_raw() {
                RawWindowHandle::Xcb(handle) => Some(Self(XlibWindowHandle::new(c_ulong::from(
                    handle.window.get(),
                )))),
                _ => None,
            }
        }
    }

    impl HasWindowHandle for Parent {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            // SAFETY: an X window id, valid while gpui's window is.
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xlib(self.0)) })
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
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

    fn edit(&self, _edit: Edit) {}

    fn park(&self) {
        self.placed.set(None);
    }

    fn take_keys(&self) {}

    fn give_keys(&self) {}
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
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
