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
use theme::{
    AppExt as _, Appearance, Theme,
    appearance::{self, AppearanceMode},
};
use wasm_bindgen::{JsCast as _, prelude::Closure, prelude::wasm_bindgen};

/// The built-in palette, set in the Geist faces `ui::register_fonts` loads: the
/// theme's defaults name system fonts, and the browser gives gpui none.
fn palette(appearance: Appearance) -> Theme {
    let mut theme = Theme::for_appearance(appearance);
    theme.font_sans = "Geist".into();
    theme.font_body = "Geist".into();
    theme.font_mono = "Geist Mono".into();
    theme
}

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

/// `?s=<key>` embeds one section; without it the page is the whole browser,
/// opened at `?open=<key>` if given. `?e=<example>` narrows an embed to one demo
/// on the page.
fn requested(param: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search)
        .ok()?
        .get(param)
        .filter(|value| !value.is_empty())
}

/// Follow the doc page's `postMessage`, `s=<key>&e=<example>` as the query
/// string spells it, so moving between pages and snippets moves one running
/// embed rather than loading another. Once listening, tell the page with a
/// `ready` message, so a page it moved to while this loaded is sent again.
///
/// Only messages from this document's own origin are followed.
///
/// The app is reached through [`APPLICATION`] rather than captured: the handle
/// is the app's one owner and is not copyable, and by the time a message can
/// arrive it is already stored. The listener is leaked rather than held — it
/// lives as long as the document, which is as long as the app it updates.
fn follow_page(gallery: Entity<Gallery>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(origin) = window.location().origin() else {
        return;
    };
    let own = origin.clone();
    let listener =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if event.origin() != own {
                return;
            }
            let Some(data) = event.data().as_string() else {
                return;
            };
            let Ok(params) = web_sys::UrlSearchParams::new_with_str(&data) else {
                return;
            };
            let Some(key) = params.get("s") else {
                return;
            };
            let example = params.get("e").filter(|example| !example.is_empty());
            APPLICATION.with(|application| {
                if let Some(handle) = application.borrow().as_ref() {
                    handle.update(|cx| {
                        gallery.update(cx, |gallery, cx| {
                            gallery.show(&key, example.as_deref(), cx);
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
    if let Ok(Some(parent)) = window.parent() {
        let _ = parent.post_message(&"ready".into(), &origin);
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    gpui_web::init_logging();

    let section = requested("s");
    let example = requested("e");
    let opening = requested("open");
    let embedded = section.is_some();

    let platform = Rc::new(gpui_web::WebPlatform::new(false).with_system_font("Geist"));
    let http_client = Arc::new(platform.fetch_http_client());
    let handle = Application::with_platform(platform)
        .with_http_client(http_client)
        .run_embedded(move |cx: &mut App| {
            if let Err(err) = ui::register_fonts(cx) {
                log::error!("font registration failed: {err:?}");
            }
            cx.set_palette(palette);
            appearance::init(AppearanceMode::System, cx);
            gallery::init(cx);
            let bounds = Bounds::centered(None, size(px(1000.0), px(860.0)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    // No `observe_window` here, unlike the native app: it reconciles
                    // appearance synchronously during init, and on a mismatch that
                    // reaches `reapply_window_background`, which updates the window
                    // still being constructed.
                    move |_, cx| match section.as_deref() {
                        Some(key) => cx.new(|cx| Gallery::embedded(key, example.as_deref(), cx)),
                        None => match opening.as_deref() {
                            Some(key) => cx.new(|cx| Gallery::showing(key, cx)),
                            None => cx.new(Gallery::new),
                        },
                    },
                )
                .expect("failed to open the gallery window");
            cx.activate(true);
            // On the web this closure runs once the browser's graphics resolve,
            // after `run_embedded` has returned and the handle is stored.
            if embedded && let Ok(gallery) = window.entity(cx) {
                follow_page(gallery);
            }
        });
    APPLICATION.with(|application| *application.borrow_mut() = Some(handle));
}
