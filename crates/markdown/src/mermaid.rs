//! ` ```mermaid ` — the diagram, drawn as a read-only canvas in a fence's box,
//! or as a chart for the kinds [`chart::mermaid`] reads.
//!
//! Parsing and layout cost milliseconds and paint runs every frame, so each
//! source is laid out once per text size and kept. Zoom lays the source out
//! again at a scaled text size, so each zoom step is one more cache entry.
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
    Pixels, Point, SharedString, Window, div, prelude::*, px,
};
use theme::{TextStyle, Theme};
use ui::{
    icons::glyph,
    tooltip::Tooltip,
    widgets::{ButtonStyle, Buttons as _},
};

use crate::render::{copy_button, fence_band, fence_panel};

pub const LANGUAGE: &str = "mermaid";

/// Diagrams kept laid out. Past this the cache starts over.
const CACHED: usize = 64;

/// The zoom steps the corner buttons walk, and where a diagram starts.
const ZOOMS: [f32; 7] = [0.5, 0.67, 0.8, 1.0, 1.25, 1.5, 2.0];
const UNZOOMED: usize = 3;

/// The tallest the diagram stands on its own, whatever its height. A height
/// the fence states is the whole box's, band included, taken as it is.
const MAX_HEIGHT: f32 = 480.0;

/// Laid-out diagrams by source and text size. `None` for source that did not
/// lay out, so a fence being typed is not parsed again every frame.
#[derive(Default)]
struct Cache(HashMap<u64, Option<Rc<Canvas>>>);

impl Global for Cache {}

/// One diagram's view: its zoom step, how far it is panned, and the press a
/// pan is following — where the pointer and the pan were when it went down.
#[derive(Clone, Copy)]
struct View {
    zoom: usize,
    pan: Point<Pixels>,
    held: Option<(Point<Pixels>, Point<Pixels>)>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: UNZOOMED,
            pan: Point::default(),
            held: None,
        }
    }
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
    let Some(base) = laid_out(code, size, cx) else {
        return crate::charts::render(LANGUAGE, code, chart::mermaid::import, height, window, cx);
    };
    let (_, natural) = canvas_core::diagram::size_of(&base)?;
    let key = hash(code, size);
    let id = format!("mermaid-{key:x}");
    let state = window.use_keyed_state(
        ElementId::from(SharedString::from(id.clone())),
        cx,
        |_, _| View::default(),
    );
    let view = *state.read(cx);
    let canvas = match view.zoom {
        UNZOOMED => base,
        step => laid_out(code, size * ZOOMS[step], cx)?,
    };
    let theme = Theme::of(cx).clone();

    let (press, follow, release, release_out) =
        (state.clone(), state.clone(), state.clone(), state.clone());
    let viewport = div()
        .id(SharedString::from(format!("{id}-view")))
        .debug_selector(|| LANGUAGE.into())
        .relative()
        .w_full()
        .map(|el| match height {
            Some(_) => el.flex_1().min_h_0(),
            None => el.h(px(natural.min(MAX_HEIGHT))),
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
            div()
                .absolute()
                .left(view.pan.x)
                .top(view.pan.y)
                .child(canvas_core::diagram(&canvas, cx)),
        )
        .child(controls(&id, &state, view, &theme));

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

/// Zoom out, zoom in and reset, in the box's bottom-right corner.
fn controls(id: &str, state: &Entity<View>, view: View, theme: &Theme) -> AnyElement {
    let button =
        |name: &str, icon: &'static [u8], tip: &'static str, enabled: bool, act: fn(&mut View)| {
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
            view.zoom > 0,
            |view| view.zoom -= 1,
        ))
        .child(button(
            "zoom-in",
            glyph::ZoomIn,
            "Zoom in",
            view.zoom + 1 < ZOOMS.len(),
            |view| view.zoom += 1,
        ))
        .child(button(
            "reset",
            glyph::RotateCcw,
            "Reset view",
            view.zoom != UNZOOMED || view.pan != Point::default(),
            |view| *view = View::default(),
        ))
        .into_any_element()
}
