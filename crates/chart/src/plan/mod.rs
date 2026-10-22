//! A chart laid out at a size: every mark, axis and label in pixels from the
//! chart's top-left corner, with no window involved.
//!
//! Bars and areas stack by their value channel's [`Stack`], as Vega's Stack
//! does, unless `x2` or `y2` ranges them; other marks stack only where it
//! asks. A stack is the rows at one place along the shared axis, a band and
//! an offset for a discrete one, in their series' order down a vertical
//! stack and along a horizontal one, or by `order`; an area's or line's
//! series stands at zero where it has no row. Lines, areas and trails join
//! their series' rows in ascending x, or by `order`, through their
//! interpolation's curve, and a missing y breaks them. A layer's
//! [`Invalid`] mode says what its rows with an invalid value draw and which
//! domains take them in; null is a value of its own on a discrete scale.
//! A bar along a
//! continuous axis without `x2` or `y2` is Vega-Lite's continuous band size
//! across, and a bar's, area's, line's or trail's `orient` says which of two
//! continuous axes, or of one alone, it stands along. A bar, rule, rect,
//! text, tick, point or image layer may leave out x or y: a bar, rule or
//! rect then spans the plot that way, and the others stand at its middle. A
//! chart with x alone is laid out along it, as is one with y alone, and x
//! and y may both be discrete.
//! A position in pixels places every row of its layer there, and a rect
//! placed in pixels without its far end reaches the base of the axis: zero,
//! or the axis's start. `xOffset` and `yOffset` values move a mark, and
//! `x2Offset` and `y2Offset` its far end where it has an `x2` or `y2`.
//! Arcs, and text with a `theta`, over axes stack one pie per place their x
//! and y give, at the plot's middle without them, half the plot's shorter
//! side across without a `radius`. A layer's rows split into series by colour, `detail` and `key`, and for a
//! line, area or bar by every other channel that names a field but shape and
//! angle. A channel's [`Scale`] and [`Sort`] shape its domain and range as
//! Vega-Lite's defaults and Vega's scales do. Its axis or legend, and the
//! chart's title, are laid out as Vega lays them out, inside the size given:
//! the title and legends take their room from the edges, and the plot what
//! its axes leave of the rest. The pointer reads out the
//! band under it, and the nearest row along the shared axis of each series
//! of a line, area, trail or point layer placed on scales both ways. A bar
//! spanning y to `y2` over a continuous x runs across, and a tick with an
//! `x2` or `y2` runs to it.
//!
//! [`Invalid`]: crate::model::Invalid
//! [`Scale`]: crate::model::Scale
//! [`Sort`]: crate::model::Sort
//! [`Stack`]: crate::model::Stack

use crate::{
    data::Column,
    model::{
        Align, Baseline, Cap, Channel, Chart, Dir, Encode, Encoding, Join, Mark, ScaleType, Shape,
        Side,
    },
    plan::{domain::Colors, layout::Layout, title::Heading},
};
use d3::shape::curve::Curve;
use gpui::{FontStyle, FontWeight, Hsla, SharedString};
use std::{f32::consts::TAU, sync::Arc};
use theme::Theme;

mod arcs;
mod axis;
mod cartesian;
mod domain;
mod invalid;
mod layout;
mod legend;
mod look;
mod scales;
mod title;

/// The least a plot takes either way, at any size.
const MIN_PLOT: f32 = 16.0;
/// How far along the shared axis from the hovered point another series'
/// point may be and still be read out with it.
const SAME_X: f32 = 1.0;
/// How far past a thin mark a click still lands on it.
const SLOP: f32 = 3.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn contains(&self, [x, y]: [f32; 2]) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// The least rect holding both.
    pub fn union(self, other: Rect) -> Rect {
        let (x, y) = (self.x.min(other.x), self.y.min(other.y));
        Rect {
            x,
            y,
            w: self.right().max(other.right()) - x,
            h: self.bottom().max(other.bottom()) - y,
        }
    }

    /// Moved by `by`.
    pub fn moved(self, by: [f32; 2]) -> Rect {
        Rect {
            x: self.x + by[0],
            y: self.y + by[1],
            ..self
        }
    }
}

