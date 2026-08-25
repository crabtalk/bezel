//! The composer: hue, chroma and radius over the shipped palette, and the code
//! that reproduces what you are looking at.
//!
//! The page keeps no palette of its own. Every knob writes
//! [`theme::set_brand`], every readout builds from
//! [`Theme::branded`](theme::Theme::branded), and the snippet at the bottom
//! prints the same [`Brand`] both of those used — so the code you copy is the
//! thing on screen rather than a second description of it.

use gpui::{
    AnyElement, App, Context, DragMoveEvent, Empty, SharedString, Window, div, prelude::*, px,
};
use theme::{Appearance, BASE_COLORS, Brand, Theme};
use ui::{
    focus, popover,
    widgets::{self, ButtonStyle, Buttons, Content, Controls, SliderDrag},
};

use crate::{Gallery, stack};

/// How many focus handles the page needs — one per row of [`KNOBS`], so adding
/// a knob cannot outrun the handles the view holds for them.
pub const KNOB_COUNT: usize = KNOBS.len();

/// One slider. Six near-identical rows collapse to a table plus a loop, and
/// adding a knob is a row rather than another forty lines of drag wiring.
struct Knob {
    label: &'static str,
    id: &'static str,
    max: f32,
    /// What one press of ← or → is worth.
    step: f32,
    decimals: usize,
    read: fn(&Brand) -> f32,
    write: fn(&mut Brand, f32),
}

/// The ranges are the palette's own extremes, not round numbers: the widest
/// neutral is Slate at chroma 0.046, the shipped accents peak at red-400's
/// 0.191, and the largest corner in the library is the message bubble at twice
/// the base radius.
const KNOBS: [Knob; 6] = [
    Knob {
        label: "Base hue",
        id: "tint-hue",
        max: 360.0,
        step: 5.0,
        decimals: 0,
        read: |b| b.tint.hue,
        write: |b, v| b.tint.hue = v,
    },
    Knob {
        label: "Base chroma",
        id: "tint-chroma",
        max: 0.06,
        step: 0.002,
        decimals: 3,
        read: |b| b.tint.chroma,
        write: |b, v| b.tint.chroma = v,
    },
    Knob {
        label: "Accent hue",
        id: "accent-hue",
        max: 360.0,
        step: 5.0,
        decimals: 0,
        read: |b| b.accent.hue,
        write: |b, v| b.accent.hue = v,
    },
    Knob {
        label: "Accent chroma",
        id: "accent-chroma",
        max: 0.2,
        step: 0.005,
        decimals: 3,
        read: |b| b.accent.chroma,
        write: |b, v| b.accent.chroma = v,
    },
    Knob {
        label: "Radius",
        id: "radius",
        max: 16.0,
        step: 1.0,
        decimals: 1,
        read: |b| b.radius,
        write: |b, v| b.radius = v,
    },
    // Left is more desktop. At 1.00 glass is off and the window composites
    // opaque again, which is what every platform but macOS starts at.
    Knob {
        label: "Frost",
        id: "glass",
        max: 1.0,
        step: 0.05,
        decimals: 2,
        read: |b| b.glass,
        write: |b, v| b.glass = v,
    },
];

/// Which of the three files the code panel is showing, and the fence tag that
/// colors it.
const FILES: [(&str, &str); 3] = [
    ("Brand", "rust"),
    ("Cargo.toml", "toml"),
    ("main.rs", "rust"),
];

pub fn page(
    view: &Gallery,
    theme: &Theme,
    window: &mut Window,
    cx: &mut Context<Gallery>,
) -> AnyElement {
    let brand = theme::brand(cx);
    stack()
        .child(popover::menu_heading(theme, "Base color"))
        .child(presets(theme, &brand, cx))
        .children(
            KNOBS.iter().enumerate().map(|(index, knob)| {
                slider(view, theme, &brand, knob, index, cx).into_any_element()
            }),
        )
        .child(popover::menu_heading(theme, "Contrast"))
        .child(contrast(theme, &brand))
        .child(popover::menu_heading(theme, "Specimen"))
        .child(specimen(theme))
        .child(popover::menu_heading(theme, "Copy"))
        .child(files(view, theme, cx))
        .child(code(view.create_file, &brand, window, cx))
        .into_any_element()
}

