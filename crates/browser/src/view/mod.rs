use crate::{
    DataStore,
    host::Host,
    page::{Edit, Page, Present as _, Report},
};
use anyhow::{Result, ensure};
use gpui::{
    Action, AnyWindowHandle, App, Context, EventEmitter, FocusHandle, Focusable, Global,
    IntoElement, KeyBinding, Render, RenderImage, Subscription, Task, Window, actions, div,
    native_view::Viewport, prelude::*,
};
use serde::de::DeserializeOwned;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use theme::{TextStyle, Typeset as _};
static EVALUATION_ID: AtomicU64 = AtomicU64::new(1);

type EvaluationReplies =
    Rc<RefCell<HashMap<u64, async_channel::Sender<Result<String, EvalError>>>>>;

#[cfg(target_os = "linux")]
mod linux;

/// A retained page, built after its first paint. Hiding it preserves document
/// state; showing it in another window moves the same page there.
///
/// On macOS, GPUI composes the native view with its clips, overlays and materials.
/// [`Self::set_presentation`] fixes the logical viewport independently of the
/// displayed rectangle and controls whether native page input is enabled.
/// AppKit handles page input, including keyboard focus and pointer gestures.
///
/// Linux composes WPEPlatform buffers through GPUI. Windows composes live
/// WebView2 Graphics Capture frames through shared D3D textures.
pub struct WebView {
    page: Rc<Page>,
    evaluations: EvaluationReplies,
    location: Option<String>,
    title: String,
    loading: bool,
    /// Whether the page could step back, and forward, when it last moved.
    history: (bool, bool),
    /// The window [`Self::_focus`] listens in.
    focus_window: Option<AnyWindowHandle>,
    _focus: [Subscription; 2],
    _reports: Task<()>,
    pub error: Option<String>,
    #[cfg(target_os = "linux")]
    menu: ui::context_menu::ContextMenu<Self>,
    #[cfg(target_os = "linux")]
    menu_was_open: bool,
}

/// Fixed logical page viewport and input ownership for a native canvas frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Presentation {
    pub viewport: Viewport,
    pub interactive: bool,
}

/// What the page reports about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebViewEvent {
    /// The page's URL changed: a load, a redirect, or the page's own history
    /// move (`pushState`, `replaceState`, a fragment).
    Location(String),
    /// The page's title changed.
    Title(String),
    /// Whether [`WebView::back`] and [`WebView::forward`] would move the page
    /// changed.
    History {
        back: bool,
        forward: bool,
    },
    Load(LoadState),
    /// The page asked to open this URL in a window of its own: a
    /// `target="_blank"` link or `window.open`. No window opens and the page
    /// stays where it is; where the URL goes is the host's.
    NewWindow(String),
    /// The page logged, with a page built [`WebView::with_console`].
    Console(ConsoleMessage),
    /// The page backend or runtime failed.
    Error(String),
}

/// One `console` call, uncaught error or unhandled rejection in the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleMessage {
    pub level: ConsoleLevel,
    /// The logged values, each a string as is, an error as its stack, and
    /// anything else as JSON, joined by spaces.
    pub text: String,
    /// The URL of the frame that logged.
    pub source: String,
}

