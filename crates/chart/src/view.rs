//! A [`Chart`] drawn as one element: laid out once per data generation, size
//! and text style, then painted every frame from what was kept.
//!
//! A click on a mark with an `href` reports the url to the function installed
//! with [`set_on_href`], and opens nothing. Without one it does nothing.
//!
//! An image mark paints what the function installed with [`set_image_loader`]
//! answers for its url, and nothing without one.

use crate::{
    model::{Align, Baseline, Cap, Chart, Encoding, Join, Mark, Properties, Shape, Title},
    plan::{self, Face, Fill, Glyph, Label, Marks, Metrics, Paint, Plan, Rect, Slice, Tip},
};
use d3::shape::{arc::Sector, curve::Curve, path::Segment};
use gpui::{
    App, Bounds, ContentMask, Corners, CursorStyle, DispatchPhase, Element, ElementId, FillOptions,
    Font, FontStyle, Global, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, Path, PathBuilder,
    PathStyle, Pixels, Point, Refineable, RenderImage, ShapedLine, SharedString, StrokeOptions,
    Style, StyleRefinement, Styled, TextAlign, TextRun, Window, fill, linear_color_stop,
    linear_gradient, point, px, relative, size,
};
use lyon::tessellation::{LineCap, LineJoin};
use std::{cell::Cell, collections::HashMap, f32::consts::PI, rc::Rc, sync::Arc};
use theme::{TextStyle, Theme};

/// The height a chart takes when nothing sizes it.
const HEIGHT: f32 = 240.0;
const HOVER_POINT: f32 = 8.0;
const AREA_FILL: f32 = 0.25;
/// How many straight segments draw half a turn of a trail's end.
const TRAIL_SEGMENTS: usize = 8;
const TIP_PAD: f32 = 8.0;
const TIP_OFFSET: f32 = 12.0;
const TIP_RADIUS: f32 = 6.0;
const BAND_WASH: f32 = 0.05;

/// `chart` as an element, full width and [`HEIGHT`] tall unless styled.
/// `id` keeps its layout and hover across frames, so it must be stable.
pub fn view(id: impl Into<ElementId>, chart: Chart) -> ChartView {
    let mut style = StyleRefinement::default();
    style.size.width = Some(relative(1.0).into());
    style.size.height = Some(px(HEIGHT).into());
    ChartView {
        id: id.into(),
        chart,
        style,
    }
}

/// What a click on a mark with an `href` does, given the url as written.
pub type OnHref = fn(&str, &mut Window, &mut App);

struct Installed(OnHref);

impl Global for Installed {}

/// Installs what a click on a mark with an `href` reports to, once at boot.
pub fn set_on_href(cx: &mut App, on_href: OnHref) {
    cx.set_global(Installed(on_href));
}

/// Answers an image mark's url, as the chart writes it, with the image at
/// it. Asked at paint. `None` for a url the app has nothing for yet: the
/// mark paints nothing until the app next repaints the chart.
pub type ImageLoader = fn(&str, &mut Window, &mut App) -> Option<Arc<RenderImage>>;

struct Images(ImageLoader);

impl Global for Images {}

/// Installs what answers image marks' urls, once at boot.
pub fn set_image_loader(cx: &mut App, loader: ImageLoader) {
    cx.set_global(Images(loader));
}

pub struct ChartView {
    id: ElementId,
    chart: Chart,
    style: StyleRefinement,
}

impl Styled for ChartView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for ChartView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// What a plan was made from. Equal keys make equal plans.
#[derive(Clone, PartialEq)]
struct Key {
    /// Each layer's data generation, mark, properties and encoding.
    layers: Vec<(u64, Mark, Properties, Encoding)>,
    title: Option<Title>,
    size: [f32; 2],
    font: Font,
    font_size: f32,
    colors: [Hsla; 2],
}

/// A plan and everything painting it needs that does not change with it:
/// what paints its guides and marks at the chart's own origin.
struct Built {
    key: Key,
    plan: Plan,
    /// The marks, in paint order, each with where it is cut off.
    marks: Vec<(Option<Rect>, Op)>,
    /// Axes, legends and the title, under the marks and over them, each
    /// with where it is cut off.
    under: Vec<(Option<Rect>, Op)>,
    over: Vec<(Option<Rect>, Op)>,
    line: f32,
}

/// One thing painted, at the chart's own origin.
enum Op {
    Path(Path<Pixels>, Paint),
    Quad {
        rect: Rect,
        /// Top-left, top-right, bottom-right, bottom-left.
        corners: [f32; 4],
        fill: Option<Paint>,
        border: Option<(Paint, f32)>,
    },
    /// A blend from one colour to the other, `angle` degrees clockwise
    /// from upward.
    Gradient {
        rect: Rect,
        angle: f32,
        colors: [Hsla; 2],
    },
    /// A shaped line with its top-left at `at`.
    Text {
        line: Box<ShapedLine>,
        at: [f32; 2],
    },
    Image(plan::Image),
}

