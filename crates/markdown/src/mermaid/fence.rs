//! ` ```mermaid ` — the diagram, drawn as a read-only canvas in a fence's box,
//! or as a chart for the kinds [`chart::mermaid`] reads.
//!
//! Parsing and layout cost milliseconds and paint runs every frame, so each
//! source is laid out once per text size and kept. Zoom scales the drawing of
//! that one layout.
//!
//! A press on the diagram pans it and does not reach the editor; the band
//! above it does, which is how the source is reached for editing.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
};

use canvas_core::Canvas;
use gpui::{
    AnyElement, App, ElementId, Entity, Global, MouseButton, MouseDownEvent, MouseMoveEvent,
    Pixels, Point, SharedString, Size, Window, div, prelude::*, px,
};
use theme::{TextStyle, Theme};
use ui::{
    icons::glyph,
    tooltip::Tooltip,
    widgets::{ButtonStyle, Buttons as _},
};

use crate::render::{copy_button, fence_band, fence_panel};

pub const LANGUAGE: &str = "mermaid";

/// The debug selector on the drawn diagram, inside the box [`LANGUAGE`]
/// selects.
pub const DIAGRAM_SELECTOR: &str = "mermaid-diagram";

/// Diagrams kept laid out. Past this the cache starts over.
const CACHED: usize = 64;

/// How far the corner buttons zoom a diagram. `cx.set_mermaid_zoom(zoom)`
/// installs it for the app; [`Zoom::default`] stands before that. A diagram
/// larger than its box zooms out past [`Self::min`] to the scale at which it
/// fits, and opens there.
///
/// `min <= 1.0 <= max` and `step > 1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    /// The farthest out, as a scale of the diagram's own size.
    pub min: f32,
    /// The closest in.
    pub max: f32,
    /// One button press's zoom.
    pub step: f32,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            min: 0.5,
            max: 2.0,
            step: 1.25,
        }
    }
}

impl Zoom {
    pub(crate) fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }

    /// One press from `scale`, in or out, between `floor` and [`Self::max`].
    /// A press across 100% stops there.
    fn press(&self, scale: f32, inward: bool, floor: f32) -> f32 {
        let next = match inward {
            true => scale * self.step,
            false => scale / self.step,
        };
        let next = match (scale < 1.0) != (next < 1.0) && scale != 1.0 {
            true => 1.0,
            false => next,
        };
        next.clamp(floor, self.max)
    }
}

/// The largest scale, up to 1, at which `natural` fits in `frame`.
fn fit(natural: (f32, f32), frame: Size<Pixels>) -> f32 {
    let ratio = (f32::from(frame.width) / natural.0).min(f32::from(frame.height) / natural.1);
    match ratio > 0.0 {
        true => ratio.min(1.0),
        false => 1.0,
    }
}

struct Installed(Zoom);

impl Global for Installed {}

/// `cx.set_mermaid_zoom(my_zoom)` — call once at boot.
pub(crate) fn set_zoom(cx: &mut App, zoom: Zoom) {
    cx.set_global(Installed(zoom));
}

/// The tallest the diagram stands on its own, whatever its height. A height
/// the fence states is the whole box's, band included, taken as it is.
const MAX_HEIGHT: f32 = 480.0;

/// Laid-out diagrams by source and text size. `None` for source that did not
/// lay out, so a fence being typed is not parsed again every frame.
#[derive(Default)]
struct Cache(HashMap<u64, Option<Rc<Canvas>>>);

impl Global for Cache {}

/// One diagram's view: its scale, `None` until a zoom button is pressed; how
/// far it is panned; the press a pan is following — where the pointer and the
/// pan were when it went down; and the box's size as last frame laid it out.
#[derive(Clone, Copy, Default)]
struct View {
    zoom: Option<f32>,
    pan: Point<Pixels>,
    held: Option<(Point<Pixels>, Point<Pixels>)>,
    frame: Option<Size<Pixels>>,
}

fn hash(code: &str, size: f32) -> u64 {
    let mut hasher = DefaultHasher::new();
    (code, size.to_bits()).hash(&mut hasher);
    hasher.finish()
}

fn laid_out(code: &str, size: f32, cx: &mut App) -> Option<Rc<Canvas>> {
    let key = hash(code, size);
    let cache = &mut cx.default_global::<Cache>().0;
    if let Some(canvas) = cache.get(&key) {
        return canvas.clone();
    }
    if cache.len() >= CACHED {
        cache.clear();
    }
    let canvas = canvas_core::mermaid::import(code, size).map(Rc::new);
    cache.insert(key, canvas.clone());
    canvas
}

