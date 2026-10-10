//! A [`Chart`] drawn as one element: laid out once per data generation, size
//! and text style, then painted every frame from what was kept.

use std::{cell::Cell, collections::HashMap, f32::consts::FRAC_PI_2, rc::Rc};

use gpui::{
    App, Bounds, ContentMask, Corners, DispatchPhase, Element, ElementId, Font, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, MouseMoveEvent, Path,
    PathBuilder, Pixels, Point, Refineable, ShapedLine, SharedString, Style, StyleRefinement,
    Styled, TextAlign, TextRun, Window, fill, point, px, relative, size,
};
use theme::{TextStyle, Theme};

use crate::{
    model::{Chart, Encoding, Mark},
    plan::{self, Marks, Plan, Rect, Side, Slice, Tip},
};

/// The height a chart takes when nothing sizes it.
const HEIGHT: f32 = 240.0;
const LINE_WIDTH: f32 = 2.0;
const POINT: f32 = 6.0;
const HOVER_POINT: f32 = 8.0;
const AREA_FILL: f32 = 0.25;
const BAR_RADIUS: f32 = 2.0;
/// How many straight segments draw a full turn of a slice's rim.
const TURN_SEGMENTS: f32 = 128.0;
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

/// The colour of series `index`: the theme's accent, then its status colours,
/// then hues between them.
pub fn series_color(theme: &Theme, index: usize) -> Hsla {
    let hue = |color: Hsla, degrees: f32| Hsla {
        h: degrees / 360.0,
        ..color
    };
    let palette = [
        theme.accent,
        theme.success,
        theme.warning,
        theme.danger,
        hue(theme.success, 185.0),
        hue(theme.accent, 270.0),
        hue(theme.danger, 330.0),
        hue(theme.warning, 50.0),
    ];
    palette[index % palette.len()]
}

/// What a plan was made from. Equal keys make equal plans.
#[derive(Clone, PartialEq)]
struct Key {
    generation: u64,
    mark: Mark,
    encoding: Encoding,
    title: Option<SharedString>,
    size: [f32; 2],
    font: Font,
    font_size: f32,
    colors: [Hsla; 2],
}

