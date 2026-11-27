use crate::{ConsoleMessage, DataStore, LoadState, Presentation, host::Surface};
use gpui::{
    AnyWindowHandle, App, Bounds, FocusHandle, Hitbox, Keystroke, Pixels, RenderImage, Window,
    native_view::NativeViewState,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

#[cfg_attr(target_os = "macos", path = "macos/mod.rs")]
#[cfg_attr(target_os = "windows", path = "windows/mod.rs")]
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), path = "stub.rs")]
pub(crate) mod platform;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod script;

pub(crate) type SnapshotDone = Box<dyn FnOnce(Result<Arc<RenderImage>, String>) + Send>;

pub(crate) trait Present {
    fn available(&self, window: &Window) -> bool {
        window.native_surfaces.views
    }
    const COMPOSITED: bool = false;
    fn paint(
        &self,
        _page: &Rc<Page>,
        _bounds: Bounds<Pixels>,
        _hitbox: &Hitbox,
        _owner: u64,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

/// Sent from the page's callbacks to the view.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub(crate) enum Report {
    Pressed,
    #[cfg(target_os = "windows")]
    Frame,
    #[cfg(target_os = "windows")]
    Windows {
        epoch: u64,
        report: Box<Report>,
    },
    #[cfg(target_os = "windows")]
    Failed(String),
    #[cfg(target_os = "macos")]
    Released(u64),
    #[cfg(target_os = "macos")]
    Presented(AnyWindowHandle),
    /// A gesture magnified the page; the level is read back from the page.
    #[cfg(target_os = "macos")]
    Magnified,
    /// A key the page took that the keymap binds.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Key(Keystroke),
    /// The page moved its own history; its URL is read back from the page.
    Moved,
    Load(LoadState, String),
    Title(String),
    /// The page asked for a window of its own, for this URL.
    Opened(String),
    Console(ConsoleMessage),
    Evaluated {
        id: u64,
        answer: Result<String, String>,
    },
    /// A still of the page, taken for a cover; `None` if the capture failed.
    #[cfg(target_os = "macos")]
    Still(Option<Arc<RenderImage>>),
    /// The page was built, or failed to be, after the paint that asked for it.
    Built,
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Unavailable,
}

impl Report {
    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    pub(crate) fn evaluated(body: &str) -> Option<Self> {
        #[derive(serde::Deserialize)]
        struct Evaluation {
            evaluation: u64,
            value: Option<String>,
            error: Option<String>,
        }
        let evaluation: Evaluation = serde_json::from_str(body).ok()?;
        let answer = match (evaluation.value, evaluation.error) {
            (Some(value), None) => Ok(value),
            (None, Some(error)) => Err(error),
            _ => return None,
        };
        Some(Self::Evaluated {
            id: evaluation.evaluation,
            answer,
        })
    }
}

/// Whether something is painted over the page.
enum Cover {
    Off,
    /// The page is still up while its still is taken. A parked page captures
    /// blank.
    Capturing,
    /// The page is parked, and the still, if any, is painted in its place.
    On(Option<Arc<RenderImage>>),
}

/// Edits a WKWebView takes as responder actions (`copy:`, `paste:`) and only
/// through them: without an Edit menu, cmd-c in the page copies nothing.
#[derive(Clone, Copy)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) enum Edit {
    Copy,
    Cut,
    Paste,
    SelectAll,
    Undo,
    Redo,
}

pub(crate) struct Page {
    /// What the page is built with; unread once it is built.
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    pub(crate) url: RefCell<String>,
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    pub(crate) user_agent: RefCell<Option<String>>,
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    pub(crate) store: RefCell<DataStore>,
    pub(crate) read_access: RefCell<Option<std::path::PathBuf>>,
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    pub(crate) console: Cell<bool>,
    #[cfg(feature = "inspector")]
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    pub(crate) inspector: Cell<bool>,
    /// What the page is magnified to while it is not built, and is built at.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) magnification: Cell<f64>,
    /// Whether a pinch magnifies the page; read when it is built.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) pinch: Cell<bool>,
    #[cfg(target_os = "macos")]
    view: std::cell::OnceCell<Option<wry::WebView>>,
    /// A build is on its way; see [`Page::request`].
    #[cfg(target_os = "macos")]
    building: Cell<bool>,
    /// Where the page last sat; `None` before the first paint and while parked.
    pub(crate) placed: Cell<Option<Bounds<Pixels>>>,
    pub(crate) presentation: Cell<Option<Presentation>>,
    pub(crate) input: Rc<NativeViewState>,
    /// The window the page is in; `None` before the first paint.
    window: Cell<Option<AnyWindowHandle>>,
    owner: Cell<u64>,
    cover: RefCell<Cover>,
    /// A still painted before the page was uncovered, to free from the atlas.
    dropped: RefCell<Option<Arc<RenderImage>>>,
    pub(crate) focus: FocusHandle,
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    reports: async_channel::Sender<Report>,
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    pub(crate) platform: platform::State,
}