/// The diagram `code` describes, or `None` to leave the fence to its source.
pub fn render(
    code: &str,
    height: Option<u32>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let size = TextStyle::Callout.painted();
    // The box keeps the unzoomed diagram's height, so zooming moves what is in
    // it rather than the page under it.
    let Some(canvas) = laid_out(code, size, cx) else {
        return crate::charts::render(LANGUAGE, code, chart::mermaid::import, height, window, cx);
    };
    let natural = canvas_core::diagram::size_of(&canvas)?;
    let key = hash(code, size);
    let id = format!("mermaid-{key:x}");
    let state = window.use_keyed_state(
        ElementId::from(SharedString::from(id.clone())),
        cx,
        |_, _| View::default(),
    );
    let view = *state.read(cx);
    let zoom = Zoom::of(cx);
    let fit = view.frame.map_or(1.0, |frame| fit(natural, frame));
    let floor = zoom.min.min(fit);
    // Until a press, a diagram larger than its box opens fitted to it.
    let scale = view.zoom.unwrap_or(fit).clamp(floor, zoom.max);
    let theme = Theme::of(cx).clone();

    let (press, follow, release, release_out, measure) = (
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
    );
    let viewport = div()
        .id(SharedString::from(format!("{id}-view")))
        .debug_selector(|| LANGUAGE.into())
        .relative()
        .w_full()
        .map(|el| match height {
            Some(_) => el.flex_1().min_h_0(),
            None => el.h(px(natural.1.min(MAX_HEIGHT))),
        })
        .overflow_hidden()
        .map(|el| match view.held {
            Some(_) => el.cursor_grabbing(),
            None => el.cursor_grab(),
        })
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
            cx.stop_propagation();
            press.update(cx, |view, cx| {
                view.held = Some((event.position, view.pan));
                cx.notify();
            });
        })
        .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
            let Some((from, pan)) = follow.read(cx).held else {
                return;
            };
            if !event.dragging() {
                return;
            }
            follow.update(cx, |view, cx| {
                view.pan = pan + (event.position - from);
                cx.notify();
            });
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| end_pan(&release, cx))
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| end_pan(&release_out, cx))
        .child(
            gpui::canvas(
                move |bounds, _, cx| {
                    measure.update(cx, |view, cx| {
                        if view.frame != Some(bounds.size) {
                            view.frame = Some(bounds.size);
                            cx.notify();
                        }
                    })
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(
            div()
                .debug_selector(|| DIAGRAM_SELECTOR.into())
                .absolute()
                .left(view.pan.x)
                .top(view.pan.y)
                .child(canvas_core::diagram(&canvas, scale, cx)),
        )
        .child(controls(&id, &state, view, scale, floor, zoom, &theme));

    Some(
        fence_panel(&theme)
            .when_some(height, |el, height| {
                el.h(px(height as f32)).flex().flex_col()
            })
            .child(
                fence_band(&theme)
                    .flex_none()
                    .text_color(theme.text_muted)
                    .child(LANGUAGE),
            )
            .child(viewport)
            .child(copy_button(code, key as usize, &theme, window, cx))
            .into_any_element(),
    )
}

fn end_pan(state: &Entity<View>, cx: &mut App) {
    state.update(cx, |view, cx| {
        if view.held.take().is_some() {
            cx.notify();
        }
    });
}

/// Zoom out, zoom in and reset, in the box's bottom-right corner. `floor` is
/// the farthest out `scale` may go.
fn controls(
    id: &str,
    state: &Entity<View>,
    view: View,
    scale: f32,
    floor: f32,
    zoom: Zoom,
    theme: &Theme,
) -> AnyElement {
    let button = |name: &str,
                  icon: &'static [u8],
                  tip: &'static str,
                  enabled: bool,
                  act: Box<dyn Fn(&mut View)>| {
        let state = state.clone();
        theme
            .icon_button(icon, ButtonStyle::Ghost, None)
            .id(SharedString::from(format!("{id}-{name}")))
            .when(!enabled, |el| el.opacity(0.4))
            .tooltip(move |window, cx| Tooltip::text(tip, window, cx))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                if enabled {
                    state.update(cx, |view, cx| {
                        act(view);
                        cx.notify();
                    });
                }
            })
    };
    theme
        .control_group()
        .absolute()
        .right(px(6.0))
        .bottom(px(6.0))
        .cursor_default()
        .child(button(
            "zoom-out",
            glyph::ZoomOut,
            "Zoom out",
            scale > floor,
            Box::new(move |view| view.zoom = Some(zoom.press(scale, false, floor))),
        ))
        .child(button(
            "zoom-in",
            glyph::ZoomIn,
            "Zoom in",
            scale < zoom.max,
            Box::new(move |view| view.zoom = Some(zoom.press(scale, true, floor))),
        ))
        .child(button(
            "reset",
            glyph::RotateCcw,
            "Reset view",
            view.zoom.is_some() || view.pan != Point::default(),
            Box::new(|view| {
                view.zoom = None;
                view.pan = Point::default();
            }),
        ))
        .into_any_element()
}