impl Op {
    fn paint(&self, origin: Point<Pixels>, theme: &Theme, window: &mut Window, cx: &mut App) {
        match self {
            Op::Path(path, paint) => window.paint_path(moved(path, origin), paint.on(theme)),
            Op::Quad {
                rect,
                corners: [top_left, top_right, bottom_right, bottom_left],
                fill: inside,
                border,
            } => {
                let mut quad = fill(
                    bounds_of(*rect, origin),
                    inside.map_or(Hsla::transparent_black(), |paint| paint.on(theme)),
                )
                .corner_radii(Corners {
                    top_left: px(*top_left),
                    top_right: px(*top_right),
                    bottom_right: px(*bottom_right),
                    bottom_left: px(*bottom_left),
                });
                if let Some((paint, width)) = border {
                    quad = quad.border_widths(px(*width)).border_color(paint.on(theme));
                }
                window.paint_quad(quad);
            }
            Op::Gradient {
                rect,
                angle,
                colors: [from, to],
            } => window.paint_quad(fill(
                bounds_of(*rect, origin),
                linear_gradient(
                    *angle,
                    linear_color_stop(*from, 0.0),
                    linear_color_stop(*to, 1.0),
                ),
            )),
            Op::Text { line, at } => {
                let height = line.ascent + line.descent;
                line.paint(
                    origin + point(px(at[0]), px(at[1])),
                    height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
            Op::Image(image) => {
                let Some(Images(loader)) = cx.try_global::<Images>() else {
                    return;
                };
                let loader = *loader;
                let Some(data) = loader(&image.url, window, cx) else {
                    return;
                };
                let natural = data.size(0);
                let natural = [natural.width.0 as f32, natural.height.0 as f32];
                let placed = image.placed(natural);
                let bounds = bounds_of(placed, origin);
                window
                    .paint_image(bounds, bounds, Corners::default(), data, 0, false)
                    .ok();
            }
        }
    }
}

impl plan::Image {
    /// Where it paints, `natural` its own width and height: its box set
    /// against its anchor, and the image centred in the box at its aspect
    /// ratio when it keeps it.
    fn placed(&self, natural: [f32; 2]) -> Rect {
        let ratio = match natural[1] > 0.0 {
            true => natural[0] / natural[1],
            false => 1.0,
        };
        let (w, h) = match (self.width, self.height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) if self.aspect => (w, w / ratio),
            (None, Some(h)) if self.aspect => (h * ratio, h),
            (w, h) => (w.unwrap_or(natural[0]), h.unwrap_or(natural[1])),
        };
        let x = self.at[0]
            - w * match self.align {
                Align::Left => 0.0,
                Align::Center => 0.5,
                Align::Right => 1.0,
            };
        let y = self.at[1]
            - h * match self.baseline {
                Baseline::Top | Baseline::LineTop => 0.0,
                Baseline::Middle => 0.5,
                Baseline::Bottom | Baseline::Alphabetic | Baseline::LineBottom => 1.0,
            };
        let mut placed = Rect { x, y, w, h };
        if self.aspect && h > 0.0 && w / h != ratio {
            if ratio > w / h {
                placed.h = w / ratio;
                placed.y += (h - placed.h) / 2.0;
            } else {
                placed.w = h * ratio;
                placed.x += (w - placed.w) / 2.0;
            }
        }
        placed
    }
}

#[derive(Default)]
struct State {
    built: Option<Rc<Built>>,
    /// The pointer, from the chart's origin.
    hover: Rc<Cell<Option<[f32; 2]>>>,
}

pub struct Prepainted {
    built: Option<Rc<Built>>,
    hover: Rc<Cell<Option<[f32; 2]>>>,
    hitbox: Hitbox,
}

impl Element for ChartView {
    type RequestLayoutState = ();
    type PrepaintState = Prepainted;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.refine(&self.style);
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepainted {
        let theme = Theme::of(cx);
        let key = Key {
            layers: self
                .chart
                .layers
                .iter()
                .map(|layer| {
                    (
                        layer.data.generation(),
                        layer.mark,
                        layer.properties.clone(),
                        layer.encoding.clone(),
                    )
                })
                .collect(),
            title: self.chart.title.clone(),
            size: [bounds.size.width.as_f32(), bounds.size.height.as_f32()],
            font: window.text_style().font(),
            font_size: TextStyle::Caption.painted(),
            colors: [theme.text, theme.text_muted],
        };
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let id = id.expect("a chart has an id");
        window.with_element_state(id, |state: Option<State>, window| {
            let mut state = state.unwrap_or_default();
            if state.built.as_ref().is_none_or(|built| built.key != key) {
                state.built = build(&self.chart, key, theme, window).map(Rc::new);
            }
            let prepainted = Prepainted {
                built: state.built.clone(),
                hover: state.hover.clone(),
                hitbox,
            };
            (prepainted, state)
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        prepainted: &mut Prepainted,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(built) = prepainted.built.clone() else {
            return;
        };
        let origin = bounds.origin;
        let hover = prepainted.hover.clone();
        let hitbox = prepainted.hitbox.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            let at = hitbox.is_hovered(window).then(|| {
                let local = event.position - origin;
                [local.x.as_f32(), local.y.as_f32()]
            });
            if hover.replace(at) != at {
                window.refresh();
            }
        });

        let hitbox = prepainted.hitbox.clone();
        let clicked = built.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble
                || event.button != MouseButton::Left
                || !hitbox.is_hovered(window)
            {
                return;
            }
            let local = event.position - origin;
            let Some(href) = clicked.plan.href([local.x.as_f32(), local.y.as_f32()]) else {
                return;
            };
            if let Some(Installed(on_href)) = cx.try_global::<Installed>() {
                let on_href = *on_href;
                cx.stop_propagation();
                on_href(href, window, cx);
            }
        });
        if let Some(at) = prepainted.hover.get()
            && built.plan.href(at).is_some()
        {
            window.set_cursor_style(CursorStyle::PointingHand, &prepainted.hitbox);
        }

