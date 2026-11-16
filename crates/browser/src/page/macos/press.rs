use crate::page::{
    Report,
    platform::{
        self,
        input::{GuiRoot, PageEvent as _},
    },
};
use block2::RcBlock;
use objc2::{Message as _, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask, NSView};
use std::{
    ptr::{self, NonNull},
    rc::Rc,
};

pub(super) struct Monitor(Retained<AnyObject>);

impl Monitor {
    pub(super) fn new(
        page: &NSView,
        root: Rc<GuiRoot>,
        reports: async_channel::Sender<Report>,
    ) -> Option<Self> {
        let page = page.retain();
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit supplies a live event on the main thread.
            if let Some(hit) = unsafe { event.as_ref() }.target(&page) {
                if hit.isDescendantOf(&page) {
                    let _ = reports.try_send(Report::Pressed);
                } else if platform::holds(&page)
                    && root
                        .view
                        .borrow()
                        .load()
                        .is_some_and(|view| ptr::eq(&*view, &*hit))
                {
                    let generation = root.release(&page);
                    let _ = reports.try_send(Report::Released(generation));
                }
            }
            event.as_ptr()
        });
        let mask =
            NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;
        // SAFETY: the block returns the event unchanged.
        let monitor =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) }?;
        Some(Self(monitor))
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        // SAFETY: this token came from addLocalMonitor and is removed once.
        unsafe { NSEvent::removeMonitor(&self.0) };
    }
}
