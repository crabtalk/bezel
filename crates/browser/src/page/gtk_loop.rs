//! wry's webview on Linux is webkit2gtk, driven by GTK's main loop, which gpui
//! does not run.

use gpui::{App, Window};
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle,
};
use std::{os::raw::c_ulong, time::Duration};

/// How often GTK's pending events are dispatched. Page input, painting
/// and callbacks wait for it.
const PUMP: Duration = Duration::from_millis(8);

/// Initializes GTK on X11 and starts pumping its events, once per process.
/// Does nothing under Wayland, or when GTK is already initialized: the
/// host runs the loop then.
pub(super) fn start(window: &Window, cx: &mut App) {
    if gtk::is_initialized() || Parent::of(window).is_none() {
        return;
    }
    gtk::gdk::set_allowed_backends("x11");
    if let Err(error) = gtk::init() {
        tracing::warn!(%error, "webview: gtk");
        return;
    }
    let timers = cx.background_executor().clone();
    cx.foreground_executor()
        .spawn(async move {
            loop {
                timers.timer(PUMP).await;
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
            }
        })
        .detach();
}

/// gpui's X11 window, as the Xlib handle wry takes. gpui hands out an XCB
/// one for the same window.
pub(super) struct Parent(XlibWindowHandle);

impl Parent {
    /// `None` under Wayland.
    pub(super) fn of(window: &Window) -> Option<Self> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Xcb(handle) => Some(Self(XlibWindowHandle::new(c_ulong::from(
                handle.window.get(),
            )))),
            _ => None,
        }
    }
}

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: an X window id, valid while gpui's window is.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xlib(self.0)) })
    }
}