fn presets(theme: &Theme, brand: &Brand, cx: &mut Context<Gallery>) -> AnyElement {
    theme
        .toggle_group()
        .children(BASE_COLORS.iter().map(|(name, tint)| {
            let tint = *tint;
            let mut next = *brand;
            next.tint = tint;
            theme
                .toggle_group_item(*name, brand.tint == tint)
                .id(SharedString::from(*name))
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    theme::set_brand(next, cx);
                    cx.notify();
                }))
        }))
        .into_any_element()
}

fn slider(
    view: &Gallery,
    theme: &Theme,
    brand: &Brand,
    knob: &'static Knob,
    index: usize,
    cx: &mut Context<Gallery>,
) -> AnyElement {
    let value = (knob.read)(brand);
    let nudge = move |cx: &mut Context<Gallery>, by: f32| {
        let mut next = theme::brand(cx);
        let value = ((knob.read)(&next) + by).clamp(0.0, knob.max);
        (knob.write)(&mut next, value);
        theme::set_brand(next, cx);
        cx.notify();
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .w(px(96.0))
                .flex_none()
                .text_size(px(12.5))
                .text_color(theme.text_muted)
                .child(knob.label),
        )
        .child(
            div().w(px(240.0)).flex_none().child(
                focus::focusable(
                    theme,
                    &view.brand_knobs[index],
                    theme.slider(value / knob.max),
                )
                .id(knob.id)
                .on_drag(SliderDrag(knob.id.into()), |_, _, _, cx| cx.new(|_| Empty))
                .on_drag_move(
                    cx.listener(move |_, event: &DragMoveEvent<SliderDrag>, _, cx| {
                        let Some(fraction) = widgets::slider_fraction(event, knob.id, cx) else {
                            return;
                        };
                        let mut next = theme::brand(cx);
                        (knob.write)(&mut next, fraction * knob.max);
                        theme::set_brand(next, cx);
                        cx.notify();
                    }),
                )
                .on_action(cx.listener(move |_, _: &focus::Decrement, _, cx| nudge(cx, -knob.step)))
                .on_action(cx.listener(move |_, _: &focus::Increment, _, cx| nudge(cx, knob.step))),
            ),
        )
        .child(
            div()
                .w(px(56.0))
                .flex_none()
                .text_size(px(12.0))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(SharedString::from(format!("{value:.*}", knob.decimals))),
        )
        .into_any_element()
}

/// What a row measures: the two tokens, read off one appearance's palette.
type Pairing = fn(&Theme) -> (gpui::Hsla, gpui::Hsla);

/// The four pairings a brand can break, in both appearances at once. WCAG AA
/// for body copy is 4.5; the palette's own `text_faint` is tuned to exactly
/// that, so it is the line drawn here too.
const PAIRS: [(&str, Pairing); 4] = [
    ("text / bg", |t| (t.text, t.bg)),
    ("muted / surface", |t| (t.text_muted, t.surface)),
    ("accent / bg", |t| (t.accent, t.bg)),
    ("label / accent plate", |t| (t.on_accent, t.accent_strong)),
];

fn contrast(theme: &Theme, brand: &Brand) -> AnyElement {
    let dark = Theme::branded(brand, Appearance::Dark);
    let light = Theme::branded(brand, Appearance::Light);
    stack()
        .gap(px(4.0))
        .children(PAIRS.iter().map(|(label, pick)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .w(px(150.0))
                        .flex_none()
                        .text_size(px(12.5))
                        .text_color(theme.text_muted)
                        .child(*label),
                )
                .children([&dark, &light].map(|source| {
                    let (fg, bg) = pick(source);
                    let ratio = theme::contrast_ratio(fg, bg);
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .w(px(120.0))
                        .flex_none()
                        .child(
                            div()
                                .w(px(46.0))
                                .flex_none()
                                .text_size(px(12.0))
                                .font_family(theme.font_mono.clone())
                                .text_color(theme.text)
                                .child(SharedString::from(format!("{ratio:.2}"))),
                        )
                        .child(widgets::status_dot(if ratio >= 4.5 {
                            source.success
                        } else {
                            source.danger
                        }))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(theme.text_faint)
                                .child(if source.appearance.is_dark() {
                                    "dark"
                                } else {
                                    "light"
                                }),
                        )
                }))
                .into_any_element()
        }))
        .into_any_element()
}