        let theme = Theme::of(cx).clone();
        let tip = prepainted.hover.get().and_then(|at| built.plan.hit(at));
        paint_chart(&built, origin, tip.as_ref(), &theme, window, cx);
        if let (Some(tip), Some(at)) = (tip, prepainted.hover.get()) {
            window.paint_layer(bounds, |window| {
                paint_tip(&built, &tip, at, bounds, &theme, window, cx);
            });
        }
    }
}

fn build(chart: &Chart, key: Key, theme: &Theme, window: &mut Window) -> Option<Built> {
    let text_system = window.text_system().clone();
    let line = TextStyle::Caption.painted_line_height();
    let font_of = |face: &Face| Font {
        family: face.family.clone().unwrap_or(key.font.family.clone()),
        weight: face.weight,
        style: face.style,
        ..key.font.clone()
    };
    let shape = |text: SharedString, face: &Face, color: Hsla| {
        let run = TextRun {
            len: text.len(),
            font: font_of(face),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        text_system.shape_line(text, px(face.size), &[run], None)
    };
    let mut widths: HashMap<(SharedString, FaceKey), f32> = HashMap::new();
    let mut measure = |text: &str, face: &Face| {
        let text = SharedString::from(text.to_string());
        *widths
            .entry((text.clone(), FaceKey::of(face)))
            .or_insert_with(|| shape(text, face, key.colors[1]).width().as_f32())
    };
    let metrics = Metrics {
        size: key.font_size,
        line,
    };
    let plan = plan::plan(chart, key.size, metrics, &mut measure)?;
    // A label's lines, each shaped and set against the others.
    let text = |label: &Label| -> Vec<Op> {
        let color = label.paint.on(theme);
        label
            .text
            .split('\n')
            .enumerate()
            .filter(|(_, content)| !content.is_empty())
            .map(|(index, content)| {
                let shaped = shape(content.to_string().into(), &label.face, color);
                let width = shaped.width().as_f32();
                let height = (shaped.ascent + shaped.descent).as_f32();
                let x = label.at[0] + (label.width - width) * label.align.fraction();
                let y = label.at[1] + index as f32 * label.line + (label.line - height) / 2.0;
                Op::Text {
                    line: Box::new(shaped),
                    at: [x, y],
                }
            })
            .collect()
    };

    let mut painted = Vec::new();
    for series in &plan.series {
        let mut marks = Vec::new();
        let style = &series.style;
        let stroke = style.stroke_paint();
        let curve = series.curve;
        match &series.marks {
            Marks::Lines(runs) => {
                if let Some(paint) = stroke {
                    marks.extend(runs.iter().filter_map(|run| {
                        Some(Op::Path(stroked(&curve.line(&wide(run)), style)?, paint))
                    }));
                }
            }
            Marks::Areas(areas) => {
                for area in areas {
                    if let Some(paint) = style.fill_paint() {
                        let outline = curve.area(&wide(&area.top), &wide(&area.base));
                        let paint = Paint {
                            alpha: paint.alpha * AREA_FILL,
                            ..paint
                        };
                        marks.extend(filled(&outline).map(|path| Op::Path(path, paint)));
                    }
                    if let Some(paint) = stroke {
                        marks.extend(
                            stroked(&curve.line(&wide(&area.top)), style)
                                .map(|path| Op::Path(path, paint)),
                        );
                    }
                }
            }
            Marks::Slices(slices) => {
                for slice in slices {
                    let outline = wedge(slice);
                    if let Some(paint) = style.fill_paint() {
                        marks.extend(filled(&outline).map(|path| Op::Path(path, paint)));
                    }
                    if let Some(paint) = stroke {
                        marks.extend(stroked(&outline, style).map(|path| Op::Path(path, paint)));
                    }
                }
            }
            Marks::Bars(bars) => marks.extend(bars.iter().map(|bar| Op::Quad {
                rect: bar.rect,
                corners: bar.corners,
                fill: style.fill_paint(),
                border: stroke.map(|paint| (paint, style.stroke_width)),
            })),
            Marks::Rules(rules) => {
                if let Some(paint) = stroke {
                    marks.extend(rules.iter().filter_map(|ends| {
                        Some(Op::Path(
                            stroked(&Curve::Linear.line(&wide(ends)), style)?,
                            paint,
                        ))
                    }));
                }
            }
            Marks::Trails(trails) => {
                if let Some(paint) = style.fill_paint() {
                    marks.extend(
                        trails
                            .iter()
                            .filter_map(|run| Some(Op::Path(trail(run)?, paint))),
                    );
                }
            }
            Marks::Points(symbols) => {
                for symbol in symbols {
                    marks.extend(glyph(
                        &symbol.shape,
                        symbol.size,
                        symbol.angle,
                        symbol.at,
                        symbol.style.fill_paint(),
                        symbol
                            .style
                            .stroke_paint()
                            .map(|paint| (paint, symbol.style.stroke_width)),
                        None,
                    ));
                }
            }
            Marks::Texts(texts, set) => {
                let Some(paint) = style.fill_paint() else {
                    continue;
                };
                let color = paint.on(theme);
                let font = Font {
                    family: set.family.clone().unwrap_or(key.font.family.clone()),
                    weight: set.weight,
                    style: set.style,
                    ..key.font.clone()
                };
                let shape_at = |text: SharedString, size: f32| {
                    let run = TextRun {
                        len: text.len(),
                        font: font.clone(),
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    text_system.shape_line(text, px(size), &[run], None)
                };
                let plan::Font {
                    align,
                    baseline,
                    line_height,
                    limit,
                    dir,
                    ..
                } = *set;
                for text in texts {
                    let height = line_height.unwrap_or(text.size + 2.0);
                    // Vega's own baseline offsets, from the first line's.
                    let offset = match baseline {
                        Baseline::Top => 0.79 * text.size,
                        Baseline::Middle => 0.30 * text.size,
                        Baseline::Bottom => -0.21 * text.size,
                        Baseline::LineTop => 0.29 * text.size + 0.5 * height,
                        Baseline::LineBottom => 0.29 * text.size - 0.5 * height,
                        Baseline::Alphabetic => 0.0,
                    }
                    .round();
                    for (index, content) in text.text.split('\n').enumerate() {
                        let content = content.trim();
                        if content.is_empty() {
                            continue;
                        }
                        let content = match limit > 0.0 {
                            true => dir.truncated(content, limit, &set.ellipsis, |t| {
                                shape_at(t.to_string().into(), text.size).width().as_f32()
                            }),
                            false => content.to_string(),
                        };
                        let shaped = shape_at(content.into(), text.size);
                        let width = shaped.width().as_f32();
                        let x = text.at[0]
                            - width
                                * match align {
                                    Align::Left => 0.0,
                                    Align::Center => 0.5,
                                    Align::Right => 1.0,
                                };
                        let y =
                            text.at[1] + offset + index as f32 * height - shaped.ascent.as_f32();
                        marks.push(Op::Text {
                            line: Box::new(shaped),
                            at: [x, y],
                        });
                    }
                }
            }
            Marks::Images(images) => {
                marks.extend(images.iter().cloned().map(Op::Image));
            }
        }
        painted.extend(marks.into_iter().map(|op| (series.clip, op)));
    }
    let (mut under, mut over) = (Vec::new(), Vec::new());
    for axis in &plan.axes {
        let ops = match axis.over {
            true => &mut over,
            false => &mut under,
        };
        for rule in axis.grid.iter().chain(&axis.rules) {
            ops.extend(ruled(rule).map(|op| (None, op)));
        }
        for label in axis.labels.iter().chain(&axis.title) {
            ops.extend(text(label).into_iter().map(|op| (None, op)));
        }
    }
    for legend in &plan.legends {
        let ops = match legend.over {
            true => &mut over,
            false => &mut under,
        };
        if let Some(frame) = &legend.frame {
            ops.push((
                None,
                Op::Quad {
                    rect: frame.rect,
                    corners: [frame.radius; 4],
                    fill: frame.fill,
                    border: frame.stroke.map(|paint| (paint, 1.0)),
                },
            ));
        }
        if let Some(gradient) = &legend.gradient {
            ops.extend(ramp(gradient, theme).into_iter().map(|op| (None, op)));
        }
        for swatch in &legend.swatches {
            let r = swatch.swatch;
            let center = [r.x + r.w / 2.0, r.y + r.h / 2.0];
            let Glyph {
                shape,
                size,
                fill,
                stroke,
                stroke_width,
                dash,
                dash_offset,
            } = &swatch.glyph;
            let border = stroke.map(|paint| (paint, *stroke_width));
            let dash = dash.as_deref().map(|dash| dashed(dash, *dash_offset));
            let glyphs = glyph(shape, *size, 0.0, center, *fill, border, dash.as_deref());
            ops.extend(glyphs.into_iter().map(|op| (swatch.clip, op)));
            ops.extend(text(&swatch.label).into_iter().map(|op| (None, op)));
        }
        for label in legend.title.iter().chain(&legend.labels) {
            ops.extend(text(label).into_iter().map(|op| (None, op)));
        }
    }
    let ops = match plan.title_over {
        true => &mut over,
        false => &mut under,
    };
    for label in plan.title.iter().chain(&plan.subtitle) {
        ops.extend(text(label).into_iter().map(|op| (None, op)));
    }
    Some(Built {
        key,
        plan,
        marks: painted,
        under,
        over,
        line,
    })
}

/// What tells two faces' widths apart.
#[derive(Clone, PartialEq, Eq, Hash)]
struct FaceKey(Option<SharedString>, u32, u32, u8);

impl FaceKey {
    fn of(face: &Face) -> Self {
        let style = match face.style {
            FontStyle::Normal => 0,
            FontStyle::Italic => 1,
            FontStyle::Oblique => 2,
        };
        Self(
            face.family.clone(),
            face.size.to_bits(),
            face.weight.0.to_bits(),
            style,
        )
    }
}

/// A guide's line: a quad where it runs straight across or down solid,
/// else a stroked path.
fn ruled(rule: &plan::Rule) -> Option<Op> {
    let stroke = &rule.stroke;
    let ([x0, y0], [x1, y1]) = (rule.from, rule.to);
    let width = stroke.width;
    if width <= 0.0 {
        return None;
    }
    if stroke.dash.is_none() && (x0 == x1 || y0 == y1) {
        let rect = match x0 == x1 {
            true => Rect {
                x: x0 - width / 2.0,
                y: y0.min(y1),
                w: width,
                h: (y1 - y0).abs(),
            },
            false => Rect {
                x: x0.min(x1),
                y: y0 - width / 2.0,
                w: (x1 - x0).abs(),
                h: width,
            },
        };
        return Some(Op::Quad {
            rect,
            corners: [0.0; 4],
            fill: Some(stroke.paint),
            border: None,
        });
    }
    let style = plan::Style {
        fill: None,
        stroke: Some(stroke.paint.ink),
        opacity: 1.0,
        fill_opacity: 1.0,
        stroke_opacity: stroke.paint.alpha,
        stroke_width: width,
        dash: stroke.dash.clone(),
        dash_offset: stroke.dash_offset,
        cap: stroke.cap,
        join: Join::Miter,
        miter_limit: 10.0,
    };
    let segments = Curve::Linear.line(&wide(&[rule.from, rule.to]));
    Some(Op::Path(stroked(&segments, &style)?, stroke.paint))
}

/// A gradient legend's ramp: each run between stops blended, or each step
/// filled, and its outline.
fn ramp(gradient: &plan::Gradient, theme: &Theme) -> Vec<Op> {
    let r = gradient.rect;
    let mut ops = Vec::new();
    match &gradient.fill {
        Fill::Stops(stops) => {
            for pair in stops.windows(2) {
                let ((t0, a), (t1, b)) = (pair[0], pair[1]);
                let color = |ink: plan::Ink| {
                    let color = ink.on(theme);
                    color.opacity(color.a * gradient.opacity)
                };
                let (rect, angle) = match gradient.vertical {
                    true => (
                        Rect {
                            x: r.x,
                            y: r.bottom() - t1 * r.h,
                            w: r.w,
                            h: (t1 - t0) * r.h,
                        },
                        0.0,
                    ),
                    false => (
                        Rect {
                            x: r.x + t0 * r.w,
                            y: r.y,
                            w: (t1 - t0) * r.w,
                            h: r.h,
                        },
                        90.0,
                    ),
                };
                ops.push(Op::Gradient {
                    rect,
                    angle,
                    colors: [color(a), color(b)],
                });
            }
        }
        Fill::Bands(bands) => ops.extend(bands.iter().map(|(rect, ink)| Op::Quad {
            rect: *rect,
            corners: [0.0; 4],
            fill: Some(Paint {
                ink: *ink,
                alpha: gradient.opacity,
            }),
            border: None,
        })),
    }
    if let Some(stroke) = &gradient.stroke {
        ops.push(Op::Quad {
            rect: r,
            corners: [0.0; 4],
            fill: None,
            border: Some((stroke.paint, stroke.width)),
        });
    }
    ops
}

fn points(run: &[[f32; 2]]) -> Vec<Point<Pixels>> {
    run.iter().map(|&[x, y]| point(px(x), px(y))).collect()
}

fn wide(run: &[[f32; 2]]) -> Vec<[f64; 2]> {
    run.iter().map(|&[x, y]| [x as f64, y as f64]).collect()
}

/// `segments` traced into `builder`.
fn trace(builder: &mut PathBuilder, segments: &[Segment]) {
    let at = |[x, y]: [f64; 2]| point(px(x as f32), px(y as f32));
    for segment in segments {
        match *segment {
            Segment::Move(to) => builder.move_to(at(to)),
            Segment::Line(to) => builder.line_to(at(to)),
            Segment::Cubic(a, b, to) => builder.cubic_bezier_to(at(to), at(a), at(b)),
            Segment::Arc {
                radius,
                large,
                sweep,
                to,
            } => {
                let radius = px(radius as f32);
                builder.arc_to(point(radius, radius), px(0.0), large, sweep, at(to));
            }
            Segment::Close => builder.close(),
        }
    }
}

/// `segments` filled.
fn filled(segments: &[Segment]) -> Option<Path<Pixels>> {
    let mut builder = PathBuilder::fill();
    trace(&mut builder, segments);
    builder.build().ok()
}

/// `segments` stroked as `style` says. A single point draws as a dot.
fn stroked(segments: &[Segment], style: &plan::Style) -> Option<Path<Pixels>> {
    let width = style.stroke_width;
    let options = StrokeOptions::default()
        .with_line_width(width)
        .with_line_cap(match style.cap {
            Cap::Butt => LineCap::Butt,
            Cap::Round => LineCap::Round,
            Cap::Square => LineCap::Square,
        })
        .with_line_join(match style.join {
            Join::Miter => LineJoin::MiterClip,
            Join::Round => LineJoin::Round,
            Join::Bevel => LineJoin::Bevel,
        })
        .with_miter_limit(style.miter_limit.max(1.0));
    let mut builder = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(options));
    if let Some(dash) = &style.dash {
        let dash = dashed(dash, style.dash_offset);
        builder = builder.dash_array(&dash.iter().map(|d| px(*d)).collect::<Vec<_>>());
    }
    match segments {
        [] => return None,
        [Segment::Move(only)] | [Segment::Move(only), Segment::Close] => {
            let [x, y] = only.map(|v| v as f32);
            builder.move_to(point(px(x - width / 2.0), px(y)));
            builder.line_to(point(px(x + width / 2.0), px(y)));
        }
        _ => trace(&mut builder, segments),
    }
    builder.build().ok()
}

/// `dash` begun `offset` into its pattern, as one pattern from its start.
fn dashed(dash: &[f32], offset: f32) -> Vec<f32> {
    let mut dash = dash.to_vec();
    if dash.len() % 2 == 1 {
        dash.extend_from_within(..);
    }
    let period: f32 = dash.iter().sum();
    if offset == 0.0 || period <= 0.0 {
        return dash;
    }
    let mut into = offset.rem_euclid(period);
    let mut index = 0;
    while index < dash.len() - 1 && into >= dash[index] {
        into -= dash[index];
        index += 1;
    }
    // A gap begun part way leads with an empty dash, so dashes stay dashes.
    let mut out = Vec::with_capacity(dash.len() + 3);
    if index % 2 == 1 {
        out.push(0.0);
    }
    out.push(dash[index] - into);
    out.extend(&dash[index + 1..]);
    out.extend(&dash[..index]);
    out.push(into);
    if out.len() % 2 == 1 {
        out.push(0.0);
    }
    out
}

/// A plain line along `run`.
fn polyline(run: &[[f32; 2]], width: f32, dash: Option<&[f32]>) -> Option<Path<Pixels>> {
    let mut builder = PathBuilder::stroke(px(width));
    if let Some(dash) = dash {
        builder = builder.dash_array(&dash.iter().map(|d| px(*d)).collect::<Vec<_>>());
    }
    builder.add_polygon(&points(run), false);
    builder.build().ok()
}

/// A trail as Vega draws it: each pair of points joined by a band whose
/// ends are half circles their points' widths across, filled as one.
fn trail(run: &[([f32; 2], f32)]) -> Option<Path<Pixels>> {
    let mut builder = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::non_zero()));
    let at = |[x, y]: [f32; 2]| point(px(x), px(y));
    // Half a turn about `center` from `from` radians.
    let arc = |builder: &mut PathBuilder, center: [f32; 2], radius: f32, from: f32| {
        for step in 1..=TRAIL_SEGMENTS {
            let angle = from + PI * step as f32 / TRAIL_SEGMENTS as f32;
            builder.line_to(at([
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            ]));
        }
    };
    for pair in run.windows(2) {
        let ((p1, w1), (p2, w2)) = (pair[0], pair[1]);
        let (r1, r2) = (w1 / 2.0, w2 / 2.0);
        let (ux, uy) = (p1[1] - p2[1], p2[0] - p1[0]);
        let length = ux.hypot(uy);
        if length == 0.0 {
            builder.move_to(at([p2[0] + r2, p2[1]]));
            arc(&mut builder, p2, r2, 0.0);
            arc(&mut builder, p2, r2, PI);
            builder.close();
            continue;
        }
        let (ux, uy) = (ux / length, uy / length);
        let t = uy.atan2(ux);
        builder.move_to(at([p1[0] - ux * r1, p1[1] - uy * r1]));
        builder.line_to(at([p2[0] - ux * r2, p2[1] - uy * r2]));
        arc(&mut builder, p2, r2, t - PI);
        builder.line_to(at([p1[0] + ux * r1, p1[1] + uy * r1]));
        arc(&mut builder, p1, r1, t);
        builder.close();
    }
    if run.len() < 2 {
        return None;
    }
    builder.build().ok()
}