/// The host's text: its font size and line height, in pixels. A guide's
/// labels are set at its size, which stands for Vega's 10 pixels; titles
/// are as much larger as Vega's are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub size: f32,
    pub line: f32,
}

/// How a guide's text is set. `family` `None` is the view's own.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub family: Option<SharedString>,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
}

/// Level text, its lines split at `\n` and `line` apart, the first line's
/// top-left at `at`; `width` its widest line as measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: SharedString,
    pub at: [f32; 2],
    pub width: f32,
    pub line: f32,
    pub face: Face,
    pub paint: Paint,
    /// How its lines stand against each other.
    pub align: Align,
}

impl Label {
    pub fn height(&self) -> f32 {
        self.line * self.text.split('\n').count() as f32
    }

    pub fn rect(&self) -> Rect {
        Rect {
            x: self.at[0],
            y: self.at[1],
            w: self.width,
            h: self.height(),
        }
    }

    /// Moved so `at` is the point of it `align` and `baseline` name.
    pub fn anchored(self, at: [f32; 2], align: Align, baseline: Baseline) -> Self {
        let height = self.height();
        let y = match baseline {
            Baseline::Top | Baseline::LineTop => 0.0,
            Baseline::Middle => height / 2.0,
            Baseline::Bottom | Baseline::Alphabetic | Baseline::LineBottom => height,
        };
        Self {
            at: [at[0] - self.width * align.fraction(), at[1] - y],
            align,
            ..self
        }
    }

    /// Moved by `by`.
    pub fn moved(self, by: [f32; 2]) -> Self {
        Self {
            at: [self.at[0] + by[0], self.at[1] + by[1]],
            ..self
        }
    }
}

/// A guide's line from `from` to `to`.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub stroke: Stroke,
}

/// How a guide's line is stroked.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub paint: Paint,
    pub width: f32,
    /// Lengths of dash and gap, in turn. `None` is solid.
    pub dash: Option<Arc<[f32]>>,
    pub dash_offset: f32,
    pub cap: Cap,
}

/// An x or y axis on `side` of the plot.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub side: Side,
    /// Lines across the plot at its ticks.
    pub grid: Vec<Rule>,
    /// Its domain line, then its ticks.
    pub rules: Vec<Rule>,
    pub labels: Vec<Label>,
    pub title: Option<Label>,
    /// Whether it is drawn over the marks rather than under them.
    pub over: bool,
}

impl Axis {
    /// What its rules and text cover, grid aside.
    pub fn bounds(&self) -> Option<Rect> {
        let rules = self.rules.iter().map(|rule| {
            let half = rule.stroke.width / 2.0;
            let (x0, x1) = (rule.from[0].min(rule.to[0]), rule.from[0].max(rule.to[0]));
            let (y0, y1) = (rule.from[1].min(rule.to[1]), rule.from[1].max(rule.to[1]));
            Rect {
                x: x0 - half,
                y: y0 - half,
                w: x1 - x0 + 2.0 * half,
                h: y1 - y0 + 2.0 * half,
            }
        });
        let texts = self.labels.iter().chain(&self.title).map(Label::rect);
        rules.chain(texts).reduce(|a, b| a.union(b))
    }
}

/// A colour: a place in the theme's categorical colours, or a fixed one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Category(usize),
    Fixed(Hsla),
    /// The theme's muted text, for a legend of something other than colour.
    Muted,
    /// The theme's text, for a rule or text with no colour of its own.
    Text,
    /// The theme's border, for an axis's domain and ticks.
    Border,
    /// The theme's faint border, for grid lines.
    Faint,
}

impl Ink {
    pub fn on(self, theme: &Theme) -> Hsla {
        match self {
            Ink::Category(index) => theme.categorical(index),
            Ink::Fixed(color) => color,
            Ink::Muted => theme.text_muted,
            Ink::Text => theme.text,
            Ink::Border => theme.border,
            Ink::Faint => theme.border_faint,
        }
    }
}

