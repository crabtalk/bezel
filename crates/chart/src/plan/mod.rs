//! A chart laid out at a size: every mark, axis and label in pixels from the
//! chart's top-left corner, with no window involved.
//!
//! Bars sharing a band and an offset stack, positive values up and negative
//! down, unless `x2` or `y2` ranges them. Lines, areas and trails join their
//! series' rows in ascending x, or by `order`, through their interpolation's
//! curve, and a missing y breaks them. A rule, rect, text, tick, point or
//! image layer may leave out x or y where another layer has both: a rule or
//! rect then spans the plot that way, and the others stand at its middle.
//! A position in pixels places every row of its layer there, and a rect
//! placed in pixels without its far end reaches the base of the axis: zero,
//! or the axis's start. `xOffset` and `yOffset` values move a mark, and
//! `x2Offset` and `y2Offset` its far end where it has an `x2` or `y2`.
//! Arcs, and text with a `theta`, over axes stack one pie per place their x
//! and y give, at the plot's middle without them, half the plot's shorter
//! side across without a `radius`. A layer's rows split into series by colour, `detail` and `key`, and for a
//! line, area or bar by every other channel that names a field but shape and
//! angle. A channel's [`Scale`] and [`Sort`] shape its domain, and its title
//! sits under the bottom axis or over the left one. The pointer reads out the
//! band under it, and the nearest row along the shared axis of each series
//! of a line, area, trail or point layer placed on scales both ways.
//!
//! [`Scale`]: crate::model::Scale
//! [`Sort`]: crate::model::Sort

use std::{f32::consts::TAU, sync::Arc};

use gpui::{FontStyle, FontWeight, Hsla, SharedString};
use js::curve::Curve;
use theme::Theme;

use crate::{
    data::Column,
    model::{Align, Baseline, Cap, Channel, Chart, Dir, Encode, Encoding, Join, Mark, Shape},
    plan::{domain::Colors, layout::Layout},
    scale::Band,
};

mod arcs;
mod cartesian;
mod domain;
mod layout;
mod legend;
mod look;

/// Around the whole chart.
const PAD: f32 = 8.0;
/// Between a label and what it labels.
const GAP: f32 = 6.0;
const SWATCH: f32 = 10.0;
/// Between legend entries.
const LEGEND_GAP: f32 = 16.0;
/// Roughly how far apart ticks stand along x, and along y.
const X_SPACING: f32 = 80.0;
const Y_SPACING: f32 = 40.0;
const BAND_PADDING: f32 = 0.2;
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
}

/// A line of text one line high, its top-left at `at`, `width` as measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: SharedString,
    pub at: [f32; 2],
    pub width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Bottom,
    Left,
}

/// `at` is x on a bottom axis and y on a left one.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub at: f32,
    pub label: Option<Label>,
}

/// `grid` for an axis of values, whose ticks run lines across the plot.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub side: Side,
    pub ticks: Vec<Tick>,
    pub grid: bool,
    pub title: Option<Label>,
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
}