impl Page {
    /// Whether the view binds the edit keys to [`Edit`]s.
    pub(crate) const BINDS_EDITS: bool = platform::BINDS_EDITS;
    pub(crate) const COMPOSITED: bool = <platform::State as Present>::COMPOSITED;

    pub(crate) fn new(
        url: String,
        focus: FocusHandle,
        reports: async_channel::Sender<Report>,
        cx: &App,
    ) -> Self {
        let input = Rc::new(NativeViewState::default());
        input.enabled.set(true);
        Self {
            url: RefCell::new(url),
            user_agent: RefCell::new(None),
            store: RefCell::new(DataStore::default()),
            read_access: RefCell::new(None),
            console: Cell::new(false),
            #[cfg(feature = "inspector")]
            inspector: Cell::new(false),
            magnification: Cell::new(1.),
            pinch: Cell::new(false),
            #[cfg(target_os = "macos")]
            view: std::cell::OnceCell::new(),
            #[cfg(target_os = "macos")]
            building: Cell::new(false),
            placed: Cell::new(None),
            presentation: Cell::new(None),
            input: input.clone(),
            window: Cell::new(None),
            owner: Cell::new(0),
            cover: RefCell::new(Cover::Off),
            dropped: RefCell::new(None),
            focus,
            platform: platform::State::new(cx, input, reports.clone()),
            reports,
        }
    }
}

impl Surface for Page {
    const COMPOSITED: bool = Page::COMPOSITED;

    fn accepts_input(&self) -> bool {
        self.presentation
            .get()
            .is_none_or(|presentation| presentation.interactive)
    }

    fn paint(
        this: &Rc<Self>,
        bounds: Bounds<Pixels>,
        hitbox: &Hitbox,
        owner: u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        this.platform.paint(this, bounds, hitbox, owner, window, cx);
    }

    fn focus(&self) -> Option<&FocusHandle> {
        self.accepts_input().then_some(&self.focus)
    }

    fn owner(&self) -> &Cell<u64> {
        &self.owner
    }

    fn place(this: &Rc<Self>, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        Page::place(this, bounds, window, cx);
    }

    fn park(&self) {
        Page::park(self);
    }

    fn cover(&self) {
        if !matches!(*self.cover.borrow(), Cover::Off) {
            return;
        }
        if self.placed.get().is_some() && self.capture() {
            *self.cover.borrow_mut() = Cover::Capturing;
        } else {
            Page::park(self);
            *self.cover.borrow_mut() = Cover::On(None);
        }
    }

    fn still(&self) -> Option<Arc<RenderImage>> {
        match &*self.cover.borrow() {
            Cover::On(still) => still.clone(),
            Cover::Off | Cover::Capturing => None,
        }
    }

    fn take_dropped(&self) -> Option<Arc<RenderImage>> {
        self.dropped.take()
    }
}

impl Page {
    /// Parks the page for the still if it is still covered.
    #[cfg(target_os = "macos")]
    pub(crate) fn captured(&self, still: Option<Arc<RenderImage>>) {
        if matches!(*self.cover.borrow(), Cover::Capturing) {
            Page::park(self);
            *self.cover.borrow_mut() = Cover::On(still);
        }
    }

    /// The window the page is in; `None` before the first paint.
    pub(crate) fn window(&self) -> Option<AnyWindowHandle> {
        self.window.get()
    }

