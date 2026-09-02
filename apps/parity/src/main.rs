//! `reference.swift` in bezel: one 168x168 r34 glass square over the frosted
//! window background, with text under it to refract. The Swift probe holds a
//! real `NSGlassEffectView`, this holds ours, and the two are the same scene at
//! the same measurements — so a difference on screen is our lens and nothing
//! else.
//!
//! The knobs move the numbers the card is painted with. Chips load a look's
//! numbers and the knobs move them from there, so what the card paints is
//! always `spec` — one conduit, whichever style named the numbers in it.

use gpui::{
    App, AppContext as _, Bounds, Context, DragMoveEvent, Empty, FocusHandle, Menu, MenuItem,
    Point, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, point,
    prelude::*, px, size,
};
use motion::Painter;
use theme::{
    Appearance, Glass, SurfaceSpec, SurfaceStyle, TextStyle, Theme, Typeset as _,
    appearance::{self, AppearanceMode},
};
use ui::{
    floating::{self, Floating},
    focus,
    surface::Surfaced as _,
    widgets::{self, ButtonStyle, Buttons as _, Controls as _, SliderDrag},
};

actions!(parity, [Quit]);

const CARD: f32 = 168.0;
const CARD_RADIUS: f32 = 34.0;
const WELL: (f32, f32) = (400.0, 360.0);
const PAD: f32 = 20.0;
const FOOT: f32 = 260.0;
/// Room for the titlebar the content runs under, as in the Swift probe.
const TOP: f32 = 36.0;

const SPECIMEN: &str = "the quick brown fox jumps over the lazy dog and back again";
const LINE_H: f32 = 24.0;

/// The position code: green ramps once across the block, red sawtooths every
/// CODE_PERIOD, so a pixel under the glass names the position it came from and
/// a displacement is read rather than inferred. Both ramps are linear, which a
/// blur leaves alone, and both stay inside 20..200 so no channel clips.
const CODE_PERIOD: f32 = 32.0;
const CODE_LO: f32 = 20.0;
const CODE_HI: f32 = 200.0;

/// Every knob's full-scale value, and the arrow-key step as a share of it —
/// the gallery probe's, so a reading there and a reading here are the same
/// sweep. `mag` is the one knob still a fraction: it spans -16..+16, so the
/// sweep passes through zero and the lens inverts halfway.
const GAIN_RANGE: f32 = 1.5;
const SAT_RANGE: f32 = 3.0;
const LIFT_RANGE: f32 = 1.0;
const EDGE_RANGE: f32 = 0.6;
const EDGE_W_RANGE: f32 = 4.0;
const DISPERSION_RANGE: f32 = 0.05;
const RIM_RANGE: f32 = 32.0;
const BLUR_RANGE: f32 = 60.0;
/// What `extent / 2` gave on this card before `reach` was a number, so the old
/// behaviour is the top of the sweep.
const REACH_RANGE: f32 = CARD / 2.0;
const STEP: f32 = 1.0 / 320.0;

/// Label, current value, and full scale, in the knob's own units — so the label
/// is the number to write back into the palette and only the slider sees a
/// fraction.
const KNOBS: [(&str, f32); 12] = [
    ("gain", GAIN_RANGE),
    ("sat", SAT_RANGE),
    ("lift", LIFT_RANGE),
    ("blur", BLUR_RANGE),
    ("rim", RIM_RANGE),
    ("reach", REACH_RANGE),
    ("edge", EDGE_RANGE),
    ("edge w", EDGE_W_RANGE),
    ("edge aa", EDGE_W_RANGE),
    ("mag", 1.0),
    ("disp", DISPERSION_RANGE),
    ("frost", LIFT_RANGE),
];

