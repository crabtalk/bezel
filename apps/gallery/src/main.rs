//! The bezel gallery — every component rendered in a real window. This is the
//! dev surface: new components land here the day they land in `crates/ui`.
//!
//! The view itself lives in `lib.rs`, so `shots` can mount its sections one at
//! a time; this file is the window around it.

use gallery::{
    Gallery, ResetFrameOverlayStats, TRAFFIC_LIGHT_X, TRAFFIC_LIGHT_Y, ToggleFpsOverlay,
    ToggleFullScreen,
};
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, actions, point, px, size,
};
use theme::{Theme, appearance};
use ui::icons;

actions!(gallery_app, [Quit]);

fn main() {
    let section = std::env::args().nth(1);
    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            if let Err(err) = ui::register_fonts(cx) {
                eprintln!("FONT REGISTRATION FAILED: {err:?}");
            }
            appearance::init(appearance::AppearanceMode::System, cx);
            gallery::init(cx);
            cx.bind_keys([
                // The chords zed's own keymap carries for these two.
                KeyBinding::new("ctrl-alt-shift-p", ToggleFpsOverlay, None),
                KeyBinding::new("ctrl-alt-shift-o", ResetFrameOverlayStats, None),
                // Both of the macOS defaults. Bound before `set_menus` so the
                // menu item can pick the keystroke up off the keymap.
                KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
                KeyBinding::new("fn-f", ToggleFullScreen, None),
            ]);
            set_menus(cx);
            let bounds = Bounds::centered(None, size(px(1000.0), px(700.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // No strip of its own: the traffic lights sit in the
                    // nav. `app_owns_titlebar_drag` stays false, so AppKit
                    // still moves the window by the top edge and the app owes
                    // no drag bar of its own.
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        traffic_light_position: Some(point(
                            px(TRAFFIC_LIGHT_X),
                            px(TRAFFIC_LIGHT_Y),
                        )),
                        ..Default::default()
                    }),
                    // Glass needs a blurred window background to blur INTO;
                    // without it `material` has nothing behind it.
                    window_background: Theme::of(cx).window_background_appearance(),
                    ..Default::default()
                },
                |window, cx| {
                    appearance::observe_window(window, cx).detach();
                    // `gallery editor` opens on that page — the native half of
                    // the website's `?s=`, and the shortest way to look at the
                    // one screen you are working on.
                    let gallery = cx.new(|cx| match section.as_deref() {
                        Some(key) => Gallery::showing(key, cx),
                        None => Gallery::new(cx),
                    });
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
