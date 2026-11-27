use crate::page::{Edit, Page, Present, Report};
use gpui::{App, Bounds, Pixels, Window, native_view::NativeViewState};
use std::rc::Rc;

pub(super) const BINDS_EDITS: bool = false;

pub(crate) struct State;

impl State {
    pub(super) fn new(
        _cx: &App,
        _input: Rc<NativeViewState>,
        reports: async_channel::Sender<Report>,
    ) -> Self {
        let _ = reports.try_send(Report::Unavailable);
        Self
    }
}

impl Page {
    pub(crate) fn snapshot(&self, done: crate::page::SnapshotDone) {
        done(Err("page snapshots are unavailable on this platform".into()));
    }

    pub(crate) fn ready(&self) -> bool {
        false
    }

    pub(super) fn place(this: &Rc<Self>, bounds: Bounds<Pixels>, window: &Window, _cx: &mut App) {
        this.uncover();
        this.window.set(Some(window.window_handle()));
        this.placed.set(Some(bounds));
    }

    pub(super) fn capture(&self) -> bool {
        false
    }

    pub(super) fn park(&self) {
        self.placed.set(None);
    }

    pub(crate) fn start(_window: &Window, _cx: &mut App) {}

    pub(crate) fn watch_keys(&self, _window: &Window) {}

    pub(crate) fn load(&self, url: String) {
        *self.url.borrow_mut() = url;
        self.reload();
    }

    pub(crate) fn back(&self) {}

    pub(crate) fn forward(&self) {}

    pub(crate) fn history(&self) -> (bool, bool) {
        (false, false)
    }

    pub(crate) fn reload(&self) {
        let _ = self.reports.try_send(Report::Unavailable);
    }

    pub(crate) fn reload_bypassing_cache(&self) {
        self.reload();
    }

    pub(crate) fn location(&self) -> Option<String> {
        None
    }

    pub(crate) fn magnification(&self) -> f64 {
        1.
    }

    pub(crate) fn magnify(&self, _level: f64) {}

    #[cfg(feature = "inspector")]
    pub(crate) fn open_inspector(&self) {}

    pub(crate) fn eval(&self, _script: &str, _done: impl Fn(String) + Send + 'static) -> bool {
        false
    }

    pub(crate) fn holds_keys(&self) -> bool {
        false
    }

    pub(crate) fn edit(&self, _edit: Edit) {}

    pub(crate) fn take_keys(&self) {}

    pub(crate) fn give_keys(&self) {}
}

pub(super) fn clear_store(_store: &crate::DataStore, done: impl FnOnce(bool) + Send + 'static) {
    done(false);
}

pub(super) fn store_usage(
    _store: &crate::DataStore,
    done: impl FnOnce(Option<crate::Usage>) + Send + 'static,
) {
    done(None);
}

impl Present for State {
    fn available(&self, _window: &Window) -> bool {
        false
    }
}
