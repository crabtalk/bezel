//! The README's example — the smallest consumer of bezel. One window, a
//! click counter, a switch that flips light/dark. Every gpui type path is
//! `bezel::gpui`, exactly as an external app would consume the library.

use bezel::gpui::{
    App, AppContext as _, Bounds, Context, FocusHandle, Menu, MenuItem, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, size,
};
use bezel::motion;
use bezel::theme::{
    self, Appearance, Theme,
    appearance::{self, AppearanceMode},
};
use bezel::ui::{
    self,
    focus::{self, Activate},
    widgets::{ButtonStyle, Buttons, Controls},
};

actions!(hello, [Quit]);

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        if let Err(err) = ui::register_fonts(cx) {
            eprintln!("FONT REGISTRATION FAILED: {err:?}");
        }
        theme::appearance::init(theme::appearance::AppearanceMode::System, cx);
        // Tab order and `enter`/`space` activation for focused controls.
        focus::init(cx);
        // Without a menu item `cmd-q` does nothing — a gpui app gets no
        // menu for free, the standard ones come from a nib and there is
        // no nib here.
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.set_menus(vec![
            Menu::new("hello").items([MenuItem::action("Quit", Quit)]),
        ]);
        let bounds = Bounds::centered(None, size(px(520.0), px(360.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                theme::appearance::observe_window(window, cx).detach();
                cx.new(Hello::new)
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

struct Hello {
    clicks: usize,
    toggle: FocusHandle,
}

impl Hello {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            clicks: 0,
            toggle: cx.focus_handle(),
        }
    }
}

impl Render for Hello {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        // Hover fades paint once and stick unless a frame is asked for — see
        // the README's Theme section.
        if motion::hover_fades_active() {
            window.request_animation_frame();
        }
        let dark = matches!(theme.appearance, Appearance::Dark);
        // The switch's state lives in the appearance global, not in the view —
        // flipping it repaints the whole window.
        let flip = move |cx: &mut Context<Self>| {
            appearance::set_mode(
                if dark {
                    AppearanceMode::Light
                } else {
                    AppearanceMode::Dark
                },
                cx,
            );
            cx.notify();
        };
        focus::traversal(
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(16.0))
                .bg(theme.bg)
                .text_color(theme.text)
                .child(div().text_size(px(20.0)).child("hello, bezel"))
                .child(
                    theme
                        .button("Click me", ButtonStyle::Prominent, None)
                        .id("click")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.clicks += 1;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme.text_muted)
                        .child(format!("clicked {} times", self.clicks)),
                )
                .child(
                    focus::focusable(theme, &self.toggle, theme.toggle(dark))
                        .id("theme")
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| flip(cx)))
                        .on_action(cx.listener(move |_, _: &Activate, _, cx| flip(cx))),
                ),
        )
    }
}