    #[cfg(not(target_os = "windows"))]
    fn uncover(&self) {
        if let Cover::On(still) = self.cover.replace(Cover::Off) {
            *self.dropped.borrow_mut() = still;
        }
    }
}

#[cfg(target_os = "macos")]
impl Page {
    /// Where a parked page waits. Hidden alone, a WKWebView stays registered as
    /// a drag destination over its last rect and takes every drag that crosses
    /// it.
    const PARKED: f64 = -20_000.0;

    pub(crate) fn ready(&self) -> bool {
        self.view.get().is_some_and(Option::is_some)
    }

    fn place(this: &Rc<Self>, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        this.uncover();
        let Some(view) = this.view.get() else {
            Self::request(this, bounds, window, cx);
            return;
        };
        let Some(view) = view else { return };
        if this.window.get() != Some(window.window_handle()) {
            this.give_keys();
            this.platform.parked();
            if !platform::reparent(view, window) {
                return;
            }
            this.window.set(Some(window.window_handle()));
            this.placed.set(None);
        }
        let placed = this.placed.get();
        if placed == Some(bounds) {
            return;
        }
        let _ = view.set_bounds(rect(bounds));
        let _ = view.set_visible(true);
        this.placed.set(Some(bounds));
        if placed.is_none() && this.focus.is_focused(window) {
            this.take_keys();
        }
    }

    /// Builds the page in a task of its own, between gpui updates, and reports
    /// [`Report::Built`] when it lands. The page is built hidden and shown by
    /// the next [`Page::place`].
    fn request(this: &Rc<Self>, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if this.building.replace(true) {
            return;
        }
        let page = this.clone();
        window
            .spawn(cx, async move |cx| {
                let Ok((handle, parent)) =
                    cx.update(|window, _| (window.window_handle(), Parent::of(window)))
                else {
                    return;
                };
                let view = parent.and_then(|parent| page.build(bounds, &parent));
                if view.is_some() {
                    page.window.set(Some(handle));
                }
                let _ = page.view.set(view);
                let _ = page.reports.try_send(Report::Built);
            })
            .detach();
    }

