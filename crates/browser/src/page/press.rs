//! Reports a press on the page, in any of its frames, before WebKit takes it.

use super::Report;
use block2::RcBlock;
use objc2::{MainThreadMarker, Message, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask, NSView};
use std::ptr::NonNull;

pub(super) struct Monitor(Retained<AnyObject>);

impl Monitor {
    pub(super) fn new(page: &NSView, reports: async_channel::Sender<Report>) -> Option<Self> {
        let mtm = MainThreadMarker::new().expect("the page is built on the main thread");
        let page = page.retain();
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit hands a valid event.
            if on(&page, unsafe { event.as_ref() }, mtm) {
                let _ = reports.try_send(Report::Pressed);
            }
            event.as_ptr()
        });
        let mask =
            NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;
        // SAFETY: the block returns the event it was handed.
        let monitor =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) }?;
        Some(Self(monitor))
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        // SAFETY: a monitor `addLocalMonitorForEventsMatchingMask:handler:`
        // returned, removed once.
        unsafe { NSEvent::removeMonitor(&self.0) };
    }
}

/// Whether `event` lands on the page. A hidden page takes no hit.
fn on(page: &NSView, event: &NSEvent, mtm: MainThreadMarker) -> bool {
    let (Some(window), Some(pressed)) = (page.window(), event.window(mtm)) else {
        return false;
    };
    if !std::ptr::eq(&*window, &*pressed) {
        return false;
    }
    // SAFETY: called on the main thread.
    let Some(parent) = (unsafe { page.superview() }) else {
        return false;
    };
    // `hitTest:` takes a point in the superview's coordinates.
    let point = parent.convertPoint_fromView(event.locationInWindow(), None);
    page.hitTest(point).is_some()
}