/// The `console` method a [`ConsoleMessage`] came from. Uncaught errors and
/// unhandled rejections are `Error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLevel {
    Debug,
    Log,
    Info,
    Warn,
    Error,
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
    JavaScript(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("no page to evaluate in"),
            Self::Timeout => f.write_str("script timed out"),
            Self::Script => f.write_str("script threw or returned no JSON value"),
            Self::Decode(error) => write!(f, "script value: {error}"),
            Self::JavaScript(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for EvalError {}

impl WebView {
    pub const HAS_INSPECTOR: bool = cfg!(all(
        feature = "inspector",
        any(target_os = "macos", target_os = "windows")
    ));

    pub fn can_evaluate(&self) -> bool {
        self.error.is_none() && self.page.ready()
    }

    pub fn new(url: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Page::start(window, cx);
        bind_edits(cx);
        let focus = cx.focus_handle();
        let subscriptions = Self::listen(&focus, window, cx);
        let (reports, received) = async_channel::unbounded();
        // Ends when the page drops, which drops every sender.
        let task = cx.spawn(async move |this, cx| {
            while let Ok(report) = received.recv().await {
                if this.update(cx, |this, cx| this.report(report, cx)).is_err() {
                    return;
                }
            }
        });
        let page = Rc::new(Page::new(url.into(), focus, reports, cx));
        #[cfg(target_os = "linux")]
        let _ = page.platform.view.set(cx.entity().downgrade());
        Self {
            evaluations: Rc::default(),
            page,
            error: None,
            #[cfg(target_os = "linux")]
            menu: ui::context_menu::ContextMenu::default(),
            #[cfg(target_os = "linux")]
            menu_was_open: false,
            location: None,
            title: String::new(),
            loading: false,
            history: (false, false),
            focus_window: Some(window.window_handle()),
            _focus: subscriptions,
            _reports: task,
        }
    }

    /// `None` restores a responsive viewport and page input.
    pub fn set_presentation(
        &mut self,
        presentation: Option<Presentation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        ensure!(
            Page::COMPOSITED && self.page.platform.available(window),
            "live browser composition is unavailable on this backend"
        );
        if self.page.presentation.replace(presentation) == presentation {
            return Ok(());
        }
        self.page
            .input
            .enabled
            .set(presentation.is_none_or(|presentation| presentation.interactive));
        if presentation.is_some_and(|presentation| !presentation.interactive) {
            self.page.give_keys();
            if self.page.focus.is_focused(window) {
                window.blur(cx);
            }
        }
        cx.notify();
        Ok(())
    }

    /// Focus listeners are per window.
    fn listen(
        focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> [Subscription; 2] {
        [
            cx.on_focus(focus, window, |this: &mut Self, window, _| {
                if this.page.window() == Some(window.window_handle()) {
                    this.page.watch_keys(window);
                    this.page.take_keys();
                }
            }),
            cx.on_blur(focus, window, |this: &mut Self, window, _| {
                if this.page.window() == Some(window.window_handle())
                    && (!Page::COMPOSITED || !this.page.focus.is_focused(window))
                {
                    this.page.give_keys();
                }
            }),
        ]
    }

    /// The URL the page last reported; `None` before its first report.
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Whether [`Self::back`] would move the page, as of its last load or
    /// history move. `false` before the first paint.
    pub fn can_go_back(&self) -> bool {
        self.history.0
    }

    /// Whether [`Self::forward`] would move the page, as of its last load or
    /// history move. `false` before the first paint.
    pub fn can_go_forward(&self) -> bool {
        self.history.1
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

    /// Reports the page's console as [`WebViewEvent::Console`]. Read at the
    /// first paint. Engine messages such as failed loads are not console
    /// calls and are not reported. In the inspector, every console message
    /// is attributed to the script that reports it.
    pub fn with_console(self) -> Self {
        self.page.console.set(true);
        self
    }

    /// Grants macOS file pages access to resources under `directory`.
    /// Set before the page's first paint.
    pub fn with_read_access(self, directory: impl Into<std::path::PathBuf>) -> Self {
        *self.page.read_access.borrow_mut() = Some(directory.into());
        self
    }

    /// The store the page is built with, in place of the platform's default.
    /// Read at the first paint. A page keeps the store it was built with: a
    /// different store takes a new `WebView`.
    pub fn with_data_store(self, store: DataStore) -> Self {
        *self.page.store.borrow_mut() = store;
        self
    }

    /// Builds the page inspectable. Read at the first paint. On macOS 13.3+
    /// Safari's Develop menu lists the page, and the page's context menu has
    /// Inspect Element.
    #[cfg(feature = "inspector")]
    pub fn with_inspector(self) -> Self {
        self.page.inspector.set(true);
        self
    }

    /// Opens the inspector of a page built with [`Self::with_inspector`].
    /// Does nothing before the first paint.
    #[cfg(feature = "inspector")]
    pub fn open_inspector(&self) {
        self.page.open_inspector();
    }

    /// Before the first paint, replaces the URL the page is built with.
    /// After it, navigates the page.
    pub fn load(&mut self, url: impl Into<String>) {
        self.error = None;
        self.page.load(url.into());
    }

    /// Takes the keys back from the page if it holds them, whatever gpui's
    /// focus is.
    pub fn release_keys(&self) {
        self.page.give_keys();
    }

    pub fn back(&mut self) {
        self.page.back();
    }

    pub fn forward(&mut self) {
        self.page.forward();
    }

    /// Copies the current page viewport into an owned BGRA image.
    /// Loading, failed and unrendered pages return an error.
    pub fn snapshot(&self, cx: &App) -> Task<Result<Arc<RenderImage>, String>> {
        if self.loading || self.error.is_some() {
            return Task::ready(Err(self
                .error
                .clone()
                .unwrap_or_else(|| "page is still loading".into())));
        }
        let (answer, answered) = async_channel::bounded(1);
        let timer = answer.clone();
        self.page.snapshot(Box::new(move |image| {
            let _ = answer.try_send(image);
        }));
        let expired = cx.background_executor().timer(Duration::from_secs(10));
        cx.foreground_executor()
            .spawn(async move {
                expired.await;
                let _ = timer.try_send(Err("page snapshot timed out".into()));
            })
            .detach();
        cx.foreground_executor().spawn(async move {
            answered
                .recv()
                .await
                .unwrap_or_else(|_| Err("page closed during snapshot".into()))
        })
    }

    pub fn reload(&mut self) {
        self.error = None;
        self.page.reload();
    }

    /// Reloads from the network, past the cache.
    pub fn reload_bypassing_cache(&mut self) {
        self.error = None;
        self.page.reload_bypassing_cache();
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

    /// Evaluates a main-frame expression and awaits its promise, if any.
    /// The result must have a JSON representation. Timeout stops waiting;
    /// it does not cancel JavaScript already running in the page.
    pub fn eval_async<T: DeserializeOwned + 'static>(
        &self,
        script: &str,
        timeout: Duration,
        cx: &App,
    ) -> Task<Result<T, EvalError>> {
        let id = EVALUATION_ID.fetch_add(1, Ordering::Relaxed);
        let (answer, answered) = async_channel::bounded(1);
        self.evaluations.borrow_mut().insert(id, answer.clone());
        let script = format!(
            r#"void (async () => {{
            const bridge = window.ipc ?? window.webkit?.messageHandlers?.ipc ?? window.chrome?.webview;
            let reply;
            try {{
                const value = await (
{script}
);
                const json = JSON.stringify(value ?? null);
                if (json === undefined) throw new Error('result has no JSON representation');
                reply = {{ evaluation: {id}, value: json }};
            }} catch (error) {{
                reply = {{ evaluation: {id}, error: String(error?.stack ?? error) }};
            }}
            bridge.postMessage(JSON.stringify(reply));
        }})();"#
        );
        let script = format!("(() => {{ {script}\n return true; }})()");
        let started = answer.clone();
        if !self.page.eval(&script, move |value| {
            if value != "true" {
                let _ = started.try_send(Err(EvalError::Script));
            }
        }) {
            self.evaluations.borrow_mut().remove(&id);
            return Task::ready(Err(EvalError::Unavailable));
        }
        let expired = cx.background_executor().timer(timeout);
        cx.foreground_executor()
            .spawn(async move {
                expired.await;
                let _ = answer.try_send(Err(EvalError::Timeout));
            })
            .detach();
        let evaluations = self.evaluations.clone();
        cx.foreground_executor().spawn(async move {
            let result = answered.recv().await;
            evaluations.borrow_mut().remove(&id);
            let json = result.map_err(|_| EvalError::Unavailable)??;
            serde_json::from_str(&json).map_err(EvalError::Decode)
        })
    }

    fn report(&mut self, report: Report, cx: &mut Context<Self>) {
        match report {
            Report::Evaluated { id, answer } => {
                if let Some(reply) = self.evaluations.borrow_mut().remove(&id) {
                    let _ = reply.try_send(answer.map_err(EvalError::JavaScript));
                }
            }
            #[cfg(target_os = "linux")]
            Report::Linux(event) => self.linux_event(event, cx),
            // Deferred: the window's handlers may update this view.
            Report::Pressed => {
                if self.page.holds_keys()
                    && let Some(window) = self.page.window()
                {
                    let focus = self.page.focus.clone();
                    App::defer(cx, move |cx| {
                        let _ = window.update(cx, |_, window, cx| window.focus(&focus, cx));
                    });
                }
            }
            #[cfg(target_os = "macos")]
            Report::Released(generation) => {
                if self.page.input.focus.get().generation == generation
                    && let Some(handle) = self.page.window()
                {
                    let page = self.page.clone();
                    App::defer(cx, move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            if page.input.focus.get().generation == generation
                                && page.focus.is_focused(window)
                            {
                                window.blur(cx);
                            }
                        });
                    });
                }
            }
            #[cfg(target_os = "macos")]
            Report::Presented(handle) => {
                let view = cx.entity().downgrade();
                App::defer(cx, move |cx| {
                    let _ = handle.update(cx, |_, window, cx| {
                        if let Some(view) = view.upgrade() {
                            view.update(cx, |this, cx| {
                                if this.page.window() == Some(handle)
                                    && this.focus_window != Some(handle)
                                {
                                    this._focus = Self::listen(&this.page.focus, window, cx);
                                    this.focus_window = Some(handle);
                                }
                            });
                        }
                    });
                });
            }
            Report::Key(keystroke) => {
                if let Some(window) = self.page.window() {
                    App::defer(cx, move |cx| {
                        let _ = window.update(cx, |_, window, cx| {
                            window.dispatch_keystroke(keystroke, cx);
                        });
                    });
                }
            }
            #[cfg(target_os = "windows")]
            Report::Frame => cx.notify(),
            #[cfg(target_os = "windows")]
            Report::Windows { epoch, report } => {
                if self
                    .page
                    .platform
                    .epoch
                    .load(std::sync::atomic::Ordering::Acquire)
                    == epoch
                {
                    self.report(*report, cx);
                }
            }
            #[cfg(target_os = "windows")]
            Report::Failed(error) => {
                self.page.platform.failed.set(true);
                self.loading = false;
                self.error = Some(error.clone());
                cx.emit(WebViewEvent::Error(error));
                cx.notify();
            }
            Report::Moved => {
                if let Some(url) = self.page.location() {
                    self.locate(url, cx);
                }
                self.step(cx);
            }
            Report::Load(state, url) => {
                self.locate(url, cx);
                self.loading = state == LoadState::Started;
                cx.emit(WebViewEvent::Load(state));
                self.step(cx);
            }
            // Live pages rewrite their title, some every second.
            Report::Title(title) => {
                if title != self.title {
                    self.title = title.clone();
                    cx.emit(WebViewEvent::Title(title));
                }
            }
            Report::Opened(url) => cx.emit(WebViewEvent::NewWindow(url)),
            Report::Console(message) => cx.emit(WebViewEvent::Console(message)),
            #[cfg(target_os = "macos")]
            Report::Still(still) => {
                self.page.captured(still);
                cx.notify();
            }
            Report::Built => cx.notify(),
        }
    }

    fn step(&mut self, cx: &mut Context<Self>) {
        let history = self.page.history();
        if history != self.history {
            self.history = history;
            let (back, forward) = history;
            cx.emit(WebViewEvent::History { back, forward });
        }
    }

    fn locate(&mut self, url: String, cx: &mut Context<Self>) {
        *self.page.url.borrow_mut() = url.clone();
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_window != Some(window.window_handle()) {
            Page::start(window, cx);
            self._focus = Self::listen(&self.page.focus, window, cx);
            self.focus_window = Some(window.window_handle());
        }
        if self.page.focus.is_focused(window) {
            self.page.watch_keys(window);
        }
        let content = match &self.error {
            Some(error) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .child(
                    div()
                        .text_style(TextStyle::Body)
                        .max_w(gpui::px(560.))
                        .child(error.clone()),
                )
                .into_any_element(),
            None => Host {
                surface: self.page.clone(),
            }
            .into_any_element(),
        };
        #[cfg(target_os = "linux")]
        let menu = {
            if self.menu_was_open && !self.menu.is_open() {
                self.page
                    .platform
                    .command(serde_json::json!({"cmd":"dismiss-menu"}));
            }
            self.menu_was_open = self.menu.is_open();
            self.menu
                .render("browser-page-menu", |this| &mut this.menu, window, cx)
        };
        div()
            .size_full()
            .key_context(KEY_CONTEXT)
            .on_action(on_edit::<Copy>(Edit::Copy, cx))
            .on_action(on_edit::<Cut>(Edit::Cut, cx))
            .on_action(on_edit::<Paste>(Edit::Paste, cx))
            .on_action(on_edit::<SelectAll>(Edit::SelectAll, cx))
            .on_action(on_edit::<Undo>(Edit::Undo, cx))
            .on_action(on_edit::<Redo>(Edit::Redo, cx))
            .child(content)
            .children({
                #[cfg(target_os = "linux")]
                {
                    menu
                }
                #[cfg(not(target_os = "linux"))]
                {
                    None::<gpui::AnyElement>
                }
            })
    }
}