fn main() {
    // The reference's argv, so one command line puts both windows in the same
    // state: `[clear] [light] [opaque] [coded]`.
    let args: Vec<String> = std::env::args().collect();
    let arg = move |name: &str| args.iter().any(|a| a == name);
    let (clear, light, opaque, coded) = (arg("clear"), arg("light"), arg("opaque"), arg("coded"));

    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(err) = ui::register_fonts(cx) {
            eprintln!("FONT REGISTRATION FAILED: {err:?}");
        }
        // Pinned, as the reference pins it: `System` would resolve off whatever
        // the OS is set to and the two windows could disagree.
        appearance::init(
            if light {
                AppearanceMode::Light
            } else {
                AppearanceMode::Dark
            },
            cx,
        );
        if opaque {
            let mut brand = theme::brand(cx);
            brand.vibrancy = false;
            theme::set_brand(brand, cx);
        }
        focus::init(cx);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.set_menus(vec![
            Menu::new("parity").items([MenuItem::action("Quit", Quit)]),
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
                cx.new(|cx| Parity::new(clear, coded, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

struct Parity {
    spec: SurfaceSpec,
    /// A fraction, mapped to -16..+16 on the way to the theme.
    magnify: f32,
    dispersion: f32,
    /// The frost's coverage, painted through a theme of our own so a sweep does
    /// not disturb the installed palette.
    frost: f32,
    coded: bool,
    at: Floating,
    knobs: Vec<FocusHandle>,
}

impl Parity {
    fn new(clear: bool, coded: bool, cx: &mut Context<Self>) -> Self {
        let theme = Theme::of(cx);
        let mut this = Self {
            spec: theme.glass_regular,
            magnify: (theme.glass_magnify + 16.0) / 32.0,
            dispersion: theme.glass_dispersion,
            frost: theme.vibrancy_alpha,
            coded,
            at: Floating::new(Painter::of(cx)),
            knobs: KNOBS.iter().map(|_| cx.focus_handle()).collect(),
        };
        if clear {
            this.spec = theme.glass_clear;
        }
        this
    }

    /// Load a shipped look's numbers; the knobs move them from there.
    fn load(&mut self, glass: Glass, cx: &App) {
        let theme = Theme::of(cx);
        self.spec = match glass {
            Glass::Regular => theme.glass_regular,
            Glass::Clear => theme.glass_clear,
        };
        self.magnify = (theme.glass_magnify + 16.0) / 32.0;
        self.dispersion = theme.glass_dispersion;
    }

    fn knob(&mut self, slot: usize) -> &mut f32 {
        match slot {
            0 => &mut self.spec.gain,
            1 => &mut self.spec.saturation,
            2 => &mut self.spec.tint.a,
            3 => &mut self.spec.blur,
            4 => &mut self.spec.rim,
            5 => &mut self.spec.reach,
            6 => &mut self.spec.edge,
            7 => &mut self.spec.edge_width,
            8 => &mut self.spec.edge_aa,
            9 => &mut self.magnify,
            10 => &mut self.dispersion,
            _ => &mut self.frost,
        }
    }

    fn value(&self, slot: usize) -> f32 {
        match slot {
            0 => self.spec.gain,
            1 => self.spec.saturation,
            2 => self.spec.tint.a,
            3 => self.spec.blur,
            4 => self.spec.rim,
            5 => self.spec.reach,
            6 => self.spec.edge,
            7 => self.spec.edge_width,
            8 => self.spec.edge_aa,
            9 => self.magnify,
            10 => self.dispersion,
            _ => self.frost,
        }
    }

    fn label(&self, slot: usize) -> SharedString {
        let (name, _) = KNOBS[slot];
        match slot {
            0 => format!("{name} {:.3}", self.spec.gain),
            1 => format!("{name} {:.2}", self.spec.saturation),
            2 => format!("{name} {:.0}/255", self.spec.tint.a * 255.0),
            3 => format!("{name} {:.1}pt", self.spec.blur),
            4 => format!("{name} {:.1}pt", self.spec.rim),
            5 => format!("{name} {:.1}pt", self.spec.reach),
            6 => format!("{name} {:.2}", self.spec.edge),
            7 => format!("{name} {:.1}pt", self.spec.edge_width),
            8 => format!("{name} {:.1}pt", self.spec.edge_aa),
            9 => format!("{name} {:+.1}", self.magnify * 32.0 - 16.0),
            10 => format!("{name} {:.3}", self.dispersion),
            _ => format!("{name} {:.2}", self.frost),
        }
        .into()
    }

    /// One 1pt strip per point, as the reference paints it.
    fn code() -> impl Iterator<Item = gpui::Div> {
        (0..WELL.0 as usize).map(|i| {
            let x = i as f32 + 0.5;
            let coarse = CODE_LO + (CODE_HI - CODE_LO) * x / WELL.0;
            let fine = CODE_LO + (CODE_HI - CODE_LO) * (x % CODE_PERIOD) / CODE_PERIOD;
            div()
                .absolute()
                .left(px(i as f32))
                .top(px(0.0))
                .w(px(1.0))
                .h(px(WELL.1))
                .bg(gpui::rgb(
                    ((fine.round() as u32) << 16) | ((coarse.round() as u32) << 8),
                ))
        })
    }

    fn home() -> Point<gpui::Pixels> {
        point(px((WELL.0 - CARD) / 2.0), px((WELL.1 - CARD) / 2.0))
    }

    /// The window mode alone: the frost's coverage and the chrome's glass both
    /// stay where they are, so the card is the one thing that has not changed
    /// between the two shots.
    fn set_frosted(on: bool, cx: &mut App) {
        let mut brand = theme::brand(cx);
        brand.vibrancy = on;
        theme::set_brand(brand, cx);
    }
}

impl Render for Parity {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let dark = matches!(theme.appearance, Appearance::Dark);
        let frosted = theme.vibrancy;
        // The card is painted from `spec` whichever chip named it, so the knobs
        // have one conduit into the lens.
        let tuned = Theme {
            glass_regular: self.spec,
            glass_magnify: self.magnify * 32.0 - 16.0,
            glass_dispersion: self.dispersion,
            vibrancy_alpha: self.frost,
            ..theme.clone()
        };
        let clear = self.spec == theme.glass_clear;

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

        let knob = |slot: usize| {
            let (_, range) = KNOBS[slot];
            let id: SharedString = format!("knob-{slot}").into();
            let element = id.clone();
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .w(px(70.0))
                        .text_style(TextStyle::Caption)
                        .text_color(theme.text_muted)
                        .child(self.label(slot)),
                )
                .child(
                    // Tab reaches it and the arrows step it, which is the only
                    // way to set a number a drag cannot hit.
                    focus::focusable(
                        theme,
                        &self.knobs[slot],
                        div()
                            .w(px(110.0))
                            .child(theme.slider(self.value(slot) / range)),
                    )
                    .id(element.clone())
                    .on_drag(SliderDrag(element.clone().into()), |_, _, _, cx| {
                        cx.new(|_| Empty)
                    })
                    .on_drag_move(cx.listener({
                        let element = element.clone();
                        move |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                            let Some(f) = widgets::slider_fraction(event, element.clone(), cx)
                            else {
                                return;
                            };
                            *view.knob(slot) = f * range;
                            cx.notify();
                        }
                    }))
                    .on_action(cx.listener(move |view, _: &focus::Decrement, _, cx| {
                        let v = view.knob(slot);
                        *v = (*v - STEP * range).clamp(0.0, range);
                        cx.notify();
                    }))
                    .on_action(cx.listener(
                        move |view, _: &focus::Increment, _, cx| {
                            let v = view.knob(slot);
                            *v = (*v + STEP * range).clamp(0.0, range);
                            cx.notify();
                        },
                    )),
                )
        };

        let rows = (0..KNOBS.len()).step_by(2).map(|slot| {
            div()
                .flex()
                .flex_row()
                .gap(px(10.0))
                .child(knob(slot))
                .children((slot + 1 < KNOBS.len()).then(|| knob(slot + 1)))
        });

        focus::traversal(
            div()
                .size_full()
                .flex()
                .flex_col()
                .bg(tuned.window_bg())
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
                        .children(self.coded.then(Self::code).into_iter().flatten())
                        .children(
                            (!self.coded)
                                .then(|| {
                                    (0..(WELL.1 / LINE_H) as usize).map(|line| {
                                        // Rotated per line so the block is not a
                                        // vertical grid, which would be as
                                        // periodic as the bars it replaced.
                                        div()
                                            .h(px(LINE_H))
                                            .pl(px(8.0))
                                            .whitespace_nowrap()
                                            .child(SPECIMEN[line * 5 % 17..].to_string())
                                    })
                                })
                                .into_iter()
                                .flatten(),
                        )
                        .child(floating::panel(
                            "glass",
                            &self.at,
                            Self::home(),
                            div()
                                .w(px(CARD))
                                .h(px(CARD))
                                .rounded(px(CARD_RADIUS))
                                .surface(&tuned, SurfaceStyle::Glass(Glass::Regular)),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .p(px(PAD))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(pick("regular", !clear).on_click(cx.listener(
                                    |view, _, _, cx| {
                                        view.load(Glass::Regular, cx);
                                        cx.notify();
                                    },
                                )))
                                .child(pick("clear", clear).on_click(cx.listener(
                                    |view, _, _, cx| {
                                        view.load(Glass::Clear, cx);
                                        cx.notify();
                                    },
                                )))
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
                                            view.load(Glass::Regular, cx);
                                            view.frost = Theme::of(cx).vibrancy_alpha;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(pick("frosted", frosted).on_click(cx.listener(
                                    |_, _, _, cx| {
                                        Self::set_frosted(true, cx);
                                        cx.notify();
                                    },
                                )))
                                .child(pick("opaque", !frosted).on_click(cx.listener(
                                    |_, _, _, cx| {
                                        Self::set_frosted(false, cx);
                                        cx.notify();
                                    },
                                )))
                                .child(div().w(px(12.0)))
                                .child(pick("text", !self.coded).on_click(cx.listener(
                                    |view, _, _, cx| {
                                        view.coded = false;
                                        cx.notify();
                                    },
                                )))
                                .child(pick("coded", self.coded).on_click(cx.listener(
                                    |view, _, _, cx| {
                                        view.coded = true;
                                        cx.notify();
                                    },
                                ))),
                        )
                        .children(rows)
                        .child(
                            div()
                                .text_style(TextStyle::Caption)
                                .text_color(theme.text_muted)
                                .child(readout),
                        ),
                ),
        )
    }
}
