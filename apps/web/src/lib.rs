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
    App, AppContext as _, Application, ApplicationHandle, Bounds, Entity, WindowBounds,
    WindowOptions, px, size,
};
use theme::appearance::{self, AppearanceMode};
use wasm_bindgen::{JsCast as _, prelude::Closure, prelude::wasm_bindgen};

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
/// `?e=<example>` narrows that to one demo on the page.
fn requested(param: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search)
        .ok()?
        .get(param)
        .filter(|value| !value.is_empty())
}

/// Follow the doc page's `postMessage` from one example to the next, so a
/// reader scrolling past six snippets moves one embed rather than loading six.
///
/// The app is reached through [`APPLICATION`] rather than captured: the handle
/// is the app's one owner and is not copyable, and by the time a message can
/// arrive it is already stored. The listener is leaked rather than held — it
/// lives as long as the document, which is as long as the app it updates.
fn follow_examples(gallery: Entity<Gallery>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let listener =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let Some(example) = event.data().as_string() else {
                return;
            };
            // An empty string is the page saying "no snippet is in view" — the
            // embed widens back to the whole section rather than going blank.
            let example = (!example.is_empty()).then_some(example);
            APPLICATION.with(|application| {
                if let Some(handle) = application.borrow().as_ref() {
                    handle.update(|cx| {
                        gallery.update(cx, |gallery, cx| {
                            gallery.show_example(example.as_deref(), cx);
                        });
                    });
                }
            });
        });
    if window
        .add_event_listener_with_callback("message", listener.as_ref().unchecked_ref())
        .is_ok()
    {
        listener.forget();
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    gpui_web::init_logging();

    let section = requested("s");
    let example = requested("e");
    // Filled by the window builder below, and read back once the app hands its
    // handle over — the listener needs both, and neither exists before the
    // other.
    let embedded: Rc<RefCell<Option<Entity<Gallery>>>> = Rc::new(RefCell::new(None));
    let slot = embedded.clone();

    let platform = Rc::new(gpui_web::WebPlatform::new(false));
    let http_client = Arc::new(platform.fetch_http_client());
    let handle = Application::with_platform(platform)
        .with_http_client(http_client)
        .run_embedded(move |cx: &mut App| {
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
                    Some(key) => {
                        let gallery = cx.new(|cx| Gallery::embedded(key, example.as_deref(), cx));
                        *slot.borrow_mut() = Some(gallery.clone());
                        gallery
                    }
                    None => cx.new(Gallery::new),
                },
            )
            .expect("failed to open the gallery window");
            cx.activate(true);
        });
    APPLICATION.with(|application| *application.borrow_mut() = Some(handle));
    if let Some(gallery) = embedded.borrow().clone() {
        follow_examples(gallery);
    }
}