actions!(webview, [Copy, Cut, Paste, SelectAll, Undo, Redo]);

const KEY_CONTEXT: &str = "WebView";

fn on_edit<A: Action>(
    edit: Edit,
    cx: &mut Context<WebView>,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |this, _: &A, window, cx| {
        if this.page.window() == Some(window.window_handle()) {
            #[cfg(target_os = "linux")]
            this.page.platform.share_clipboard(cx);
            this.page.edit(edit);
        } else {
            cx.propagate();
        }
    })
}

/// Binds the edit keys in the view's context, once per app.
fn bind_edits(cx: &mut App) {
    struct Bound;
    impl Global for Bound {}

    if !Page::BINDS_EDITS || cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    let context = Some(KEY_CONTEXT);
    let is_mac = cx.platform_info().is_mac();
    cx.bind_keys([
        KeyBinding::new(if is_mac { "cmd-c" } else { "ctrl-c" }, Copy, context),
        KeyBinding::new(if is_mac { "cmd-x" } else { "ctrl-x" }, Cut, context),
        KeyBinding::new(if is_mac { "cmd-v" } else { "ctrl-v" }, Paste, context),
        KeyBinding::new(if is_mac { "cmd-a" } else { "ctrl-a" }, SelectAll, context),
        KeyBinding::new(if is_mac { "cmd-z" } else { "ctrl-z" }, Undo, context),
        KeyBinding::new(
            if is_mac {
                "cmd-shift-z"
            } else {
                "ctrl-shift-z"
            },
            Redo,
            context,
        ),
    ]);
}
