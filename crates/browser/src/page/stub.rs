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
    pub(super) fn place(&self, bounds: Bounds<Pixels>, _window: &Window) {
        self.uncover();
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

    pub(crate) fn reload(&self) {}

    pub(crate) fn location(&self) -> Option<String> {
        None
    }

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