/// A plan and everything painting it needs that does not change with it:
/// tessellated paths at the chart's own origin, and shaped labels.
struct Built {
    key: Key,
    plan: Plan,
    /// Series, path, and whether it is a fill.
    paths: Vec<(usize, Path<Pixels>, bool)>,
    labels: HashMap<SharedString, ShapedLine>,
    title: Option<ShapedLine>,
    line: f32,
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
            generation: self.chart.data.generation(),
            mark: self.chart.mark,
            encoding: self.chart.encoding.clone(),
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
                state.built = build(&self.chart, key, window).map(Rc::new);
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

fn build(chart: &Chart, key: Key, window: &mut Window) -> Option<Built> {
    let text_system = window.text_system().clone();
    let font_size = px(key.font_size);
    let line = TextStyle::Caption.painted_line_height();
    let shape = |text: SharedString, color: Hsla| {
        let run = TextRun {
            len: text.len(),
            font: key.font.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        text_system.shape_line(text, font_size, &[run], None)
    };
    let mut labels = HashMap::new();
    let mut measure = |text: &str| {
        let text = SharedString::from(text.to_string());
        labels
            .entry(text.clone())
            .or_insert_with(|| shape(text, key.colors[1]))
            .width()
            .as_f32()
    };
    let plan = plan::plan(chart, key.size, line, &mut measure)?;
    let title = plan
        .title
        .as_ref()
        .map(|title| shape(title.text.clone(), key.colors[0]));

    let mut paths = Vec::new();
    for (s, series) in plan.series.iter().enumerate() {
        match &series.marks {
            Marks::Lines(runs) => {
                paths.extend(runs.iter().filter_map(|run| Some((s, stroke(run)?, false))));
            }
            Marks::Areas(outlines) => {
                for outline in outlines {
                    let mut builder = PathBuilder::fill();
                    builder.add_polygon(&points(outline), true);
                    paths.extend(builder.build().ok().map(|path| (s, path, true)));
                    paths
                        .extend(stroke(&outline[..outline.len() - 2]).map(|path| (s, path, false)));
                }
            }
            Marks::Slices(slices) => {
                paths.extend(
                    slices
                        .iter()
                        .filter_map(|slice| Some((s, wedge(slice)?, true))),
                );
            }
            Marks::Bars(_) | Marks::Points(_) => {}
        }
    }
    Some(Built {
        key,
        plan,
        paths,
        labels,
        title,
        line,
    })
}

fn points(run: &[[f32; 2]]) -> Vec<Point<Pixels>> {
    run.iter().map(|&[x, y]| point(px(x), px(y))).collect()
}

fn stroke(run: &[[f32; 2]]) -> Option<Path<Pixels>> {
    let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
    match run {
        [] => return None,
        [only] => {
            builder.move_to(point(px(only[0] - LINE_WIDTH / 2.0), px(only[1])));
            builder.line_to(point(px(only[0] + LINE_WIDTH / 2.0), px(only[1])));
        }
        _ => builder.add_polygon(&points(run), false),
    }
    builder.build().ok()
}

fn wedge(slice: &Slice) -> Option<Path<Pixels>> {
    let [cx, cy] = slice.center;
    let span = slice.end - slice.start;
    let steps = (span / std::f32::consts::TAU * TURN_SEGMENTS)
        .ceil()
        .max(1.0) as usize;
    let mut outline = vec![point(px(cx), px(cy))];
    outline.extend((0..=steps).map(|step| {
        // Clockwise from twelve o'clock.
        let angle = slice.start + span * step as f32 / steps as f32 - FRAC_PI_2;
        point(
            px(cx + slice.radius * angle.cos()),
            px(cy + slice.radius * angle.sin()),
        )
    }));
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&outline, true);
    builder.build().ok()
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

fn paint_label(
    built: &Built,
    label: &plan::Label,
    origin: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(shaped) = built.labels.get(&label.text) {
        let at = origin + point(px(label.at[0]), px(label.at[1]));
        shaped
            .paint(at, px(built.line), TextAlign::Left, None, window, cx)
            .ok();
    }
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

    for axis in &plan.axes {
        if axis.grid {
            for tick in &axis.ticks {
                let line = match axis.side {
                    Side::Left => Rect {
                        x: plan.plot.x,
                        y: tick.at.round(),
                        w: plan.plot.w,
                        h: 1.0,
                    },
                    Side::Bottom => Rect {
                        x: tick.at.round(),
                        y: plan.plot.y,
                        w: 1.0,
                        h: plan.plot.h,
                    },
                };
                window.paint_quad(fill(bounds_of(line, origin), theme.border_faint));
            }
        }
        let labels = axis.ticks.iter().filter_map(|tick| tick.label.as_ref());
        for label in labels.chain(&axis.title) {
            paint_label(built, label, origin, window, cx);
        }
    }
    if !plan.axes.is_empty() {
        let base = Rect {
            x: plan.plot.x,
            y: plan.plot.bottom().round(),
            w: plan.plot.w,
            h: 1.0,
        };
        window.paint_quad(fill(bounds_of(base, origin), theme.border));
    }
    if let Some(band) = tip.and_then(|tip| tip.band) {
        window.paint_quad(fill(bounds_of(band, origin), theme.ink(BAND_WASH)));
    }

    // A fixed domain can leave marks past the plot; they stop at its edge,
    // short of half a point.
    let clip = Rect {
        x: plan.plot.x - POINT / 2.0,
        y: plan.plot.y - POINT / 2.0,
        w: plan.plot.w + POINT,
        h: plan.plot.h + POINT,
    };
    let mask = ContentMask {
        bounds: bounds_of(clip, origin),
        ..ContentMask::default()
    };
    window.with_content_mask(Some(mask), |window| {
        let mut paths = built.paths.iter().peekable();
        for (s, series) in plan.series.iter().enumerate() {
            let color = series_color(theme, s);
            while let Some((_, path, filled)) = paths.next_if(|(of, ..)| *of == s) {
                let color = if *filled && plan.chart().mark == Mark::Area {
                    color.opacity(AREA_FILL)
                } else {
                    color
                };
                window.paint_path(moved(path, origin), color);
            }
            match &series.marks {
                Marks::Bars(bars) => {
                    for bar in bars {
                        window.paint_quad(
                            fill(bounds_of(*bar, origin), color)
                                .corner_radii(Corners::all(px(BAR_RADIUS))),
                        );
                    }
                }
                Marks::Points(at) => {
                    for &[x, y] in at {
                        let dot = Rect {
                            x: x - POINT / 2.0,
                            y: y - POINT / 2.0,
                            w: POINT,
                            h: POINT,
                        };
                        window.paint_quad(
                            fill(bounds_of(dot, origin), color)
                                .corner_radii(Corners::all(px(POINT / 2.0))),
                        );
                    }
                }
                _ => {}
            }
        }
    });

    if let Some(tip) = tip
        && let Some(&(_, [x, _])) = tip.points.first()
    {
        let rule = Rect {
            x: x.round(),
            y: plan.plot.y,
            w: 1.0,
            h: plan.plot.h,
        };
        window.paint_quad(fill(bounds_of(rule, origin), theme.border_strong));
        for &(s, [x, y]) in &tip.points {
            let half = HOVER_POINT / 2.0;
            let dot = Rect {
                x: x - half,
                y: y - half,
                w: HOVER_POINT,
                h: HOVER_POINT,
            };
            window.paint_quad(
                fill(bounds_of(dot, origin), series_color(theme, s))
                    .corner_radii(Corners::all(px(half)))
                    .border_widths(px(1.5))
                    .border_color(theme.surface),
            );
        }
    }

    if let (Some(label), Some(shaped)) = (&plan.title, &built.title) {
        let at = origin + point(px(label.at[0]), px(label.at[1]));
        shaped
            .paint(at, px(built.line), TextAlign::Left, None, window, cx)
            .ok();
    }
    if let Some(label) = &plan.legend_title {
        paint_label(built, label, origin, window, cx);
    }
    for swatch in &plan.legend {
        window.paint_quad(
            fill(
                bounds_of(swatch.swatch, origin),
                series_color(theme, swatch.series),
            )
            .corner_radii(Corners::all(px(2.0))),
        );
        paint_label(built, &swatch.label, origin, window, cx);
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
            let text = match &built.plan.series[*s].name {
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
            fill(bounds_of(dot, origin), series_color(theme, s))
                .corner_radii(Corners::all(px(2.0))),
        );
        let text_at = row_at + point(px(swatch + TIP_PAD / 2.0), px(0.0));
        row.paint(text_at, px(line), TextAlign::Left, None, window, cx)
            .ok();
    }
}
