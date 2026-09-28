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

    /// GTK reports focus, and a press with it.
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

/// Store identifiers are macOS's.
pub(super) fn store<'a>(
    builder: wry::WebViewBuilder<'a>,
    _identifier: Option<[u8; 16]>,
) -> wry::WebViewBuilder<'a> {
    builder
}

/// Moves the X window wry built the page in under `window`. `false` under
/// Wayland.
pub(super) fn reparent(view: &wry::WebView, window: &Window) -> bool {
    use gtk::prelude::WidgetExt;

    let Some(parent) = gtk_loop::Parent::of(window) else {
        return false;
    };
    let Some(container) = view.webview().toplevel().and_then(|top| top.window()) else {
        return false;
    };
    let Some(parent) = parent.gdk_window() else {
        return false;
    };
    container.reparent(&parent, 0, 0);
    true
}

pub(super) fn give_keys(view: &wry::WebView) {
    let _ = view.focus_parent();
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

pub(super) fn reload_bypassing_cache(view: &wry::WebView) {
    use webkit2gtk::WebViewExt;

    view.webview().reload_bypass_cache();
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

/// Clears every kind of data from the store `store` names. `false` before GTK
/// is initialized.
pub(super) fn clear_store(store: &crate::DataStore, done: impl FnOnce(bool) + Send + 'static) {
    use webkit2gtk::{WebContextExt as _, WebsiteDataManagerExtManual as _};
    use wry::WebContextExt as _;

    if !gtk::is_initialized() {
        done(false);
        return;
    }
    super::shared_context(store.directory.clone(), |context| {
        let Some(manager) = context.context().website_data_manager() else {
            done(false);
            return;
        };
        manager.clear(
            webkit2gtk::WebsiteDataTypes::ALL,
            gtk::glib::TimeSpan::from_seconds(0),
            None::<&webkit2gtk::gio::Cancellable>,
            move |result| done(result.is_ok()),
        );
    });
}