/// How a mark is painted. Its opacities multiply its colours' own.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub fill: Option<Ink>,
    pub stroke: Option<Ink>,
    pub opacity: f32,
    pub fill_opacity: f32,
    pub stroke_opacity: f32,
    pub stroke_width: f32,
    /// Lengths of dash and gap, in turn. `None` is solid.
    pub dash: Option<Arc<[f32]>>,
    /// How far into the dash pattern a stroke starts.
    pub dash_offset: f32,
    pub cap: Cap,
    pub join: Join,
    pub miter_limit: f32,
}

impl Style {
    pub fn fill_paint(&self) -> Option<Paint> {
        Some(Paint {
            ink: self.fill?,
            alpha: self.opacity * self.fill_opacity,
        })
    }

    pub fn stroke_paint(&self) -> Option<Paint> {
        Some(Paint {
            ink: self.stroke?,
            alpha: self.opacity * self.stroke_opacity,
        })
    }
}

/// An ink at an opacity, which multiplies the ink's own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub ink: Ink,
    pub alpha: f32,
}

impl Paint {
    pub fn on(self, theme: &Theme) -> Hsla {
        let color = self.ink.on(theme);
        color.opacity(color.a * self.alpha)
    }
}

/// A slice as d3's arc draws it. Angles in radians, clockwise from twelve
/// o'clock. `inner` is 0 for a pie.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slice {
    pub center: [f32; 2],
    pub radius: f32,
    pub inner: f32,
    pub start: f32,
    pub end: f32,
    /// The angle left between it and its neighbours, half from each side.
    pub pad: f32,
    /// Its corners' radius, in pixels.
    pub corner: f32,
}

impl Slice {
    /// Whether `point` is inside it, padding taken out and corners left in.
    pub fn contains(&self, [x, y]: [f32; 2]) -> bool {
        let (dx, dy) = (x - self.center[0], y - self.center[1]);
        let distance = dx.hypot(dy);
        // d3 pads by the width the pad angle spans at its pad radius.
        let pad = match self.pad > 0.0 {
            true => {
                let reach = self.inner.hypot(self.radius) * (self.pad / 2.0).sin();
                (reach / distance).min(1.0).asin()
            }
            false => 0.0,
        };
        let past = (dx.atan2(-dy) - self.start - pad).rem_euclid(TAU);
        distance >= self.inner
            && distance <= self.radius
            && past < self.end - self.start - 2.0 * pad
    }
}

/// A point: `size` is its area in square pixels, `angle` clockwise degrees.
#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    pub at: [f32; 2],
    pub shape: Shape,
    pub size: f32,
    pub angle: f32,
    pub style: Style,
}

/// A run along the top, and the run beneath it in the same direction,
/// which the outline comes back along.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub top: Vec<[f32; 2]>,
    pub base: Vec<[f32; 2]>,
}

impl Area {
    /// The top, then the base back.
    pub fn outline(&self) -> Vec<[f32; 2]> {
        self.top
            .iter()
            .chain(self.base.iter().rev())
            .copied()
            .collect()
    }
}

/// A bar, rect or tick: its corners' radii top-left, top-right,
/// bottom-right, bottom-left, each at most half its shorter side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub rect: Rect,
    pub corners: [f32; 4],
}

/// A text mark's text, its anchor `at` and font size.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub at: [f32; 2],
    pub text: SharedString,
    pub size: f32,
}

/// How a layer's texts are set: `align` and `baseline` say which edge of
/// each its anchor is.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// `None` for the view's own.
    pub family: Option<SharedString>,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub align: Align,
    pub baseline: Baseline,
    /// Between lines, in pixels. `None` for the font's own.
    pub line_height: Option<f32>,
    /// The widest a text is drawn, past which it ends in `ellipsis`. `0`
    /// for no limit.
    pub limit: f32,
    pub ellipsis: SharedString,
    /// Which end of a text cut short keeps its characters.
    pub dir: Dir,
}

