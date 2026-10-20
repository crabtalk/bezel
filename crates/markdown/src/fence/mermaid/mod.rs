//! ` ```mermaid ` — the diagram [`::mermaid`] draws, painted from its SVG
//! in a fence's box.
//!
//! Drawing and parsing the SVG cost milliseconds and paint runs every frame,
//! so each source is drawn once per text size, theme and font and kept, and
//! rasterized again only when its scale on screen changes. Drawing and
//! parsing run on gpui's background executor, each fence's on its own; until
//! they finish the fence shows the diagram it last drew, or stands empty at
//! its stated height. The SVG names the theme's colours, and its labels are
//! measured and drawn in [`ui::fonts`] ([`fonts`]). Source that does not draw
//! shows as code, with the error under it.
//!
//! A press on the diagram pans it and does not reach the editor; the band
//! above it does, which is how the source is reached for editing.

mod fonts;

use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
    sync::Arc,
};

use gpui::{
    AnyElement, App, AsyncApp, ElementId, Entity, Global, Hsla, ImageSource, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Point, RenderImage, Rgba as GpuiRgba, SharedString,
    Size, Window, div, img, prelude::*, px,
};
use theme::{TextStyle, Theme};
use ui::{
    icons::glyph,
    tooltip::Tooltip,
    widgets::{ButtonStyle, Buttons as _},
};

use ::mermaid::{Colors, Options, Rgba};

use self::fonts::{Families, Shaper};
use super::{copy_button, fence_band, fence_panel};

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

/// A drawn diagram, or the error's text for source that did not draw, so a
/// fence being typed is not parsed again every frame.
type Drawing = Result<Rc<Diagram>, SharedString>;

/// Diagrams by key: drawn, or `None` while being drawn.
#[derive(Default)]
struct Cache(HashMap<u64, Option<Drawing>>);

impl Global for Cache {}

/// A drawn diagram: its SVG parsed, its size, and its last raster with the
/// device scale that was made at.
struct Diagram {
    svg: usvg::Tree,
    natural: (f32, f32),
    raster: RefCell<Option<(f32, Arc<RenderImage>)>>,
}

impl Diagram {
    /// The diagram at `scale` device pixels to its own, made again only when
    /// the scale moves.
    fn raster(&self, scale: f32, window: &mut Window, cx: &mut App) -> Option<Arc<RenderImage>> {
        if let Some((at, image)) = &*self.raster.borrow()
            && *at == scale
        {
            return Some(image.clone());
        }
        let image = rasterize(&self.svg, scale)?;
        if let Some((_, old)) = self.raster.replace(Some((scale, image.clone()))) {
            cx.drop_image(old, Some(window));
        }
        Some(image)
    }
}