/// A point's symbol at `at`: a circle as a rounded quad, any other shape as
/// a path filled and then outlined.
/// A symbol at `at`, its outline dashed by `dash` where it has one.
fn glyph(
    shape: &Shape,
    size: f32,
    angle: f32,
    at: [f32; 2],
    fill: Option<Paint>,
    border: Option<(Paint, f32)>,
    dash: Option<&[f32]>,
) -> Vec<Op> {
    if *shape == Shape::Circle && dash.is_none() {
        let d = size.max(0.0).sqrt();
        return vec![Op::Quad {
            rect: Rect {
                x: at[0] - d / 2.0,
                y: at[1] - d / 2.0,
                w: d,
                h: d,
            },
            corners: [d / 2.0; 4],
            fill,
            border,
        }];
    }
    let outlines: Vec<Vec<[f32; 2]>> = shape
        .outlines(size, angle)
        .into_iter()
        .map(|outline| {
            outline
                .into_iter()
                .map(|[x, y]| [at[0] + x, at[1] + y])
                .collect()
        })
        .collect();
    let mut ops = Vec::new();
    if *shape == Shape::Stroke {
        let paint = border.map(|(paint, _)| paint).or(fill);
        let width = border.map_or(1.0, |(_, width)| width);
        if let Some(paint) = paint {
            for outline in &outlines {
                ops.extend(polyline(outline, width, dash).map(|path| Op::Path(path, paint)));
            }
        }
        return ops;
    }
    if let Some(paint) = fill {
        let mut builder = PathBuilder::fill();
        for outline in &outlines {
            builder.add_polygon(&points(outline), true);
        }
        ops.extend(builder.build().ok().map(|path| Op::Path(path, paint)));
    }
    if let Some((paint, width)) = border {
        for outline in &outlines {
            let mut closed = outline.clone();
            closed.extend(outline.first());
            ops.extend(polyline(&closed, width, dash).map(|path| Op::Path(path, paint)));
        }
    }
    ops
}

