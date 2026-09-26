//! Hands the cursor to WebKit while the pointer is over the page.
//!
//! gpui's view registers a cursor rect over its whole visible rect, the page's
//! pixels included, and AppKit applies it over the cursor WebKit sets. While
//! the pointer is inside the page the window's cursor rects are disabled.

use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, NSObjectProtocol},
};
use objc2_app_kit::{NSEvent, NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindow};
use objc2_foundation::NSRect;
use std::cell::Cell;

pub(super) struct Watch {
    page: Retained<NSView>,
    area: Retained<NSTrackingArea>,
    /// A tracking area does not retain its owner.
    watcher: Retained<Watcher>,
}

impl Watch {
    pub(super) fn new(page: &NSView) -> Self {
        let mtm = MainThreadMarker::new().expect("the page is built on the main thread");
        let watcher: Retained<Watcher> = {
            let watcher = Watcher::alloc(mtm).set_ivars(Inside::default());
            // SAFETY: `NSObject`'s `init` on a freshly allocated instance.
            unsafe { msg_send![super(watcher), init] }
        };
        // SAFETY: the watcher answers `mouseEntered:` and `mouseExited:`
        // and outlives the area, which `Drop` removes first.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&watcher),
                None,
            )
        };
        page.addTrackingArea(&area);
        Self {
            page: page.retain(),
            area,
            watcher,
        }
    }

    /// Gives the cursor back to gpui if the pointer was inside the page.
    /// A hidden view reports no exit.
    pub(super) fn release(&self) {
        if self.watcher.ivars().0.replace(false)
            && let Some(window) = self.page.window()
        {
            enable(&window);
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.page.removeTrackingArea(&self.area);
        self.release();
    }
}

#[derive(Default)]
struct Inside(Cell<bool>);

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "BezelWebViewCursorWatcher"]
    #[ivars = Inside]
    struct Watcher;

    unsafe impl NSObjectProtocol for Watcher {}

    impl Watcher {
        #[unsafe(method(mouseEntered:))]
        fn entered(&self, event: &NSEvent) {
            self.ivars().0.set(true);
            if let Some(window) = event.window(self.mtm()) {
                window.disableCursorRects();
            }
        }

        #[unsafe(method(mouseExited:))]
        fn exited(&self, event: &NSEvent) {
            if self.ivars().0.replace(false)
                && let Some(window) = event.window(self.mtm())
            {
                enable(&window);
            }
        }
    }
);

fn enable(window: &NSWindow) {
    window.enableCursorRects();
    if let Some(content) = window.contentView() {
        window.invalidateCursorRectsForView(&content);
    }
}