/// An image at its anchor `at`, `align` and `baseline` saying which edge of
/// it the anchor is. A missing width or height is the image's own, or kept
/// in its aspect ratio to the other.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub at: [f32; 2],
    pub url: SharedString,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub align: Align,
    pub baseline: Baseline,
    /// Whether it keeps its aspect ratio inside its width and height.
    pub aspect: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Marks {
    /// Bars, rects and ticks.
    Bars(Vec<Bar>),
    /// Each a run of points with no gap in it.
    Lines(Vec<Vec<[f32; 2]>>),
    Areas(Vec<Area>),
    /// At most one to a pixel, each styled on its own.
    Points(Vec<Symbol>),
    Slices(Vec<Slice>),
    /// Each from one end to the other.
    Rules(Vec<[[f32; 2]; 2]>),
    Texts(Vec<Text>, Font),
    /// Each a run of points with no gap in it, and its width at each.
    Trails(Vec<Vec<([f32; 2], f32)>>),
    Images(Vec<Image>),
}

/// One series' marks in one layer, styled as its first row is. `name` is its
/// value of the colour field.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub name: Option<SharedString>,
    /// Its place in [`Plan::colors`].
    pub color: usize,
    pub mark: Mark,
    pub marks: Marks,
    pub style: Style,
    /// What a line, area or trail runs through its points along.
    pub curve: Curve,
    /// Where its marks are cut off: the plot, for a layer with `clip`.
    /// `None` draws them whole.
    pub clip: Option<Rect>,
}

/// One of the chart's colours. `name` is its value of the colour field,
/// `None` for a layer's own colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Color {
    pub name: Option<SharedString>,
    pub ink: Ink,
}

/// A legend: its box, title, and a symbol legend's entries or a gradient
/// legend's ramp and labels.
#[derive(Clone, Debug, PartialEq)]
pub struct Legend {
    pub frame: Option<Frame>,
    pub title: Option<Label>,
    pub swatches: Vec<Swatch>,
    pub gradient: Option<Gradient>,
    /// A gradient legend's labels.
    pub labels: Vec<Label>,
    /// Whether it is drawn over the marks rather than under them.
    pub over: bool,
}

/// A box drawn behind a legend.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub rect: Rect,
    pub fill: Option<Paint>,
    pub stroke: Option<Paint>,
    pub radius: f32,
}

/// One legend entry: `glyph` centred in `swatch`, then its label. `clip`
/// cuts the glyph off.
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    pub glyph: Glyph,
    pub swatch: Rect,
    pub label: Label,
    pub clip: Option<Rect>,
}

/// A legend entry's symbol: `size` square pixels of `shape`, the stroke
/// `shape` a line across.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub shape: Shape,
    pub size: f32,
    pub fill: Option<Paint>,
    pub stroke: Option<Paint>,
    pub stroke_width: f32,
    /// Lengths of dash and gap along its outline, in turn, from
    /// `dash_offset` into the pattern.
    pub dash: Option<Arc<[f32]>>,
    pub dash_offset: f32,
}

/// A gradient legend's ramp in `rect`, its colours from the start of its
/// run to the end: up a vertical one, rightward a horizontal one.
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub rect: Rect,
    pub vertical: bool,
    pub fill: Fill,
    pub stroke: Option<Stroke>,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    /// Colours at places from 0 to 1 along the run, blended between.
    Stops(Vec<(f32, Ink)>),
    /// A discretizing scale's steps, each its own cell.
    Bands(Vec<(Rect, Ink)>),
}

/// What is under the pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub title: SharedString,
    /// Colour and the value read out in it.
    pub rows: Vec<(usize, SharedString)>,
    /// Colour and the point read out.
    pub points: Vec<(usize, [f32; 2])>,
    /// The band read out.
    pub band: Option<Rect>,
    /// A line one pixel wide across the plot through the points read out.
    pub rule: Option<Rect>,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub size: [f32; 2],
    /// What the marks are drawn in. For arcs, the square the pie fills.
    pub plot: Rect,
    pub title: Option<Label>,
    pub subtitle: Option<Label>,
    /// Whether the title and subtitle are drawn over the marks.
    pub title_over: bool,
    pub legends: Vec<Legend>,
    pub axes: Vec<Axis>,
    /// Paint order.
    pub series: Vec<Series>,
    pub colors: Vec<Color>,
    chart: Chart,
    hover: Hover,
    links: Vec<Link>,
}

/// Where a mark with an `href` takes a click.
#[derive(Clone, Debug)]
struct Link {
    hit: Hit,
    href: SharedString,
}

