use super::{Edit, Page};
use gpui::{App, Bounds, Pixels, Window};

pub(super) const BINDS_EDITS: bool = false;

pub(super) struct State;

impl State {
    pub(super) fn new(_cx: &App) -> Self {
        Self
    }
}

impl Page {
    pub(super) fn place(&self, bounds: Bounds<Pixels>, window: &Window) {
        self.uncover();
        self.window.set(Some(window.window_handle()));
        self.placed.set(Some(bounds));
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
    }

    pub(crate) fn back(&self) {}

    pub(crate) fn forward(&self) {}

    pub(crate) fn history(&self) -> (bool, bool) {
        (false, false)
    }

    pub(crate) fn reload(&self) {}

    pub(crate) fn reload_bypassing_cache(&self) {}

    pub(crate) fn location(&self) -> Option<String> {
        None
    }

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