/// A slice's outline as d3's arc writes it, at its centre.
fn wedge(slice: &Slice) -> Vec<Segment> {
    let [cx, cy] = slice.center.map(f64::from);
    let moved = |[x, y]: [f64; 2]| [x + cx, y + cy];
    Sector {
        inner: slice.inner.into(),
        outer: slice.radius.into(),
        start: slice.start.into(),
        end: slice.end.into(),
        pad: slice.pad.into(),
        corner: slice.corner.into(),
    }
    .path()
    .into_iter()
    .map(|segment| match segment {
        Segment::Move(to) => Segment::Move(moved(to)),
        Segment::Line(to) => Segment::Line(moved(to)),
        Segment::Cubic(a, b, to) => Segment::Cubic(moved(a), moved(b), moved(to)),
        Segment::Arc {
            radius,
            large,
            sweep,
            to,
        } => Segment::Arc {
            radius,
            large,
            sweep,
            to: moved(to),
        },
        Segment::Close => Segment::Close,
    })
    .collect()
}

/// `path`, built at the chart's origin, moved to `origin`.
fn moved(path: &Path<Pixels>, origin: Point<Pixels>) -> Path<Pixels> {
    let mut path = path.clone();
    path.bounds.origin += origin;
    for vertex in &mut path.vertices {
        vertex.xy_position += origin;
    }
    path
}

