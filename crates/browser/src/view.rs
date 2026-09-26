use crate::{
    host::Host,
    page::{Edit, Page, Report},
};
use gpui::{
    Action, App, Context, EventEmitter, FocusHandle, Focusable, Global, IntoElement, KeyBinding,
    Render, Subscription, Task, Window, actions, div, prelude::*,
};
use serde::de::DeserializeOwned;
use std::{fmt, rc::Rc, time::Duration};

/// A webview showing one page.
///
/// The page is built the first time the view is painted, in the window it is
/// painted in, and stays in that window. It sits at the element's bounds in
/// every frame the element is painted and is parked in every frame it is not.
/// A parked page stays loaded.
///
/// The page takes keys while the view's focus handle is focused, and a press
/// in the page focuses the handle. On macOS, key equivalents (cmd or ctrl
/// held) reach gpui's key dispatch before the page sees them. On Windows, a
/// key with ctrl or alt held, or a function key, goes to gpui's key dispatch
/// in place of the page when the keymap binds it in the view's key context.
///
/// gpui elements behind the page are not hovered, and gpui's cursor over the
/// page is the arrow. On macOS the page sets the cursor over itself.
///
/// In a frame where a [`ui::cover`] recorded after the view overlaps it, the
/// page is parked. On macOS a still of the page, taken while it is still up,
/// is painted in its place; until the still arrives the page stays over the
/// cover. Elsewhere the place is left empty.
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
        Page::start(window, cx);
        bind_edits(cx);
        let focus = cx.focus_handle();
        let subscriptions = [
            cx.on_focus(&focus, window, |this: &mut Self, window, _| {
                this.page.watch_keys(window);
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
            page: Rc::new(Page::new(url.into(), focus, reports, cx)),
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
            Report::Key(keystroke) => {
                window.dispatch_keystroke(keystroke, cx);
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
            Report::Still(still) => {
                self.page.captured(still);
                cx.notify();
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.page.focus.is_focused(window) {
            self.page.watch_keys(window);
        }
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

    if !Page::BINDS_EDITS || cx.has_global::<Bound>() {
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