    fn build(&self, bounds: Bounds<Pixels>, parent: &Parent) -> Option<wry::WebView> {
        let url = self.url.borrow();
        let (ipc, loads, titles, opened) = (
            self.reports.clone(),
            self.reports.clone(),
            self.reports.clone(),
            self.reports.clone(),
        );
        let store = self.store.borrow().clone();
        let directory = store.directory.filter(|_| !store.incognito);
        let view = with_context(directory, |context| {
            let builder = match context {
                Some(context) => wry::WebViewBuilder::new_with_web_context(context),
                None => wry::WebViewBuilder::new(),
            };
            let builder = builder
                .with_incognito(store.incognito)
                .with_url(
                    if self.read_access.borrow().is_some() && url.starts_with("file:") {
                        "about:blank"
                    } else {
                        url.as_str()
                    },
                )
                .with_bounds(rect(bounds))
                .with_visible(false)
                .with_initialization_script(script::MOVED)
                .with_ipc_handler(move |request| {
                    let report = match request.body().as_str() {
                        "moved" => Report::Moved,
                        body => match Report::evaluated(body).or_else(|| {
                            logged(body, request.uri().to_string()).map(Report::Console)
                        }) {
                            Some(report) => report,
                            None => return,
                        },
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
                .with_new_window_req_handler(move |url, _| {
                    let _ = opened.try_send(Report::Opened(url));
                    wry::NewWindowResponse::Deny
                });
            let user_agent = self
                .user_agent
                .borrow()
                .clone()
                .or_else(platform::default_user_agent);
            let builder = match user_agent {
                Some(user_agent) => builder.with_user_agent(user_agent),
                None => builder,
            };
            let builder = if self.console.get() {
                builder.with_initialization_script_for_main_only(script::CONSOLE, false)
            } else {
                builder
            };
            #[cfg(feature = "inspector")]
            let builder = builder.with_devtools(self.inspector.get());
            let builder = platform::store(builder, store.identifier);
            builder
                .build_as_child(parent)
                .inspect_err(|error| tracing::warn!(%error, url = %url, "webview: build"))
                .ok()
        })?;
        self.platform.attach(&view, &self.reports);
        platform::allow_magnification(&view, self.pinch.get());
        platform::magnify(&view, self.magnification.get());
        if self.read_access.borrow().is_some()
            && url.starts_with("file:")
            && let Err(error) =
                platform::navigate(&view, &url, self.read_access.borrow().as_deref())
        {
            tracing::warn!(%error, "load local page");
        }
        Some(view)
    }

    fn built(&self) -> Option<&wry::WebView> {
        self.view.get()?.as_ref()
    }

    pub(crate) fn snapshot(&self, done: SnapshotDone) {
        let Some(view) = self.built() else {
            done(Err("page has not rendered".into()));
            return;
        };
        platform::capture(view, move |image| {
            done(image.ok_or_else(|| "WebKit snapshot unavailable".into()))
        });
    }

    /// Whether a still was asked for; it arrives as [`Report::Still`].
    fn capture(&self) -> bool {
        let Some(view) = self.built() else {
            return false;
        };
        let reports = self.reports.clone();
        platform::capture(view, move |still| {
            let _ = reports.try_send(Report::Still(still));
        })
    }

    pub(crate) fn start(window: &Window, cx: &mut App) {
        platform::start(window, cx);
    }

    pub(crate) fn watch_keys(&self, window: &Window) {
        self.platform.watch_keys(window);
    }

    pub(crate) fn back(&self) {
        if let Some(view) = self.built() {
            platform::back(view);
        }
    }

    pub(crate) fn forward(&self) {
        if let Some(view) = self.built() {
            platform::forward(view);
        }
    }

    /// Whether the page can step back and forward; `(false, false)` before
    /// it is built.
    pub(crate) fn history(&self) -> (bool, bool) {
        self.built().map_or((false, false), |view| {
            (
                view.can_go_back().unwrap_or(false),
                view.can_go_forward().unwrap_or(false),
            )
        })
    }

    /// Whether the page, or a view inside it, holds keyboard focus.
    pub(crate) fn holds_keys(&self) -> bool {
        self.built()
            .is_some_and(|view| self.platform.holds_keys(view))
    }

    pub(crate) fn edit(&self, edit: Edit) {
        if self.holds_keys() {
            platform::edit(edit);
        }
    }

    pub(crate) fn load(&self, url: String) {
        match self.built() {
            Some(view) => {
                let _ = platform::navigate(view, &url, self.read_access.borrow().as_deref());
            }
            None => *self.url.borrow_mut() = url,
        }
    }

    pub(crate) fn reload(&self) {
        if let Some(view) = self.built() {
            let _ = view.reload();
        }
    }

    pub(crate) fn reload_bypassing_cache(&self) {
        if let Some(view) = self.built() {
            platform::reload_bypassing_cache(view);
        }
    }

    pub(crate) fn location(&self) -> Option<String> {
        self.built()?.url().ok()
    }

    pub(crate) fn magnification(&self) -> f64 {
        self.built()
            .map_or(self.magnification.get(), platform::magnification)
    }

    pub(crate) fn magnify(&self, level: f64) {
        self.magnification.set(level);
        if let Some(view) = self.built() {
            platform::magnify(view, level);
        }
    }

    #[cfg(feature = "inspector")]
    pub(crate) fn open_inspector(&self) {
        if let Some(view) = self.built() {
            platform::open_inspector(view);
        }
    }

    /// Whether the script was handed to the page.
    pub(crate) fn eval(&self, script: &str, done: impl Fn(String) + Send + 'static) -> bool {
        self.built()
            .is_some_and(|view| view.evaluate_script_with_callback(script, done).is_ok())
    }

    pub(crate) fn take_keys(&self) {
        let Some(view) = self.built() else {
            return;
        };
        if Self::COMPOSITED || (self.placed.get().is_some() && !platform::closed(view)) {
            self.platform.take_keys(view);
        }
    }

    /// Hands keyboard focus back to gpui's view if the page holds it.
    pub(crate) fn give_keys(&self) {
        if let Some(view) = self.built()
            && (Self::COMPOSITED || self.holds_keys())
        {
            self.platform.give_keys(view);
        }
    }

    fn park(&self) {
        let Some(view) = self.built() else {
            return;
        };
        let Some(bounds) = self.placed.take() else {
            return;
        };
        if Self::COMPOSITED {
            self.platform.parked();
            let _ = view.set_visible(false);
            return;
        }
        self.give_keys();
        self.platform.parked();
        let _ = view.set_visible(false);
        if platform::closed(view) {
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

/// The native window a page is built in, by its raw handle: the build runs
/// while gpui's `Window` is not borrowed.
#[cfg(target_os = "macos")]
struct Parent(raw_window_handle::RawWindowHandle);

#[cfg(target_os = "macos")]
impl Parent {
    fn of(window: &Window) -> Option<Self> {
        raw_window_handle::HasWindowHandle::window_handle(window)
            .ok()
            .map(|handle| Self(handle.as_raw()))
    }
}

#[cfg(target_os = "macos")]
impl raw_window_handle::HasWindowHandle for Parent {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: read from a gpui window on this thread in the update just
        // before the build. On macOS nothing runs between the two. On Windows
        // the messages WebView2's start pumps can close the window, and a
        // destroyed HWND fails the build.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(self.0) })
    }
}

/// A console message the [`script::CONSOLE`] script posted from `source`.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn logged(body: &str, source: String) -> Option<ConsoleMessage> {
    #[derive(serde::Deserialize)]
    struct Logged {
        level: String,
        text: String,
    }
    let Logged { level, text } = serde_json::from_str(body).ok()?;
    use crate::ConsoleLevel;
    let level = match level.as_str() {
        "debug" => ConsoleLevel::Debug,
        "log" => ConsoleLevel::Log,
        "info" => ConsoleLevel::Info,
        "warn" => ConsoleLevel::Warn,
        "error" => ConsoleLevel::Error,
        _ => return None,
    };
    Some(ConsoleMessage {
        level,
        text,
        source,
    })
}

/// Runs `build` with the context for `directory`, or with none, which has wry
/// make the page its own.
#[cfg(target_os = "macos")]
fn with_context<T>(
    directory: Option<std::path::PathBuf>,
    build: impl FnOnce(Option<&mut wry::WebContext>) -> T,
) -> T {
    match directory {
        Some(directory) => shared_context(Some(directory), |context| build(Some(context))),
        None => build(None),
    }
}

/// Runs `f` with the context for `directory`, `None` being the platform's
/// default store. A context lives as long as the thread: pages built with it
/// hold on to it.
#[cfg(target_os = "macos")]
fn shared_context<T>(
    directory: Option<std::path::PathBuf>,
    f: impl FnOnce(&mut wry::WebContext) -> T,
) -> T {
    thread_local! {
        static CONTEXTS: RefCell<
            std::collections::HashMap<Option<std::path::PathBuf>, wry::WebContext>,
        > = RefCell::default();
    }
    CONTEXTS.with_borrow_mut(|contexts| {
        let context = contexts
            .entry(directory.clone())
            .or_insert_with(|| wry::WebContext::new(directory));
        f(context)
    })
}

/// Clears `store`, calling `done` with whether it was cleared.
pub(crate) fn clear_store(store: &DataStore, done: impl FnOnce(bool) + Send + 'static) {
    platform::clear_store(store, done);
}

/// Reports what `store` holds, calling `done` with `None` where the platform
/// cannot tell.
pub(crate) fn store_usage(
    store: &DataStore,
    done: impl FnOnce(Option<crate::Usage>) + Send + 'static,
) {
    platform::store_usage(store, done);
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

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::logged;
    use crate::ConsoleLevel;

    #[test]
    fn reads_a_posted_message() {
        let message = logged(
            r#"{"level":"warn","text":"low disk"}"#,
            "https://a.test/".into(),
        );
        let message = message.expect("a console message");
        assert_eq!(message.level, ConsoleLevel::Warn);
        assert_eq!(message.text, "low disk");
        assert_eq!(message.source, "https://a.test/");
    }

    #[test]
    fn ignores_what_is_not_one() {
        assert!(logged("moved", String::new()).is_none());
        assert!(logged(r#"{"level":"trace","text":""}"#, String::new()).is_none());
    }
}

#[cfg(test)]
#[path = "../../tests/unit/evaluation.rs"]
mod evaluation_tests;