fn bounds_of(rect: Rect, origin: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        origin + point(px(rect.x), px(rect.y)),
        size(px(rect.w), px(rect.h)),
    )
}

fn paint_chart(
    built: &Built,
    origin: Point<Pixels>,
    tip: Option<&Tip>,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let plan = &built.plan;
    let paint = |ops: &[(Option<Rect>, Op)], window: &mut Window, cx: &mut App| {
        for (clip, op) in ops {
            let mask = clip.map(|clip| ContentMask {
                bounds: bounds_of(clip, origin),
                ..ContentMask::default()
            });
            window.with_content_mask(mask, |window| op.paint(origin, theme, window, cx));
        }
    };
    paint(&built.under, window, cx);
    if let Some(band) = tip.and_then(|tip| tip.band) {
        window.paint_quad(fill(bounds_of(band, origin), theme.ink(BAND_WASH)));
    }
    paint(&built.marks, window, cx);
    paint(&built.over, window, cx);

    if let Some(tip) = tip {
        if let Some(rule) = tip.rule {
            window.paint_quad(fill(bounds_of(rule, origin), theme.border_strong));
        }
        for &(s, [x, y]) in &tip.points {
            let half = HOVER_POINT / 2.0;
            let dot = Rect {
                x: x - half,
                y: y - half,
                w: HOVER_POINT,
                h: HOVER_POINT,
            };
            window.paint_quad(
                fill(bounds_of(dot, origin), plan.colors[s].ink.on(theme))
                    .corner_radii(Corners::all(px(half)))
                    .border_widths(px(1.5))
                    .border_color(theme.surface),
            );
        }
    }
}

