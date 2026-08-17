//! The bezel gallery — every component rendered in a real window. This is the
//! dev surface: new components land here the day they land in `crates/ui`.
//!
//! The view itself lives in `lib.rs`, so `shots` can mount its sections one at
//! a time; this file is the window around it.

use bezel_theme::{Theme, appearance};
use bezel_ui::{combobox, date, focus, icons, input, menubar, palette, tree};
use gallery::{Gallery, OpenPalette, ToggleFullScreen, ToggleInspector};
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions, actions,
    px, size,
};

actions!(gallery_app, [Quit]);

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
            date::init(cx);
            focus::init(cx);
            menubar::init(cx);
            tree::init(cx);
            cx.bind_keys([
                KeyBinding::new("cmd-k", OpenPalette, None),
                KeyBinding::new("cmd-alt-i", ToggleInspector, None),
                // Both of the macOS defaults. Bound before `set_menus` so the
                // menu item can pick the keystroke up off the keymap.
                KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
                KeyBinding::new("fn-f", ToggleFullScreen, None),
            ]);
            #[cfg(debug_assertions)]
            gallery::inspector::init(cx);
            set_menus(cx);
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

/// Without a menu bar `cmd-q` does not quit — a gpui app gets no menu items for
/// free, because the standard ones come from a nib and there is no nib here.
///
/// The same holds for full screen: nothing is auto-inserted, so the item below
/// carries an action this app binds itself. The menu is what makes the shortcut
/// *discoverable*; the keymap in [`main`] is what makes it work. Naming a menu
/// `Window` is still worth it — gpui hands that one to AppKit as the windows
/// menu, which is what maintains the window list on it.
fn set_menus(cx: &mut App) {
    cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
    cx.set_menus(vec![
        Menu::new("bezel").items([MenuItem::action("Quit", Quit)]),
        Menu::new("Window").items([MenuItem::action("Toggle Full Screen", ToggleFullScreen)]),
    ]);
}