fn specimen(theme: &Theme) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(10.0))
        .child(theme.button("Prominent", ButtonStyle::Prominent, None))
        .child(theme.button("Ghost", ButtonStyle::Ghost, None))
        .child(theme.badge("Badge"))
        .child(theme.tag("tag"))
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .rounded(px(Theme::button_radius()))
                .bg(theme.input_bg)
                .border_1()
                .border_color(theme.border)
                .text_size(px(12.5))
                .text_color(theme.text_faint)
                .child("input plate"),
        )
        .child(
            div()
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(Theme::control_radius()))
                .bg(theme.code_wash)
                .font_family(theme.font_mono.clone())
                .text_size(px(12.0))
                .text_color(theme.code_text)
                .child("code"),
        )
        .into_any_element()
}

fn files(view: &Gallery, theme: &Theme, cx: &mut Context<Gallery>) -> AnyElement {
    theme
        .toggle_group()
        .children(FILES.iter().enumerate().map(|(index, (name, _))| {
            theme
                .toggle_group_item(*name, view.create_file == index)
                .id(SharedString::from(*name))
                .cursor_pointer()
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.create_file = index;
                    cx.notify();
                }))
        }))
        .into_any_element()
}

/// Fenced, so the markdown block colors it and carries the copy button — the
/// snippet on screen and the one on the clipboard are then the same string by
/// construction.
fn code(file: usize, brand: &Brand, window: &mut Window, cx: &mut App) -> AnyElement {
    let (_, tag) = FILES[file];
    markdown::markdown(
        &format!("```{tag}\n{}\n```", source(file, brand)),
        window,
        cx,
    )
}

/// The three files, printed from the live brand.
fn source(file: usize, brand: &Brand) -> String {
    match file {
        1 => CARGO.to_string(),
        2 => MAIN.replace("{brand}", &indent(&call(brand), "        ")),
        _ => call(brand),
    }
}

fn call(brand: &Brand) -> String {
    format!(
        "use bezel::theme::{{self, Brand, Tint}};\n\n\
         // Before `appearance::init`, which installs the first palette.\n\
         theme::set_brand(\n    \
             Brand {{\n        \
                 tint: Tint::new({:.3}, {:.3}),\n        \
                 accent: Tint::new({:.3}, {:.3}),\n        \
                 radius: {:.1},\n        \
                 glass: {:.2},\n    \
             }},\n    \
             cx,\n\
         );",
        brand.tint.hue,
        brand.tint.chroma,
        brand.accent.hue,
        brand.accent.chroma,
        brand.radius,
        brand.glass,
    )
}

fn indent(text: &str, pad: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                line.to_string()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_start()
        .to_string()
}

const CARGO: &str = "\
[dependencies]
bezel = \"0.1\"
# `actions!` expands to literal `gpui::` paths, so the crate name has to be in
# scope even where no type is named through it.
gpui = { package = \"bezel-gpui\", version = \"0.3\" }

# The facade re-exports gpui but not the platform, and booting a native window
# needs it.
[target.'cfg(not(target_family = \"wasm\"))'.dependencies]
gpui_platform = { package = \"bezel-gpui-platform\", version = \"0.3\", features = [\"font-kit\"] }";

const MAIN: &str = "\
use bezel::gpui::{
    App, AppContext as _, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*,
    px, size,
};
use bezel::theme::{self, Theme};
use bezel::ui;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        ui::register_fonts(cx).ok();
        {brand}
        theme::appearance::init(theme::appearance::AppearanceMode::System, cx);
        let bounds = Bounds::centered(None, size(px(520.0), px(360.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                theme::appearance::observe_window(window, cx).detach();
                cx.new(|_| Root)
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

struct Root;

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.bg)
            .text_color(theme.text)
            .child(\"hello, bezel\")
    }
}";
