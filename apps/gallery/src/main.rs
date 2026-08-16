//! The bezel gallery — every component rendered in a real window. This is the
//! dev surface: new components land here the day they land in `crates/ui`.
//!
//! The view itself lives in `lib.rs`, so `shots` can mount its sections one at
//! a time; this file is the window around it.

use bezel_theme::{Theme, appearance};
use bezel_ui::{combobox, icons, input, palette};
use gallery::{Gallery, OpenPalette, ToggleInspector};
use gpui::{App, AppContext as _, Bounds, KeyBinding, WindowBounds, WindowOptions, px, size};

fn main() {
    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(|cx: &mut App| {
            if let Err(err) = bezel_ui::register_fonts(cx) {
                eprintln!("FONT REGISTRATION FAILED: {err:?}");
            }
            appearance::init(appearance::AppearanceMode::System, cx);
            input::init(cx);
            palette::init(cx);
            combobox::init(cx);
            cx.bind_keys([
                KeyBinding::new("cmd-k", OpenPalette, None),
                KeyBinding::new("cmd-alt-i", ToggleInspector, None),
            ]);
            #[cfg(debug_assertions)]
            gallery::inspector::init(cx);
            let bounds = Bounds::centered(None, size(px(1000.0), px(860.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // Glass needs a blurred window background to blur INTO;
                    // without it `material` has nothing behind it.
                    window_background: Theme::of(cx).window_background_appearance(),
                    ..Default::default()
                },
                |window, cx| {
                    appearance::observe_window(window, cx).detach();
                    let gallery = cx.new(Gallery::new);
                    // The gallery itself takes focus, so its key context is
                    // live from the first frame whatever page is showing.
                    let focus = gallery.read(cx).focus_handle();
                    window.focus(&focus, cx);
                    gallery
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
