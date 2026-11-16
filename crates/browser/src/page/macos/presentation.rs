use crate::page::{Page, Present, Report, platform::State};
use gpui::{
    self, App, Bounds, Hitbox, Pixels, Window,
    native_view::{NativeView, Viewport},
};
use objc2::rc::Weak;
use objc2_app_kit::NSView;
use raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use std::{ptr::NonNull, rc::Rc};
use wry::WebViewExtMacOS as _;

impl Present for State {
    const COMPOSITED: bool = true;

    fn paint(
        &self,
        page: &Rc<Page>,
        bounds: Bounds<Pixels>,
        hitbox: &Hitbox,
        owner: u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        let handle = Window::window_handle(window);
        if bounds.intersect(&window.content_mask().bounds).is_empty() {
            if page.window.get() == Some(handle) {
                page.park();
            }
            return;
        }
        if !window.native_surfaces.views {
            tracing::warn!("live browser composition is unavailable on this backend");
            return;
        }
        let Ok(RawWindowHandle::AppKit(root)) =
            HasWindowHandle::window_handle(window).map(|handle| handle.as_raw())
        else {
            return;
        };
        // SAFETY: the GPUI window owns this main-thread view. The callback keeps only a zeroing weak reference.
        let root = Weak::new(unsafe { root.ns_view.cast::<NSView>().as_ref() });
        let viewport = match page.presentation.get() {
            Some(presentation) => presentation.viewport,
            None => match Viewport::try_from(bounds.size) {
                Ok(viewport) => viewport,
                Err(_) => return,
            },
        };
        if page.view.get().is_none() {
            self.root.view.replace(root);
            self.root.window.set(Some(handle));
            Page::request(
                page,
                Bounds::new(gpui::point(gpui::px(0.), gpui::px(0.)), *viewport),
                window,
                cx,
            );
            return;
        }
        if page.built().is_none() {
            return;
        }
        let mut native = NativeView::new(page.clone(), viewport, page.input.clone());
        native.presented = Some(Rc::new({
            let page = page.clone();
            move || {
                if page.window.replace(Some(handle)) != Some(handle) {
                    page.platform.parked();
                    page.platform.root.view.replace(root.clone());
                    page.platform.root.window.set(Some(handle));
                    let _ = page.reports.try_send(Report::Presented(handle));
                }
                page.placed.set(Some(bounds));
                page.owner.set(owner);
            }
        }));
        if let Err(error) = window.paint_native_view(native, bounds, Some(&page.focus), hitbox) {
            tracing::warn!(%error, "webview: present");
        }
    }
}

impl HasWindowHandle for Page {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let page = self.built().ok_or(HandleError::Unavailable)?.webview();
        let handle = RawWindowHandle::AppKit(AppKitWindowHandle::new(NonNull::from(&*page).cast()));
        // SAFETY: the Page retains the WKWebView for this borrowed handle's lifetime.
        Ok(unsafe { WindowHandle::borrow_raw(handle) })
    }
}