#[derive(Clone, Debug)]
enum Hit {
    Rect(Rect),
    Disc([f32; 2], f32),
    Slice(Slice),
    /// A run of points, and how far from it a click lands.
    Line(Vec<[f32; 2]>, f32),
    Polygon(Vec<[f32; 2]>),
}

impl Hit {
    fn contains(&self, point: [f32; 2]) -> bool {
        match self {
            Hit::Rect(rect) => rect.contains(point),
            Hit::Disc(center, radius) => {
                (point[0] - center[0]).hypot(point[1] - center[1]) <= *radius
            }
            Hit::Slice(slice) => slice.contains(point),
            Hit::Line(run, reach) => run.windows(2).any(|pair| {
                let ([ax, ay], [bx, by]) = (pair[0], pair[1]);
                let (dx, dy) = (bx - ax, by - ay);
                let length = dx * dx + dy * dy;
                let t = match length > 0.0 {
                    true => {
                        (((point[0] - ax) * dx + (point[1] - ay) * dy) / length).clamp(0.0, 1.0)
                    }
                    false => 0.0,
                };
                (point[0] - ax - t * dx).hypot(point[1] - ay - t * dy) <= *reach
            }),
            Hit::Polygon(outline) => {
                let mut inside = false;
                let mut j = outline.len().wrapping_sub(1);
                for i in 0..outline.len() {
                    let ([xi, yi], [xj, yj]) = (outline[i], outline[j]);
                    if (yi > point[1]) != (yj > point[1])
                        && point[0] < (xj - xi) * (point[1] - yi) / (yj - yi) + xi
                    {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Hover {
    bands: Option<Bands>,
    traces: Vec<Trace>,
    slices: Vec<(usize, Slice, SharedString)>,
}

/// Bars' values by band.
#[derive(Clone, Debug)]
struct Bands {
    /// Each band's step along the axis: where it starts and how long it is.
    cells: Vec<(f32, f32)>,
    vertical: bool,
    across: (f32, f32),
    labels: Vec<SharedString>,
    /// Colour, and its value read out in each band.
    values: Vec<(usize, Vec<Option<SharedString>>)>,
}

/// One series of a line, area, trail or point layer, ascending along the
/// shared axis: each point and its row.
#[derive(Clone, Debug)]
struct Trace {
    layer: usize,
    color: usize,
    /// The shared axis: 0 for x, 1 for y.
    axis: usize,
    points: Vec<([f32; 2], u32)>,
}

type Body = (Rect, Vec<Axis>, Vec<Series>, Hover, Vec<Link>);

/// `chart` laid out at `size`, its text set to `metrics` and as wide as
/// `measure` says. `None` for what cannot be drawn: a field the data does
/// not hold, a quantitative field over text, a range of what its channel
/// does not take, layers that disagree on which axis is discrete, no layer
/// placed on a scale where a layer is placed in pixels, a text at an angle
/// or with an outline, a guide's text at an angle, a title beside the
/// chart, or a mark without the positions it needs. A size too small to
/// hold a plot draws a cramped one, so no size changes whether a chart
/// draws.
pub fn plan(
    chart: &Chart,
    size: [f32; 2],
    metrics: Metrics,
    measure: &mut dyn FnMut(&str, &Face) -> f32,
) -> Option<Plan> {
    let chart = &*invalid::prepare(chart);
    let colors = Colors::of(chart)?;
    // Text is drawn level, and filled without an outline: gpui paints a
    // glyph as a mask in one colour.
    let angled = |angle: Option<f32>| angle.is_some_and(|a| a.rem_euclid(360.0) != 0.0);
    if chart.layers.iter().any(|layer| {
        let e = &layer.encoding;
        let outlined = match (&e.stroke, &e.stroke_width) {
            (_, Some(Encode::Value(width))) if *width <= 0.0 => false,
            (Some(Encode::Value(color)), _) => color.a > 0.0,
            (Some(Encode::Scaled(_)), _) => true,
            (None, _) => layer.properties.filled == Some(false),
        };
        let guides = e.channels().into_iter().any(|channel| {
            let axis = channel.axis.as_deref().is_some_and(|axis| {
                angled(axis.style.label.angle)
                    || angled(axis.title.angle)
                    || axis.cases.iter().any(|case| angled(case.style.label.angle))
            });
            let legend = channel
                .legend
                .as_deref()
                .is_some_and(|legend| angled(legend.label.angle) || angled(legend.title.angle));
            axis || legend
        });
        guides
            || (layer.mark == Mark::Text
                && (outlined || !matches!(e.angle, None | Some(Encode::Value(0.0)))))
    }) {
        return None;
    }
    if let Some(title) = &chart.title {
        let beside = matches!(title.orient, Some(Side::Left | Side::Right))
            && title.lettering.angle.is_none();
        if beside || angled(title.lettering.angle) || angled(title.subtitle_lettering.angle) {
            return None;
        }
    }

    let mut layout = Layout::new(size, metrics, measure);
    let heading = chart
        .title
        .as_ref()
        .map(|title| Heading::of(title, &mut layout));
    if let Some(heading) = &heading {
        layout.reserve(heading.side(), heading.extent());
    }
    let inner = layout.inner;
    let blocks = legend::legends(chart, &colors, &mut layout, [inner.w, inner.h])?;
    legend::reserve(&blocks, &mut layout);

    // Arcs and text alone, none placed on a scale, draw without axes.
    let polar = chart.layers.iter().any(|layer| layer.mark == Mark::Arc)
        && chart.layers.iter().all(|layer| {
            let e = &layer.encoding;
            matches!(layer.mark, Mark::Arc | Mark::Text)
                && [&e.x, &e.y]
                    .iter()
                    .all(|p| p.as_ref().and_then(Encode::channel).is_none())
        });
    let first_title = |channel: fn(&Encoding) -> Option<&Channel>| {
        chart.layers.iter().find_map(|layer| {
            let channel = channel(&layer.encoding)?;
            // A position with no scale has no axis.
            (channel.scale.kind != Some(ScaleType::Identity))
                .then(|| channel.title.clone())
                .flatten()
        })
    };
    let titles = [
        first_title(|e| e.x.as_ref()?.channel()),
        first_title(|e| e.y.as_ref()?.channel()),
    ];
    let (plot, axes, series, hover, links) = match (polar, chart.layers.as_slice()) {
        (true, _) => arcs::arcs(chart, &colors, layout.inner)?,
        (false, []) => return None,
        (false, _) => cartesian::cartesian(chart, &colors, &mut layout, titles)?,
    };
    let bounds = axes.iter().filter_map(Axis::bounds).fold(plot, Rect::union);
    let blocks = legend::legends(chart, &colors, &mut layout, [plot.w, plot.h])?;
    let legends = legend::place(blocks, plot, bounds);
    let bounds = legends
        .iter()
        .filter_map(Legend::bounds)
        .fold(bounds, Rect::union);
    let title_over = heading.as_ref().is_some_and(Heading::over);
    let (title, subtitle) = match heading {
        Some(heading) => {
            let (title, subtitle) = heading.place(plot, bounds);
            (Some(title), subtitle)
        }
        None => (None, None),
    };
    Some(Plan {
        size,
        plot,
        title,
        subtitle,
        title_over,
        legends,
        axes,
        series,
        colors: colors.colors,
        chart: chart.clone(),
        hover,
        links,
    })
}

impl Chart {
    /// Whether [`plan`] draws it.
    pub fn drawable(&self) -> bool {
        let metrics = Metrics {
            size: 0.0,
            line: 0.0,
        };
        plan(self, [0.0, 0.0], metrics, &mut |_, _| 0.0).is_some()
    }
}

impl Plan {
    /// The chart as drawn: without the rows its layers' invalid data modes
    /// leave out of marks and domains alike.
    pub fn chart(&self) -> &Chart {
        &self.chart
    }

    /// The `href` of the topmost mark under `point` that has one.
    pub fn href(&self, point: [f32; 2]) -> Option<&SharedString> {
        self.links
            .iter()
            .rev()
            .find(|link| link.hit.contains(point))
            .map(|link| &link.href)
    }

    /// What `point` is over, if anything.
    pub fn hit(&self, point: [f32; 2]) -> Option<Tip> {
        if let Some((color, _, text)) = self
            .hover
            .slices
            .iter()
            .rev()
            .find(|(_, slice, _)| slice.contains(point))
        {
            return Some(Tip {
                title: self.colors[*color].name.clone().unwrap_or_default(),
                rows: vec![(*color, text.clone())],
                points: Vec::new(),
                band: None,
                rule: None,
            });
        }
        if !self.plot.contains(point) {
            return None;
        }

        let mut tip = Tip {
            title: SharedString::default(),
            rows: Vec::new(),
            points: Vec::new(),
            band: None,
            rule: None,
        };
        if let Some(bands) = &self.hover.bands {
            let Bands {
                cells,
                vertical,
                across,
                labels,
                values,
            } = bands;
            let at = if *vertical { point[0] } else { point[1] };
            let index = cells
                .iter()
                .position(|(start, step)| (*start..start + step).contains(&at))?;
            let (start, step) = cells[index];
            tip.title = labels[index].clone();
            tip.band = Some(match vertical {
                true => Rect {
                    x: start,
                    y: across.0,
                    w: step,
                    h: across.1 - across.0,
                },
                false => Rect {
                    x: across.0,
                    y: start,
                    w: across.1 - across.0,
                    h: step,
                },
            });
            tip.rows.extend(
                values
                    .iter()
                    .filter_map(|(color, values)| Some((*color, values[index].clone()?))),
            );
        }

        // Every trace runs along the same axis; the nearest point along it
        // and every other series' point there are read out.
        let Some(axis) = self.hover.traces.first().map(|trace| trace.axis) else {
            return (!tip.rows.is_empty()).then_some(tip);
        };
        let gap = |p: &([f32; 2], u32)| (p.0[axis] - point[axis]).abs();
        let at = self
            .hover
            .traces
            .iter()
            .filter_map(|trace| nearest(&trace.points, point[axis], axis))
            .min_by(|a, b| gap(a).total_cmp(&gap(b)))
            .map(|p| p.0[axis]);
        if let Some(at) = at {
            for trace in &self.hover.traces {
                let Some(&(p, row)) = nearest(&trace.points, at, axis) else {
                    continue;
                };
                if (p[axis] - at).abs() > SAME_X {
                    continue;
                }
                let encoding = &self.chart.layers[trace.layer].encoding;
                let (along, value) = match axis {
                    0 => (&encoding.x, &encoding.y),
                    _ => (&encoding.y, &encoding.x),
                };
                if tip.title.is_empty() {
                    tip.title = self.text(trace.layer, along.as_ref()?.channel()?, row as usize)?;
                }
                tip.rows.push((
                    trace.color,
                    self.text(trace.layer, value.as_ref()?.channel()?, row as usize)?,
                ));
                tip.points.push((trace.color, p));
            }
            if let Some(&(_, p)) = tip.points.first() {
                let plot = self.plot;
                tip.rule = Some(match axis {
                    0 => Rect {
                        x: p[0].round(),
                        y: plot.y,
                        w: 1.0,
                        h: plot.h,
                    },
                    _ => Rect {
                        x: plot.x,
                        y: p[1].round(),
                        w: plot.w,
                        h: 1.0,
                    },
                });
            }
        }
        (!tip.rows.is_empty()).then_some(tip)
    }

    /// Row `row` of `channel` in layer `layer`, read as its kind.
    fn text(&self, layer: usize, channel: &Channel, row: usize) -> Option<SharedString> {
        Some(match channel.column(&self.chart.layers[layer].data)? {
            Column::Number(values) => channel.kind.describe(values[row]).into(),
            column => column.label(row),
        })
    }
}

/// The point of `points`, ascending along `axis`, nearest `at` along it.
fn nearest(points: &[([f32; 2], u32)], at: f32, axis: usize) -> Option<&([f32; 2], u32)> {
    let index = points.partition_point(|(p, _)| p[axis] < at);
    [index.checked_sub(1), Some(index)]
        .into_iter()
        .flatten()
        .filter_map(|index| points.get(index))
        .min_by(|a, b| (a.0[axis] - at).abs().total_cmp(&(b.0[axis] - at).abs()))
}
