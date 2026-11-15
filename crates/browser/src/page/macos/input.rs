use gpui::{AnyWindowHandle, native_view::NativeViewState};
use objc2::{
    MainThreadOnly as _,
    rc::{Retained, Weak},
};
use objc2_app_kit::{NSEvent, NSView};
use std::{
    cell::{Cell, RefCell},
    ptr,
    rc::Rc,
};

pub(super) struct GuiRoot {
    pub(super) view: RefCell<Weak<NSView>>,
    pub(super) window: Cell<Option<AnyWindowHandle>>,
    pub(super) input: Rc<NativeViewState>,
}

impl GuiRoot {
    pub(super) fn new(input: Rc<NativeViewState>) -> Self {
        Self {
            view: RefCell::new(Weak::default()),
            window: Cell::new(None),
            input,
        }
    }

    pub(super) fn release(&self, page: &NSView) -> u64 {
        self.input
            .request_focus(false, self.window.get().map(|window| window.window_id()));
        if let Some(root) = self.view.borrow().load() {
            if let Some(window) = root.window() {
                let owns_focus = window.firstResponder().is_some_and(|responder| {
                    responder
                        .downcast_ref::<NSView>()
                        .is_some_and(|view| ptr::eq(view, page) || view.isDescendantOf(page))
                });
                if owns_focus {
                    window.makeFirstResponder(Some(&root));
                }
            }
        }
        self.input.focus.get().generation
    }
}

pub(super) trait PageEvent {
    fn target(&self, page: &NSView) -> Option<Retained<NSView>>;
}

impl PageEvent for NSEvent {
    fn target(&self, page: &NSView) -> Option<Retained<NSView>> {
        let window = page.window()?;
        let event_window = self.window(page.mtm())?;
        if !ptr::eq(&*window, &*event_window) {
            return None;
        }
        let content = window.contentView()?;
        // SAFETY: AppKit delivers these events on the main thread.
        let parent = unsafe { content.superview() };
        let position = parent.map_or(self.locationInWindow(), |parent| {
            parent.convertPoint_fromView(self.locationInWindow(), None)
        });
        content.hitTest(position)
    }
}