impl Ink {
    pub fn on(self, theme: &Theme) -> Hsla {
        match self {
            Ink::Category(index) => theme.categorical(index),
            Ink::Fixed(color) => color,
            Ink::Muted => theme.text_muted,
            Ink::Text => theme.text,
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

/// A legend: its title, then its entries flowing left to right.
#[derive(Clone, Debug, PartialEq)]
pub struct Legend {
    pub title: Option<Label>,
    pub swatches: Vec<Swatch>,
}

/// One legend entry: `glyph` drawn in `swatch`, then its label.
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    pub glyph: Glyph,
    pub swatch: Rect,
    pub label: Label,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Glyph {
    /// A filled square.
    Fill { ink: Ink, opacity: f32 },
    /// A point's symbol, `size` square pixels.
    Symbol { shape: Shape, size: f32, ink: Ink },
    /// A short line across the swatch.
    Stroke {
        ink: Ink,
        width: f32,
        dash: Option<Arc<[f32]>>,
    },
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
    band: Band,
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

/// `chart` laid out at `size`, its labels `line` high and as wide as `measure`
/// says. `None` for what cannot be drawn: a field the data does not hold, a
/// quantitative field over text, a colour, fill, stroke, shape or dash that
/// is not discrete, layers that disagree on which axis is discrete, no layer
/// with both x and y where a layer is placed on a scale, a text at an angle
/// or with an outline, or a mark without the positions it needs. A size too small to
/// hold a plot draws a cramped one, so no size changes whether a chart draws.
pub fn plan(
    chart: &Chart,
    size: [f32; 2],
    line: f32,
    measure: &mut dyn FnMut(&str) -> f32,
) -> Option<Plan> {
    let colors = Colors::of(chart)?;
    let mut layout = Layout {
        size,
        line,
        top: PAD,
        reserved: 0.0,
        measure,
    };
    let first_title = |channel: fn(&Encoding) -> Option<&Channel>| {
        chart
            .layers
            .iter()
            .find_map(|layer| channel(&layer.encoding)?.title.clone())
    };
    let title = chart.title.clone().map(|text| layout.line_at(text, PAD));
    let legends = legend::legends(chart, &colors)?
        .into_iter()
        .map(|(title, entries)| layout.legend(title, entries))
        .collect();
    // Arcs and text alone, none placed on a scale, draw without axes.
    let polar = chart.layers.iter().any(|layer| layer.mark == Mark::Arc)
        && chart.layers.iter().all(|layer| {
            let e = &layer.encoding;
            matches!(layer.mark, Mark::Arc | Mark::Text)
                && [&e.x, &e.y]
                    .iter()
                    .all(|p| p.as_ref().and_then(Encode::channel).is_none())
        });
    let x_title = first_title(|e| e.x.as_ref()?.channel()).filter(|_| !polar);
    if x_title.is_some() {
        layout.reserved = line + GAP;
    }
    let y_title = first_title(|e| e.y.as_ref()?.channel())
        .filter(|_| !polar)
        .map(|text| layout.line_at(text, PAD));

    // Text is drawn level, and filled without an outline: gpui paints a
    // glyph as a mask in one colour.
    if chart.layers.iter().any(|layer| {
        let e = &layer.encoding;
        let outlined = match (&e.stroke, &e.stroke_width) {
            (_, Some(Encode::Value(width))) if *width <= 0.0 => false,
            (Some(Encode::Value(color)), _) => color.a > 0.0,
            (Some(Encode::Scaled(_)), _) => true,
            (None, _) => layer.properties.filled == Some(false),
        };
        layer.mark == Mark::Text
            && (outlined || !matches!(e.angle, None | Some(Encode::Value(0.0))))
    }) {
        return None;
    }
    let (plot, mut axes, series, hover, links) = match (polar, chart.layers.as_slice()) {
        (true, _) => arcs::arcs(chart, &colors, &layout)?,
        (false, []) => return None,
        (false, _) => cartesian::cartesian(chart, &colors, &mut layout)?,
    };
    let x_title = x_title.map(|text| {
        let label = layout.label(text, [0.0, 0.0]);
        Label {
            at: [
                plot.x + (plot.w - label.width) / 2.0,
                plot.bottom() + GAP + line + GAP,
            ],
            ..label
        }
    });
    for axis in &mut axes {
        axis.title = match axis.side {
            Side::Bottom => x_title.clone(),
            Side::Left => y_title.clone(),
        };
    }
    Some(Plan {
        size,
        plot,
        title,
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
        plan(self, [0.0, 0.0], 0.0, &mut |_| 0.0).is_some()
    }
}

impl Plan {
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
                band,
                vertical,
                across,
                labels,
                values,
            } = bands;
            let index = band.index(if *vertical { point[0] } else { point[1] })?;
            let start = band.range.0 + band.step() * index as f32;
            tip.title = labels[index].clone();
            tip.band = Some(match vertical {
                true => Rect {
                    x: start,
                    y: across.0,
                    w: band.step(),
                    h: across.1 - across.0,
                },
                false => Rect {
                    x: across.0,
                    y: start,
                    w: across.1 - across.0,
                    h: band.step(),
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
            Column::Text(text) => text.get(row).clone(),
            Column::Number(values) => channel.kind.describe(values[row]).into(),
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
