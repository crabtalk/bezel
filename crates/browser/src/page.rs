use crate::{LoadState, host::Surface};
use gpui::{App, Bounds, FocusHandle, Keystroke, Pixels, Window};
use std::cell::{Cell, RefCell};

#[cfg_attr(target_os = "macos", path = "page/macos.rs")]
#[cfg_attr(target_os = "windows", path = "page/windows.rs")]
#[cfg_attr(target_os = "linux", path = "page/linux.rs")]
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
    path = "page/stub.rs"
)]
mod platform;

/// Sent from the page's callbacks to the view.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
    allow(dead_code)
)]
pub(crate) enum Report {
    Pressed,
    /// A key the page took that the keymap binds.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Key(Keystroke),
    /// The page moved its own history; its URL is read back from the page.
    Moved,
    Load(LoadState, String),
    Title(String),
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
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        allow(dead_code)
    )]
    url: RefCell<String>,
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        allow(dead_code)
    )]
    pub(crate) user_agent: RefCell<Option<String>>,
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    view: std::cell::OnceCell<Option<wry::WebView>>,
    /// Where the page last sat; `None` before the first paint and while parked.
    placed: Cell<Option<Bounds<Pixels>>>,
    owner: Cell<u64>,
    pub(crate) focus: FocusHandle,
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        allow(dead_code)
    )]
    reports: async_channel::Sender<Report>,
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        allow(dead_code)
    )]
    platform: platform::State,
}

impl Page {
    /// Whether the view binds the edit keys to [`Edit`]s.
    pub(crate) const BINDS_EDITS: bool = platform::BINDS_EDITS;

    pub(crate) fn new(
        url: String,
        focus: FocusHandle,
        reports: async_channel::Sender<Report>,
        cx: &App,
    ) -> Self {
        Self {
            url: RefCell::new(url),
            user_agent: RefCell::new(None),
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            view: std::cell::OnceCell::new(),
            placed: Cell::new(None),
            owner: Cell::new(0),
            focus,
            reports,
            platform: platform::State::new(cx),
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
        let user_agent = self
            .user_agent
            .borrow()
            .clone()
            .or_else(platform::default_user_agent);
        let builder = match user_agent {
            Some(user_agent) => builder.with_user_agent(user_agent),
            None => builder,
        };
        let view = platform::build(builder, window)?
            .inspect_err(|error| tracing::warn!(%error, url = %url, "webview: build"))
            .ok()?;
        self.platform.attach(&view, &self.reports);
        Some(view)
    }

    fn built(&self) -> Option<&wry::WebView> {
        self.view.get()?.as_ref()
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
                let _ = view.load_url(&url);
            }
            None => *self.url.borrow_mut() = url,
        }
    }

    pub(crate) fn reload(&self) {
        if let Some(view) = self.built() {
            let _ = view.reload();
        }
    }

    pub(crate) fn location(&self) -> Option<String> {
        self.built()?.url().ok()
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
        if self.placed.get().is_some() && !platform::closed(view) {
            let _ = view.focus();
        }
    }

    /// Hands keyboard focus back to gpui's view if the page holds it.
    pub(crate) fn give_keys(&self) {
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
