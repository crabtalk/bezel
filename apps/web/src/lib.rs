//! The gallery, in a browser tab — the same view `apps/gallery` opens in a
//! native window, on gpui's web platform.
//!
//! Single-threaded, like gpui's own browser gallery: the threaded dispatcher
//! wants nightly for `wasm_thread` and a cross-origin-isolated document for
//! `SharedArrayBuffer`, and buys nothing this page needs.

#![cfg(target_family = "wasm")]

use std::{cell::RefCell, rc::Rc, sync::Arc};

use gallery::Gallery;
use gpui::{
    App, AppContext as _, Application, ApplicationHandle, Bounds, WindowBounds, WindowOptions, px,
    size,
};
use theme::appearance::{self, AppearanceMode};
use ui::icons;
use wasm_bindgen::prelude::wasm_bindgen;

thread_local! {
    /// The whole app, and the reason it stays alive.
    ///
    /// `Platform::run` blocks for the process lifetime natively, so the stack
    /// frame owns the app; on wasm the run loop is the browser's, so it returns
    /// straight away and `Application::run` would drop everything it just
    /// built. `run_embedded` hands back the handle instead — dropping it
    /// releases the app, which is exactly what "app was released" meant.
    static APPLICATION: RefCell<Option<ApplicationHandle>> = const { RefCell::new(None) };
}

/// `?s=<key>` embeds one section; without it the page is the whole browser.
fn requested_section() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search)
        .ok()?
        .get("s")
        .filter(|key| !key.is_empty())
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    gpui_web::init_logging();

    let section = requested_section();

    let platform = Rc::new(gpui_web::WebPlatform::new(false));
    let http_client = Arc::new(platform.fetch_http_client());
    let handle = Application::with_platform(platform)
        .with_http_client(http_client)
        .with_assets(icons::Assets)
        .run_embedded(|cx: &mut App| {
            if let Err(err) = ui::register_fonts(cx) {
                log::error!("font registration failed: {err:?}");
            }
            appearance::init(AppearanceMode::System, cx);
            gallery::init(cx);
            let bounds = Bounds::centered(None, size(px(1000.0), px(860.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                // No `observe_window` here, unlike the native app: it reconciles
                // appearance synchronously during init, and on a mismatch that
                // reaches `reapply_window_background`, which updates the window
                // still being constructed.
                move |_, cx| match section.as_deref() {
                    Some(key) => cx.new(|cx| Gallery::embedded(key, cx)),
                    None => cx.new(Gallery::new),
                },
            )
            .expect("failed to open the gallery window");
            cx.activate(true);
        });
    APPLICATION.with(|application| *application.borrow_mut() = Some(handle));
}
