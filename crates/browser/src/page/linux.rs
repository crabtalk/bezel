use super::{Edit, Report};
use gpui::{App, Window};
use std::{cell::Cell, rc::Rc};
use wry::WebViewExtUnix;

mod gtk_loop;

pub(super) const BINDS_EDITS: bool = false;

pub(super) struct State {
    /// Whether the page holds keyboard focus, as GTK last reported.
    focused: Rc<Cell<bool>>,
}

impl State {
    pub(super) fn new(_cx: &App) -> Self {
        Self {
            focused: Rc::new(Cell::new(false)),
        }
    }

    /// GTK reports focus itself, so a press needs no script.
    pub(super) fn attach(&self, view: &wry::WebView, reports: &async_channel::Sender<Report>) {
        use gtk::{glib::Propagation, prelude::WidgetExt};

        let page = view.webview();
        let (got, lost) = (self.focused.clone(), self.focused.clone());
        let pressed = reports.clone();
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

    pub(super) fn parked(&self) {}

    pub(super) fn watch_keys(&self, _window: &Window) {}

    pub(super) fn holds_keys(&self, _view: &wry::WebView) -> bool {
        self.focused.get()
    }
}

pub(super) fn start(window: &Window, cx: &mut App) {
    gtk_loop::start(window, cx);
}

/// `None` under Wayland, or before GTK is initialized.
pub(super) fn build(
    builder: wry::WebViewBuilder<'_>,
    window: &Window,
) -> Option<wry::Result<wry::WebView>> {
    let parent = gtk_loop::Parent::of(window).filter(|_| gtk::is_initialized())?;
    Some(builder.build_as_child(&parent))
}

pub(super) fn edit(_edit: Edit) {}

pub(super) fn back(view: &wry::WebView) {
    use webkit2gtk::WebViewExt;

    view.webview().go_back();
}

pub(super) fn forward(view: &wry::WebView) {
    use webkit2gtk::WebViewExt;

    view.webview().go_forward();
}

pub(super) fn closed(_view: &wry::WebView) -> bool {
    false
}

pub(super) fn default_user_agent() -> Option<String> {
    None
}

pub(super) fn capture(
    _view: &wry::WebView,
    _done: impl FnOnce(Option<std::sync::Arc<gpui::RenderImage>>) + 'static,
) -> bool {
    false
}
