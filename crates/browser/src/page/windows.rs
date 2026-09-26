use super::{Edit, Report};
use gpui::{App, Window};
use std::{cell::Cell, rc::Rc};
use wry::WebViewExtWindows;

mod accelerator;

pub(super) const BINDS_EDITS: bool = false;

pub(super) struct State {
    /// Whether the page holds keyboard focus, as WebView2 last reported.
    focused: Rc<Cell<bool>>,
    keys: Rc<accelerator::Keys>,
}

impl State {
    pub(super) fn new(cx: &App) -> Self {
        Self {
            focused: Rc::new(Cell::new(false)),
            keys: Rc::new(accelerator::Keys::new(cx)),
        }
    }

    /// WebView2 reports focus itself, so a press needs no script.
    pub(super) fn attach(&self, view: &wry::WebView, reports: &async_channel::Sender<Report>) {
        use webview2_com::FocusChangedEventHandler;

        let controller = view.controller();
        accelerator::attach(&controller, self.keys.clone(), reports.clone());
        let (got, lost) = (self.focused.clone(), self.focused.clone());
        let pressed = reports.clone();
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

    pub(super) fn parked(&self) {}

    /// Call while the view's focus handle is focused.
    pub(super) fn watch_keys(&self, window: &Window) {
        self.keys.watch(window);
    }

    pub(super) fn holds_keys(&self, _view: &wry::WebView) -> bool {
        self.focused.get()
    }
}

pub(super) fn start(_window: &Window, _cx: &mut App) {}

pub(super) fn build(
    builder: wry::WebViewBuilder<'_>,
    window: &Window,
) -> Option<wry::Result<wry::WebView>> {
    Some(builder.build_as_child(window))
}

pub(super) fn edit(_edit: Edit) {}

pub(super) fn back(view: &wry::WebView) {
    // SAFETY: called on the thread that owns the webview.
    let _ = unsafe { view.webview().GoBack() };
}

pub(super) fn forward(view: &wry::WebView) {
    // SAFETY: called on the thread that owns the webview.
    let _ = unsafe { view.webview().GoForward() };
}

pub(super) fn closed(_view: &wry::WebView) -> bool {
    false
}

pub(super) fn default_user_agent() -> Option<String> {
    None
}