/// `svg` at `scale` device pixels to its own, as gpui takes an image: BGRA,
/// not premultiplied.
fn rasterize(svg: &usvg::Tree, scale: f32) -> Option<Arc<RenderImage>> {
    let size = svg.size().to_int_size().scale_by(scale)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(
        svg,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut pixels = pixmap.take();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        gpui::swap_rgba_pa_to_bgra(pixel);
    }
    let buffer = image::RgbaImage::from_raw(size.width(), size.height(), pixels)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

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

fn rgba(color: Hsla) -> Rgba {
    let GpuiRgba { r, g, b, a } = color.into();
    [r, g, b, a].map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// The theme's colours for the `--bezel-*` tokens.
fn colors(theme: &Theme) -> Colors {
    Colors {
        bg: rgba(theme.bg),
        fg: rgba(theme.text),
        muted: rgba(theme.text_muted),
        line: rgba(theme.text),
        surface: rgba(theme.surface),
        surface_alt: rgba(theme.element_active),
        border: rgba(theme.border_strong),
        accent: rgba(theme.accent),
        critical: rgba(theme.danger),
        series: std::array::from_fn(|index| rgba(theme.categorical(index))),
    }
}

/// `code` drawn at text size `size` in the theme's colours and fonts, kept;
/// `None` while it is being drawn, the windows refreshed when it is.
fn drawn(
    code: &str,
    size: f32,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> (u64, Option<Drawing>) {
    let fonts = ui::fonts::database();
    let families = Families::resolve(&theme.font_sans, &theme.font_mono, &fonts);
    let colors = colors(theme);
    let mut hasher = DefaultHasher::new();
    (code, size.to_bits(), &colors, &families, fonts.generation).hash(&mut hasher);
    let key = hasher.finish();
    if let Some(found) = cx.default_global::<Cache>().0.get(&key) {
        return (key, found.clone());
    }
    let cache = std::mem::take(&mut cx.default_global::<Cache>().0);
    let cache = if cache.len() >= CACHED {
        for diagram in cache.into_values().flatten().flatten() {
            if let Some((_, image)) = diagram.raster.take() {
                cx.drop_image(image, Some(window));
            }
        }
        HashMap::new()
    } else {
        cache
    };
    cx.default_global::<Cache>().0 = cache;
    let parsing = usvg::Options {
        font_family: families.sans.clone(),
        fontdb: fonts.fonts.clone(),
        ..usvg::Options::default()
    };
    let options = Options {
        font_family: Some(families.sans.clone()),
        font_family_mono: Some(families.mono.clone()),
        font_size: f64::from(size),
        colors: Some(colors),
        ..Options::default()
    };
    let shaper = Shaper { families, fonts };
    let code = code.to_string();
    let job = cx.background_executor().spawn(async move {
        let drawing = ::mermaid::render(&code, &options, &shaper).map_err(|error| note(&error))?;
        let svg = usvg::Tree::from_data(drawing.svg.as_bytes(), &parsing)
            .map_err(|error| SharedString::from(error.to_string()))?;
        Ok((svg, (drawing.width as f32, drawing.height as f32)))
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let result: Result<(usvg::Tree, (f32, f32)), SharedString> = job.await;
        cx.update(|cx| {
            let drawing = result.map(|(svg, natural)| {
                Rc::new(Diagram {
                    svg,
                    natural,
                    raster: RefCell::new(None),
                })
            });
            cx.default_global::<Cache>().0.insert(key, Some(drawing));
            cx.refresh_windows();
        });
    })
    .detach();
    cx.default_global::<Cache>().0.insert(key, None);
    (key, None)
}

/// What a fence shows under source that did not draw: the error's message,
/// and the line it points at where the message does not show it.
fn note(error: &::mermaid::Error) -> SharedString {
    let mut text = error.message.clone();
    if !error.snippet.is_empty() && !text.contains(&error.snippet) {
        text.push('\n');
        text.push_str(&error.snippet);
    }
    text.into()
}

/// The diagram `code` describes, or the note its source is shown with. `block`
/// is the fence's place in its document.
pub(crate) fn render(
    code: &str,
    height: Option<u32>,
    block: usize,
    window: &mut Window,
    cx: &mut App,
) -> Result<AnyElement, SharedString> {
    let size = TextStyle::Callout.painted();
    let theme = Theme::of(cx).clone();
    let (key, drawing) = drawn(code, size, &theme, window, cx);
    // The diagram this fence last showed and its view's id, shown while the
    // next is drawn.
    let last = window.use_keyed_state(
        ElementId::from(SharedString::from(format!("mermaid-last-{block}"))),
        cx,
        |_, _| None::<(String, Rc<Diagram>)>,
    );
    let (id, diagram) = match drawing {
        Some(drawing) => {
            let diagram = drawing?;
            // The view outlives a change of theme or font; the drawing does not.
            let mut hasher = DefaultHasher::new();
            (code, size.to_bits()).hash(&mut hasher);
            let id = format!("mermaid-{:x}", hasher.finish());
            if !last
                .read(cx)
                .as_ref()
                .is_some_and(|(_, shown)| Rc::ptr_eq(shown, &diagram))
            {
                last.update(cx, |last, _| *last = Some((id.clone(), diagram.clone())));
            }
            (id, diagram)
        }
        None => match last.read(cx).clone() {
            Some(shown) => shown,
            None => {
                return Ok(fence_panel(&theme)
                    .when_some(height, |el, height| el.h(px(height as f32)))
                    .child(
                        fence_band(&theme)
                            .flex_none()
                            .text_color(theme.text_muted)
                            .child(LANGUAGE),
                    )
                    .into_any_element());
            }
        },
    };
    // The box keeps the unzoomed diagram's height, so zooming moves what is in
    // it rather than the page under it.
    let natural = diagram.natural;
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
    let image = diagram
        .raster(scale * window.scale_factor(), window, cx)
        .ok_or_else(|| SharedString::from("The diagram could not be drawn."))?;

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
                .child(
                    img(ImageSource::Render(image))
                        .w(px(natural.0 * scale))
                        .h(px(natural.1 * scale)),
                ),
        )
        .child(controls(&id, &state, view, scale, floor, zoom, &theme));

    Ok(fence_panel(&theme)
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
        .into_any_element())
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
