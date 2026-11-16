use crate::page::platform::input::PageEvent as _;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::{Retained, Weak},
    runtime::{NSObject, NSObjectProtocol},
};
use objc2_app_kit::{NSEvent, NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindow};
use objc2_foundation::NSRect;
use std::{cell::RefCell, ops::Deref, ptr};

pub(super) struct Watch {
    area: Retained<NSTrackingArea>,
    // A tracking area does not retain its owner.
    watcher: Retained<Watcher>,
}

impl Watch {
    pub(super) fn new(page: &NSView) -> Self {
        let mtm = MainThreadMarker::new().expect("the page is built on the main thread");
        let watcher: Retained<Watcher> = {
            let watcher = Watcher::alloc(mtm).set_ivars(Inside {
                page: page.retain(),
                window: RefCell::new(Weak::default()),
            });
            // SAFETY: NSObject initialization on a freshly allocated instance.
            unsafe { msg_send![super(watcher), init] }
        };
        // SAFETY: the watcher implements the tracking callbacks and outlives the area.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&watcher),
                None,
            )
        };
        page.addTrackingArea(&area);
        Self { area, watcher }
    }
}

impl Deref for Watch {
    type Target = Watcher;

    fn deref(&self) -> &Watcher {
        &self.watcher
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.watcher.ivars().page.removeTrackingArea(&self.area);
        self.release();
    }
}

pub(super) struct Inside {
    page: Retained<NSView>,
    window: RefCell<Weak<NSWindow>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "BezelWebViewCursorWatcher"]
    #[ivars = Inside]
    pub(super) struct Watcher;

    unsafe impl NSObjectProtocol for Watcher {}

    impl Watcher {
        #[unsafe(method(mouseEntered:))]
        fn entered(&self, event: &NSEvent) {
            self.track(event);
        }

        #[unsafe(method(mouseMoved:))]
        fn moved(&self, event: &NSEvent) {
            self.track(event);
        }

        #[unsafe(method(mouseExited:))]
        fn exited(&self, _: &NSEvent) {
            self.release();
        }
    }
);

impl Watcher {
    fn track(&self, event: &NSEvent) {
        let state = self.ivars();
        let over_page = event.target(&state.page).is_some_and(|target| {
            ptr::eq(&*target, &*state.page) || target.isDescendantOf(&state.page)
        });
        if !over_page {
            self.release();
        } else if state.window.borrow().load().is_none()
            && let Some(window) = state.page.window()
        {
            window.disableCursorRects();
            state.window.replace(Weak::new(&window));
        }
    }

    pub(super) fn release(&self) {
        if let Some(window) = self.ivars().window.replace(Weak::default()).load() {
            window.enableCursorRects();
            if let Some(content) = window.contentView() {
                window.invalidateCursorRectsForView(&content);
            }
        }
    }
}