/// The tip's box beside the pointer: its title, then a swatch, series name
/// and value per row. Kept inside the chart where it fits.
fn paint_tip(
    built: &Built,
    tip: &Tip,
    at: [f32; 2],
    bounds: Bounds<Pixels>,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) {
    let font = built.key.font.clone();
    let font_size = px(built.key.font_size);
    let line = built.line;
    let shape = |text: SharedString, color: Hsla, window: &mut Window| {
        let run = TextRun {
            len: text.len(),
            font: font.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .shape_line(text, font_size, &[run], None)
    };
    let title = shape(tip.title.clone(), theme.text, window);
    let rows: Vec<(usize, ShapedLine)> = tip
        .rows
        .iter()
        .map(|(s, value)| {
            let text = match &built.plan.colors[*s].name {
                Some(name) if *name != tip.title => format!("{name}  {value}"),
                _ => value.to_string(),
            };
            (*s, shape(text.into(), theme.text_muted, window))
        })
        .collect();

    let swatch = line * 0.6;
    let width = rows
        .iter()
        .map(|(_, row)| swatch + TIP_PAD / 2.0 + row.width().as_f32())
        .fold(title.width().as_f32(), f32::max)
        + 2.0 * TIP_PAD;
    let height = line * (1 + rows.len()) as f32 + 2.0 * TIP_PAD;
    let [w, h] = [bounds.size.width.as_f32(), bounds.size.height.as_f32()];
    let mut x = at[0] + TIP_OFFSET;
    if x + width > w {
        x = (at[0] - TIP_OFFSET - width).max(0.0);
    }
    let y = (at[1] + TIP_OFFSET).min(h - height).max(0.0);
    let origin = bounds.origin;
    let card = Rect {
        x,
        y,
        w: width,
        h: height,
    };
    window.paint_quad(
        fill(bounds_of(card, origin), theme.surface_dialog)
            .corner_radii(Corners::all(px(TIP_RADIUS)))
            .border_widths(px(1.0))
            .border_color(theme.border),
    );
    let mut row_at = origin + point(px(x + TIP_PAD), px(y + TIP_PAD));
    title
        .paint(row_at, px(line), TextAlign::Left, None, window, cx)
        .ok();
    for (s, row) in rows {
        row_at.y += px(line);
        let dot = Rect {
            x: x + TIP_PAD,
            y: (row_at.y - origin.y).as_f32() + (line - swatch) / 2.0,
            w: swatch,
            h: swatch,
        };
        window.paint_quad(
            fill(bounds_of(dot, origin), built.plan.colors[s].ink.on(theme))
                .corner_radii(Corners::all(px(2.0))),
        );
        let text_at = row_at + point(px(swatch + TIP_PAD / 2.0), px(0.0));
        row.paint(text_at, px(line), TextAlign::Left, None, window, cx)
            .ok();
    }
}
