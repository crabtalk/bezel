//! `reference.swift` in bezel: one 168x168 r34 glass square over
//! the frosted window background, with text under it to refract. The Swift
//! probe holds a real `NSGlassEffectView`, this holds ours, and the two are the
//! same scene at the same measurements — so a difference on screen is our lens
//! and nothing else.

use gpui::{
    App, AppContext as _, Bounds, Context, Menu, MenuItem, Point, TitlebarOptions, Window,
    WindowBounds, WindowOptions, actions, div, point, prelude::*, px, size,
};
use motion::Painter;
use theme::{
    Appearance, Glass, SurfaceStyle, TextStyle, Theme, Typeset as _,
    appearance::{self, AppearanceMode},
};
use ui::{
    floating::{self, Floating},
    surface::Surfaced as _,
    widgets::{ButtonStyle, Buttons as _},
};

actions!(vibrancy, [Quit]);

const CARD: f32 = 168.0;
const CARD_RADIUS: f32 = 34.0;
const WELL: (f32, f32) = (400.0, 360.0);
const PAD: f32 = 20.0;
const FOOT: f32 = 86.0;
/// Room for the titlebar the content runs under, as in the Swift probe.
const TOP: f32 = 36.0;

const SPECIMEN: &str = "the quick brown fox jumps over the lazy dog and back again";
const LINE_H: f32 = 24.0;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        if let Err(err) = ui::register_fonts(cx) {
            eprintln!("FONT REGISTRATION FAILED: {err:?}");
        }
        appearance::init(appearance::AppearanceMode::System, cx);
        ui::focus::init(cx);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.set_menus(vec![
            Menu::new("vibrancy").items([MenuItem::action("Quit", Quit)]),
        ]);
        let bounds = Bounds::centered(
            None,
            size(px(PAD * 2.0 + WELL.0), px(TOP + WELL.1 + FOOT)),
            cx,
        );
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    ..Default::default()
                }),
                // Glass needs a blurred window background to blur INTO.
                window_background: Theme::of(cx).window_background_appearance(),
                ..Default::default()
            },
            |window, cx| {
                appearance::observe_window(window, cx).detach();
                cx.new(Vibrancy::new)
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

struct Vibrancy {
    style: SurfaceStyle,
    at: Floating,
}

impl Vibrancy {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            style: SurfaceStyle::Glass(Glass::Regular),
            at: Floating::new(Painter::of(cx)),
        }
    }

    fn home() -> Point<gpui::Pixels> {
        point(px((WELL.0 - CARD) / 2.0), px((WELL.1 - CARD) / 2.0))
    }
}

impl Render for Vibrancy {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let dark = matches!(theme.appearance, Appearance::Dark);
        let clear = matches!(self.style, SurfaceStyle::Glass(Glass::Clear));

        let at = self.at.at().unwrap_or_else(Self::home);
        let origin = window.bounds().origin;
        let scale = window.scale_factor();
        let readout = format!(
            "{CARD:.0}×{CARD:.0} r{CARD_RADIUS:.0} @ ({:.0}, {:.0}) · screen px {:.0} {:.0}",
            f32::from(at.x),
            f32::from(at.y),
            (f32::from(origin.x) + PAD + f32::from(at.x)) * scale,
            (f32::from(origin.y) + TOP + f32::from(at.y)) * scale,
        );

        let pick = |label: &'static str, on: bool| {
            theme
                .button(
                    label,
                    if on {
                        ButtonStyle::Prominent
                    } else {
                        ButtonStyle::Ghost
                    },
                    None,
                )
                .id(label)
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.window_bg())
            .text_color(theme.text)
            .child(div().h(px(TOP)))
            .child(
                div()
                    .relative()
                    .mx(px(PAD))
                    .w(px(WELL.0))
                    .h(px(WELL.1))
                    .rounded(px(12.0))
                    .overflow_hidden()
                    .children((0..(WELL.1 / LINE_H) as usize).map(|line| {
                        // Rotated per line so the block is not a vertical grid,
                        // which would be as periodic as the bars it replaced.
                        div()
                            .h(px(LINE_H))
                            .pl(px(8.0))
                            .whitespace_nowrap()
                            .child(SPECIMEN[line * 5 % 17..].to_string())
                    }))
                    .child(floating::panel(
                        "glass",
                        &self.at,
                        Self::home(),
                        div()
                            .w(px(CARD))
                            .h(px(CARD))
                            .rounded(px(CARD_RADIUS))
                            .surface(theme, self.style),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(PAD))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(pick("regular", !clear).on_click(cx.listener(|view, _, _, cx| {
                                view.style = SurfaceStyle::Glass(Glass::Regular);
                                cx.notify();
                            })))
                            .child(pick("clear", clear).on_click(cx.listener(|view, _, _, cx| {
                                view.style = SurfaceStyle::Glass(Glass::Clear);
                                cx.notify();
                            })))
                            .child(div().w(px(12.0)))
                            .child(pick("dark", dark).on_click(cx.listener(|_, _, _, cx| {
                                appearance::set_mode(AppearanceMode::Dark, cx);
                                cx.notify();
                            })))
                            .child(pick("light", !dark).on_click(cx.listener(|_, _, _, cx| {
                                appearance::set_mode(AppearanceMode::Light, cx);
                                cx.notify();
                            })))
                            .child(div().w(px(12.0)))
                            .child(
                                theme
                                    .button("reset", ButtonStyle::Ghost, None)
                                    .id("reset")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.at.move_to(Self::home());
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_style(TextStyle::Caption)
                            .text_color(theme.text_muted)
                            .child(readout),
                    ),
            )
    }
}
