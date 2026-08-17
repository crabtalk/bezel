//! The gallery, in a browser tab — the same view `apps/gallery` opens in a
//! native window, on gpui's web platform.
//!
//! Single-threaded on purpose. The threaded dispatcher needs `SharedArrayBuffer`,
//! which needs COOP/COEP response headers, and GitHub Pages cannot send them;
//! it also drags in a `wasm_thread` that only builds on nightly.

#![cfg(target_family = "wasm")]

use std::rc::Rc;
use std::sync::Arc;

use bezel_theme::appearance::{self, AppearanceMode};
use bezel_ui::{combobox, date, focus, icons, input, menubar, palette, tree};
use gallery::{Gallery, OpenPalette};
use gpui::{App, AppContext as _, Application, KeyBinding, WindowOptions};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    gpui_web::init_logging();

    let platform = Rc::new(gpui_web::WebPlatform::new(false));
    let http_client = Arc::new(platform.fetch_http_client());
    Application::with_platform(platform)
        .with_http_client(http_client)
        .with_assets(icons::Assets)
        .run(|cx: &mut App| {
            if let Err(err) = bezel_ui::register_fonts(cx) {
                log::error!("font registration failed: {err:?}");
            }
            appearance::init(AppearanceMode::System, cx);
            input::init(cx);
            palette::init(cx);
            combobox::init(cx);
            date::init(cx);
            focus::init(cx);
            menubar::init(cx);
            tree::init(cx);
            cx.bind_keys([KeyBinding::new("cmd-k", OpenPalette, None)]);
            cx.open_window(WindowOptions::default(), |window, cx| {
                appearance::observe_window(window, cx).detach();
                let gallery = cx.new(Gallery::new);
                let focus = gallery.read(cx).focus_handle();
                window.focus(&focus, cx);
                gallery
            })
            .unwrap();
        });
}
