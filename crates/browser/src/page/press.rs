//! Watches presses in the page's window before AppKit dispatches them. A
//! press on the page, in any of its frames, is reported. A press on gpui's
//! view outside the page takes the keys back from the page, whether or not
//! gpui's focus moves.

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
            match landed(&page, unsafe { event.as_ref() }, mtm) {
                Some(Landed::Page) => {
                    let _ = reports.try_send(Report::Pressed);
                }
                Some(Landed::Host) if super::holds(&page) => super::release(&page),
                _ => {}
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

enum Landed {
    Page,
    /// gpui's view, outside the page and any other view in it.
    Host,
}

/// `None` for a press outside gpui's view in the page's window, or on
/// another view inside it. A hidden page takes no hit.
fn landed(page: &NSView, event: &NSEvent, mtm: MainThreadMarker) -> Option<Landed> {
    let (window, pressed) = (page.window()?, event.window(mtm)?);
    if !std::ptr::eq(&*window, &*pressed) {
        return None;
    }
    // SAFETY: called on the main thread.
    let host = unsafe { page.superview() }?;
    // SAFETY: called on the main thread.
    let frame = unsafe { host.superview() }?;
    // `hitTest:` takes a point in the superview's coordinates.
    let point = frame.convertPoint_fromView(event.locationInWindow(), None);
    let hit = host.hitTest(point)?;
    if hit.isDescendantOf(page) {
        Some(Landed::Page)
    } else if std::ptr::eq(&*hit, &*host) {
        Some(Landed::Host)
    } else {
        None
    }
}
