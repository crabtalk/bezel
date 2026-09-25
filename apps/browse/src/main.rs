//! A Safari-shaped browser over `browser::WebView`: tabs, a unified toolbar
//! with the address field in it, a favorites start page and a sidebar.

mod shell;

use gpui::{
    App, AppContext as _, Bounds, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions,
    actions, point, px, size,
};
use shell::Shell;
use theme::{Theme, appearance};

actions!(browse, [Quit]);

/// `browse <url>` opens on that page; without one, on the start page.
fn main() {
    let url = std::env::args().nth(1);
    // Not `gpui_platform::application()`, which installs a reqwest client
    // for remote images: nothing here fetches one, and the client carries a
    // TLS stack into the binary.
    gpui::Application::with_platform(gpui_platform::current_platform(false)).run(
        move |cx: &mut App| {
            if let Err(err) = ui::register_fonts(cx) {
                eprintln!("FONT REGISTRATION FAILED: {err:?}");
            }
            appearance::init(appearance::AppearanceMode::System, cx);
            ui::input::init(cx);
            shell::init(cx);
            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            cx.bind_keys([gpui::KeyBinding::new("secondary-q", Quit, None)]);
            cx.set_menus(vec![
                Menu::new("Browse").items([MenuItem::action("Quit", Quit)]),
                Menu::new("File").items([
                    MenuItem::action("New Tab", shell::NewTab),
                    MenuItem::action("Close Tab", shell::CloseTab),
                    MenuItem::action("Open Location…", shell::FocusAddress),
                ]),
                Menu::new("View").items([
                    MenuItem::action("Toggle Sidebar", shell::ToggleSidebar),
                    MenuItem::action("Reload Page", shell::Reload),
                ]),
                Menu::new("History").items([
                    MenuItem::action("Back", shell::Back),
                    MenuItem::action("Forward", shell::Forward),
                ]),
                Menu::new("Window").items([
                    MenuItem::action("Show Next Tab", shell::NextTab),
                    MenuItem::action("Show Previous Tab", shell::PreviousTab),
                ]),
            ]);
            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Browse".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(
                            px(shell::TRAFFIC_LIGHT_X),
                            px(shell::TRAFFIC_LIGHT_Y),
                        )),
                    }),
                    window_background: Theme::of(cx).window_background_appearance(),
                    ..Default::default()
                },
                |window, cx| {
                    appearance::observe_window(window, cx).detach();
                    cx.new(|cx| Shell::new(url, window, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        },
    );
}
