//! A chart laid out at a size: every mark, axis and label in pixels from the
//! chart's top-left corner, with no window involved.
//!
//! Bars sharing a band stack, positive values up and negative down. Lines,
//! areas and points take their series in ascending x, and a missing y breaks
//! a line or area. A channel's [`Scale`] and [`Sort`] shape its domain, and its
//! title sits under the bottom axis or over the left one.

use std::{
    collections::{HashMap, HashSet},
    f32::consts::TAU,
    sync::Arc,
};

use gpui::SharedString;

use crate::{
    data::Column,
    decimate,
    model::{Channel, Chart, Encoding, Kind, Layer, Mark, Scale, Sort},
    scale::{self, Band, Linear, Ticks},
    time,
};

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
/// Below this either way there is no plot to draw in.
const MIN_PLOT: f32 = 16.0;
/// How far from the hovered x another series' point may be and still be read
/// out with it.
const SAME_X: f32 = 1.0;

/// A missing code in a discrete column.
const MISSING: u32 = u32::MAX;

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

#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    pub series: usize,
    pub swatch: Rect,
    pub label: Label,
}

/// Angles in radians, clockwise from twelve o'clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slice {
    pub center: [f32; 2],
    pub radius: f32,
    pub start: f32,
    pub end: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Marks {
    Bars(Vec<Rect>),
    /// Each a run of points with no gap in it.
    Lines(Vec<Vec<[f32; 2]>>),
    /// Each a closed outline: a run along the top, then back along zero.
    Areas(Vec<Vec<[f32; 2]>>),
    /// At most one to a pixel.
    Points(Vec<[f32; 2]>),
    Slices(Vec<Slice>),
}

/// One colour's marks in one layer. `name` is its value of the colour field.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub name: Option<SharedString>,
    /// Its place in the chart's colours.
    pub color: usize,
    pub mark: Mark,
    pub marks: Marks,
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
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub size: [f32; 2],
    /// What the marks are drawn in. For arcs, the square the pie fills.
    pub plot: Rect,
    pub title: Option<Label>,
    /// The colour channel's title, ahead of the swatches.
    pub legend_title: Option<Label>,
    pub legend: Vec<Swatch>,
    pub axes: Vec<Axis>,
    /// Paint order.
    pub series: Vec<Series>,
    /// Each colour's value of the colour field. `None` for a layer's own
    /// colour, where it has no colour field.
    pub colors: Vec<Option<SharedString>>,
    chart: Chart,
    hover: Hover,
}

#[derive(Clone, Debug, Default)]
struct Hover {
    bands: Option<Bands>,
    traces: Vec<Trace>,
    slices: Option<Slices>,
}

/// Bars' values by band.
#[derive(Clone, Debug)]
struct Bands {
    band: Band,
    vertical: bool,
    across: (f32, f32),
    labels: Vec<SharedString>,
    /// Colour, and its value in each band, `NaN` for none.
    values: Vec<(usize, Vec<f64>)>,
}

/// One colour of a line, area or point layer, ascending in x: each point and
/// its row.
#[derive(Clone, Debug)]
struct Trace {
    layer: usize,
    color: usize,
    points: Vec<([f32; 2], u32)>,
}

#[derive(Clone, Debug)]
struct Slices {
    slices: Vec<(usize, Slice, f64)>,
    total: f64,
}

/// `chart` laid out at `size`, its labels `line` high and as wide as `measure`
/// says. `None` for what cannot be drawn: a field the data does not hold, a
/// quantitative field over text, a colour that is not discrete, layers that
/// disagree on which axis is discrete, a layered arc, or a size too small to
/// hold a plot.
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
    let first_title = |channel: fn(&Encoding) -> &Option<Channel>| {
        chart
            .layers
            .iter()
            .find_map(|layer| channel(&layer.encoding).as_ref()?.title.clone())
    };
    let title = chart.title.clone().map(|text| layout.line_at(text, PAD));
    let (legend_title, legend) = layout.legend(first_title(|e| &e.color), &colors.names);
    let arc = chart.layers.iter().any(|layer| layer.mark == Mark::Arc);
    let x_title = first_title(|e| &e.x).filter(|_| !arc);
    if x_title.is_some() {
        layout.reserved = line + GAP;
    }
    let y_title = first_title(|e| &e.y)
        .filter(|_| !arc)
        .map(|text| layout.line_at(text, PAD));

    let (plot, mut axes, series, hover) = match (arc, chart.layers.as_slice()) {
        (true, [layer]) => arcs(layer, &colors, &layout)?,
        (true, _) | (false, []) => return None,
        (false, _) => cartesian(chart, &colors, &mut layout)?,
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
        legend_title,
        legend,
        axes,
        series,
        colors: colors.names,
        chart: chart.clone(),
        hover,
    })
}

impl Plan {
    pub fn chart(&self) -> &Chart {
        &self.chart
    }

    /// What `point` is over, if anything.
    pub fn hit(&self, point: [f32; 2]) -> Option<Tip> {
        if let Some(Slices { slices, total }) = &self.hover.slices {
            let (center, radius) = slices.first().map(|(_, s, _)| (s.center, s.radius))?;
            let (dx, dy) = (point[0] - center[0], point[1] - center[1]);
            if dx.hypot(dy) > radius {
                return None;
            }
            let angle = dx.atan2(-dy).rem_euclid(TAU);
            let (color, _, value) = slices
                .iter()
                .find(|(_, slice, _)| angle >= slice.start && angle < slice.end)?;
            let share = value / total * 100.0;
            return Some(Tip {
                title: self.colors[*color].clone().unwrap_or_default(),
                rows: vec![(
                    *color,
                    format!("{} ({share:.1}%)", scale::value(*value)).into(),
                )],
                points: Vec::new(),
                band: None,
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
            tip.rows.extend(values.iter().filter_map(|(color, values)| {
                let value = values[index];
                (!value.is_nan()).then(|| (*color, scale::value(value).into()))
            }));
        }

        let gap = |p: &([f32; 2], u32)| (p.0[0] - point[0]).abs();
        let at = self
            .hover
            .traces
            .iter()
            .filter_map(|trace| nearest(&trace.points, point[0]))
            .min_by(|a, b| gap(a).total_cmp(&gap(b)))
            .map(|p| p.0[0]);
        if let Some(at) = at {
            for trace in &self.hover.traces {
                let Some(&(p, row)) = nearest(&trace.points, at) else {
                    continue;
                };
                if (p[0] - at).abs() > SAME_X {
                    continue;
                }
                let encoding = &self.chart.layers[trace.layer].encoding;
                if tip.title.is_empty() {
                    tip.title = self.text(trace.layer, encoding.x.as_ref()?, row as usize)?;
                }
                tip.rows.push((
                    trace.color,
                    self.text(trace.layer, encoding.y.as_ref()?, row as usize)?,
                ));
                tip.points.push((trace.color, p));
            }
        }
        (!tip.rows.is_empty()).then_some(tip)
    }

    /// Row `row` of `channel`'s field in layer `layer`, read as its kind.
    fn text(&self, layer: usize, channel: &Channel, row: usize) -> Option<SharedString> {
        Some(
            match self.chart.layers[layer].data.column(&channel.field)? {
                Column::Text(text) => text.get(row).clone(),
                Column::Number(values) => match channel.kind {
                    Kind::Temporal => time::describe(values[row]).into(),
                    _ => scale::value(values[row]).into(),
                },
            },
        )
    }
}

/// The point of `points`, ascending in x, nearest `at` along x.
fn nearest(points: &[([f32; 2], u32)], at: f32) -> Option<&([f32; 2], u32)> {
    let index = points.partition_point(|(p, _)| p[0] < at);
    [index.checked_sub(1), Some(index)]
        .into_iter()
        .flatten()
        .filter_map(|index| points.get(index))
        .min_by(|a, b| (a.0[0] - at).abs().total_cmp(&(b.0[0] - at).abs()))
}

/// The chart's colours: every colour field's values across the layers, in
/// the order met, then one for each layer with no colour field.
struct Colors {
    names: Vec<Option<SharedString>>,
    layers: Vec<LayerColor>,
}

enum LayerColor {
    /// Each row's colour.
    Field(Vec<u32>),
    Fixed(usize),
}

impl Colors {
    fn of(chart: &Chart) -> Option<Self> {
        let mut names = Vec::new();
        let mut index: HashMap<SharedString, u32> = HashMap::new();
        let mut layers = Vec::with_capacity(chart.layers.len());
        for layer in &chart.layers {
            let Some(color) = &layer.encoding.color else {
                layers.push(None);
                continue;
            };
            if !discrete(color.kind) {
                return None;
            }
            let domain = Discrete::of(layer.data.column(&color.field)?, &color.sort);
            let global: Vec<u32> = domain
                .labels
                .into_iter()
                .map(|label| {
                    *index.entry(label.clone()).or_insert_with(|| {
                        names.push(Some(label));
                        names.len() as u32 - 1
                    })
                })
                .collect();
            let codes = domain.codes.iter().map(|&code| match code {
                MISSING => MISSING,
                code => global[code as usize],
            });
            layers.push(Some(LayerColor::Field(codes.collect())));
        }
        let layers = layers
            .into_iter()
            .map(|color| {
                color.unwrap_or_else(|| {
                    names.push(None);
                    LayerColor::Fixed(names.len() - 1)
                })
            })
            .collect();
        Some(Self { names, layers })
    }

    fn of_row(&self, layer: usize, row: usize) -> Option<usize> {
        match &self.layers[layer] {
            LayerColor::Fixed(color) => Some(*color),
            LayerColor::Field(codes) => (codes[row] != MISSING).then_some(codes[row] as usize),
        }
    }

    fn len(&self) -> usize {
        self.names.len()
    }
}

/// A column's distinct values in order, and each row's index into them.
struct Discrete {
    labels: Vec<SharedString>,
    codes: Arc<[u32]>,
}

impl Discrete {
    fn of(column: &Column, sort: &Sort) -> Self {
        let (labels, codes, numbers): (Vec<SharedString>, Arc<[u32]>, Option<Vec<f64>>) =
            match column {
                Column::Text(text) => (text.names.to_vec(), text.codes.clone(), None),
                Column::Number(values) => {
                    let mut index = HashMap::new();
                    let mut distinct = Vec::new();
                    let codes = values
                        .iter()
                        .map(|&v| match v.is_nan() {
                            true => MISSING,
                            false => *index.entry(v.to_bits()).or_insert_with(|| {
                                distinct.push(v);
                                distinct.len() as u32 - 1
                            }),
                        })
                        .collect();
                    let labels = distinct.iter().map(|&v| scale::value(v).into()).collect();
                    (labels, codes, Some(distinct))
                }
            };
        let mut order: Vec<usize> = (0..labels.len()).collect();
        match sort {
            Sort::Data => return Self { labels, codes },
            Sort::Ascending | Sort::Descending => {
                order.sort_by(|&a, &b| match &numbers {
                    Some(values) => values[a].total_cmp(&values[b]),
                    None => labels[a].cmp(&labels[b]),
                });
                if *sort == Sort::Descending {
                    order.reverse();
                }
            }
            Sort::Explicit(listed) => {
                let rank = |index: &usize| {
                    listed
                        .iter()
                        .position(|name| *name == labels[*index])
                        .unwrap_or(listed.len())
                };
                order.sort_by_key(rank);
            }
        }
        let mut rank = vec![0u32; labels.len()];
        for (new, &old) in order.iter().enumerate() {
            rank[old] = new as u32;
        }
        Self {
            labels: order.iter().map(|&old| labels[old].clone()).collect(),
            codes: codes
                .iter()
                .map(|&code| match code {
                    MISSING => MISSING,
                    code => rank[code as usize],
                })
                .collect(),
        }
    }
}

fn discrete(kind: Kind) -> bool {
    matches!(kind, Kind::Ordinal | Kind::Nominal)
}

/// Ticks for a quantitative or temporal channel whose data spans `lo..=hi`.
fn continuous(channel: &Channel, lo: f64, hi: f64, count: usize) -> Ticks {
    let Scale { zero, domain, nice } = channel.scale;
    match (channel.kind, domain) {
        (Kind::Temporal, Some([lo, hi])) => scale::temporal(lo, hi, count),
        (Kind::Temporal, None) => scale::temporal(lo, hi, count),
        (_, Some([lo, hi])) => scale::linear(lo, hi, count, false),
        (_, None) if zero => scale::linear(lo.min(0.0), hi.max(0.0), count, nice),
        (_, None) => scale::linear(lo, hi, count, nice),
    }
}

/// The chart's size, and how far down its title and legend reach.
struct Layout<'a> {
    size: [f32; 2],
    line: f32,
    top: f32,
    /// Kept under the bottom axis, for its title.
    reserved: f32,
    measure: &'a mut dyn FnMut(&str) -> f32,
}

impl Layout<'_> {
    fn label(&mut self, text: SharedString, at: [f32; 2]) -> Label {
        let width = (self.measure)(&text);
        Label { text, at, width }
    }

    /// A full-width line of the header, at `x`.
    fn line_at(&mut self, text: SharedString, x: f32) -> Label {
        let label = self.label(text, [x, self.top]);
        self.top += self.line + GAP;
        label
    }

    /// `title`, then one entry per named series, flowing left to right and
    /// wrapping.
    fn legend(
        &mut self,
        title: Option<SharedString>,
        names: &[Option<SharedString>],
    ) -> (Option<Label>, Vec<Swatch>) {
        let mut swatches = Vec::new();
        let mut x = PAD;
        let title = title
            .filter(|_| names.iter().any(Option::is_some))
            .map(|text| {
                let label = self.label(text, [PAD, self.top]);
                x += label.width + GAP;
                label
            });
        for (series, name) in names.iter().enumerate() {
            let Some(name) = name else { continue };
            let label = self.label(name.clone(), [0.0, 0.0]);
            let width = SWATCH + GAP + label.width;
            if x > PAD && x + width > self.size[0] - PAD {
                x = PAD;
                self.top += self.line;
            }
            swatches.push(Swatch {
                series,
                swatch: Rect {
                    x,
                    y: self.top + (self.line - SWATCH) / 2.0,
                    w: SWATCH,
                    h: SWATCH,
                },
                label: Label {
                    at: [x + SWATCH + GAP, self.top],
                    ..label
                },
            });
            x += width + LEGEND_GAP;
        }
        if !swatches.is_empty() {
            self.top += self.line + GAP;
        }
        (title, swatches)
    }

    /// The plot's height once a bottom axis is taken out.
    fn plot_height(&self) -> f32 {
        self.size[1] - self.top - PAD - GAP - self.line - self.reserved
    }

    /// The plot beside left labels `left` wide, over a bottom axis.
    fn plot(&self, left: f32) -> Option<Rect> {
        let x = PAD + left + GAP;
        let plot = Rect {
            x,
            y: self.top,
            w: self.size[0] - x - PAD,
            h: self.plot_height(),
        };
        (plot.w >= MIN_PLOT && plot.h >= MIN_PLOT).then_some(plot)
    }

    fn measured(&mut self, labels: &[SharedString]) -> Vec<Label> {
        labels
            .iter()
            .map(|text| self.label(text.clone(), [0.0, 0.0]))
            .collect()
    }

    /// A left axis of measured `labels`, each centred on its `at` and
    /// right-aligned against `plot`. Past one to a line, every k-th.
    fn left(&self, labels: Vec<Label>, at: &[f32], plot: Rect, grid: bool) -> Axis {
        let every = spacing(at).map_or(1, |step| (self.line / step).ceil().max(1.0) as usize);
        Axis {
            side: Side::Left,
            ticks: labels
                .into_iter()
                .zip(at)
                .enumerate()
                .map(|(index, (label, &y))| Tick {
                    at: y,
                    label: (index % every == 0).then(|| Label {
                        at: [plot.x - GAP - label.width, y - self.line / 2.0],
                        ..label
                    }),
                })
                .collect(),
            grid,
            title: None,
        }
    }

    /// A bottom axis under `plot`, each label centred on its `at` and kept
    /// inside the chart. Past what fits side by side, every k-th.
    fn bottom(&mut self, labels: &[SharedString], at: &[f32], plot: Rect, grid: bool) -> Axis {
        let measured = self.measured(labels);
        let every = spacing(at).map_or(1, |step| {
            ((widest(&measured) + GAP) / step).ceil().max(1.0) as usize
        });
        let ticks = measured
            .into_iter()
            .zip(at)
            .enumerate()
            .map(|(index, (label, &x))| Tick {
                at: x,
                label: (index % every == 0).then(|| Label {
                    at: [
                        (x - label.width / 2.0).clamp(0.0, (self.size[0] - label.width).max(0.0)),
                        plot.bottom() + GAP,
                    ],
                    ..label
                }),
            })
            .collect();
        Axis {
            side: Side::Bottom,
            ticks,
            grid,
            title: None,
        }
    }
}

/// How far apart evenly spaced positions are.
fn spacing(at: &[f32]) -> Option<f32> {
    match at {
        [a, b, ..] => Some((b - a).abs()),
        _ => None,
    }
}

fn widest(labels: &[Label]) -> f32 {
    labels.iter().map(|l| l.width).fold(0.0, f32::max)
}

type Body = (Rect, Vec<Axis>, Vec<Series>, Hover);

/// Where a row lands along the shared axis, in pixels.
type Place = Box<dyn Fn(&Along, usize) -> f32>;

/// Where a layer's rows fall along the axis the layers share their domain on.
enum Along {
    /// Codes into the chart's discrete domain.
    Discrete(Vec<u32>),
    Continuous(Arc<[f64]>),
}

impl Along {
    fn valid(&self, row: usize) -> bool {
        match self {
            Along::Discrete(codes) => codes[row] != MISSING,
            Along::Continuous(values) => !values[row].is_nan(),
        }
    }
}

/// A layer read for laying out: its data along the shared axis and along its
/// values, and for bars, each band's total per colour.
struct Read<'a> {
    index: usize,
    mark: Mark,
    along: Along,
    values: &'a Arc<[f64]>,
    /// `sums[band * colours + colour]`, `NaN` for none.
    sums: Vec<f64>,
}

fn cartesian(chart: &Chart, colors: &Colors, layout: &mut Layout) -> Option<Body> {
    // Bars along y are the only marks that put the shared axis on the left.
    let horizontal = chart.layers.iter().all(|layer| {
        let encoding = &layer.encoding;
        layer.mark == Mark::Bar
            && encoding.y.as_ref().is_some_and(|y| discrete(y.kind))
            && encoding.x.as_ref().is_some_and(|x| !discrete(x.kind))
    });
    let channels = |layer: &'_ Layer| -> Option<(Channel, Channel)> {
        let (x, y) = (layer.encoding.x.clone()?, layer.encoding.y.clone()?);
        Some(if horizontal { (y, x) } else { (x, y) })
    };
    let (shared, value) = channels(&chart.layers[0])?;
    let is_discrete = discrete(shared.kind);

    let mut labels: Vec<SharedString> = Vec::new();
    let mut index: HashMap<SharedString, u32> = HashMap::new();
    let mut read = Vec::with_capacity(chart.layers.len());
    for (layer_index, layer) in chart.layers.iter().enumerate() {
        let (along_channel, value_channel) = channels(layer)?;
        if discrete(along_channel.kind) != is_discrete
            || value_channel.kind != Kind::Quantitative
            || (layer.mark == Mark::Bar && !is_discrete)
        {
            return None;
        }
        let column = layer.data.column(&along_channel.field)?;
        let along = match (is_discrete, column) {
            (true, column) => {
                let domain = Discrete::of(column, &along_channel.sort);
                let global: Vec<u32> = domain
                    .labels
                    .into_iter()
                    .map(|label| {
                        *index.entry(label.clone()).or_insert_with(|| {
                            labels.push(label);
                            labels.len() as u32 - 1
                        })
                    })
                    .collect();
                Along::Discrete(
                    domain
                        .codes
                        .iter()
                        .map(|&code| match code {
                            MISSING => MISSING,
                            code => global[code as usize],
                        })
                        .collect(),
                )
            }
            (false, Column::Number(values)) => Along::Continuous(values.clone()),
            (false, Column::Text(_)) => return None,
        };
        let values = match layer.data.column(&value_channel.field)? {
            Column::Number(values) => values,
            Column::Text(_) => return None,
        };
        read.push(Read {
            index: layer_index,
            mark: layer.mark,
            along,
            values,
            sums: Vec::new(),
        });
    }

    let count = labels.len();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for layer in &mut read {
        let rows = (0..layer.values.len())
            .filter(|&row| layer.along.valid(row) && colors.of_row(layer.index, row).is_some());
        if layer.mark != Mark::Bar {
            for row in rows.filter(|&row| !layer.values[row].is_nan()) {
                (lo, hi) = (lo.min(layer.values[row]), hi.max(layer.values[row]));
            }
            continue;
        }
        let Along::Discrete(codes) = &layer.along else {
            return None;
        };
        let mut sums = vec![f64::NAN; count * colors.len()];
        for row in rows {
            let (v, color) = (layer.values[row], colors.of_row(layer.index, row)?);
            if v.is_nan() {
                continue;
            }
            let sum = &mut sums[codes[row] as usize * colors.len() + color];
            *sum = if sum.is_nan() { v } else { *sum + v };
        }
        for stack in sums.chunks(colors.len().max(1)) {
            let up: f64 = stack.iter().filter(|v| **v > 0.0).sum();
            let down: f64 = stack.iter().filter(|v| **v < 0.0).sum();
            (lo, hi) = (lo.min(down.min(0.0)), hi.max(up.max(0.0)));
        }
        layer.sums = sums;
    }
    let (lo, hi) = if lo > hi { (0.0, 0.0) } else { (lo, hi) };

    let barred = read.iter().any(|layer| layer.mark == Mark::Bar);
    let padding = if barred { BAND_PADDING } else { 0.0 };
    let (plot, axes, value_scale, place): (Rect, Vec<Axis>, Linear, Place) = if horizontal {
        let measured = layout.measured(&labels);
        let plot = layout.plot(widest(&measured))?;
        let band = Band {
            count,
            range: (plot.y, plot.bottom()),
            padding,
        };
        let centers: Vec<f32> = (0..count).map(|b| band.center(b)).collect();
        let left = layout.left(measured, &centers, plot, false);
        let ticks = continuous(&value, lo, hi, x_count(plot));
        let scale = Linear {
            domain: ticks.domain,
            range: (plot.x, plot.right()),
        };
        let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
        let axes = vec![layout.bottom(&ticks.labels, &at, plot, true), left];
        (plot, axes, scale, Box::new(move |_: &Along, _| 0.0))
    } else {
        let count_y = (layout.plot_height() / Y_SPACING).floor().max(2.0) as usize;
        let ticks = continuous(&value, lo, hi, count_y);
        let measured = layout.measured(&ticks.labels);
        let plot = layout.plot(widest(&measured))?;
        let scale = Linear {
            domain: ticks.domain,
            range: (plot.bottom(), plot.y),
        };
        let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
        let left = layout.left(measured, &at, plot, true);
        let (bottom, place): (Axis, Place) = if is_discrete {
            let band = Band {
                count,
                range: (plot.x, plot.right()),
                padding,
            };
            let centers: Vec<f32> = (0..count).map(|b| band.center(b)).collect();
            let axis = layout.bottom(&labels, &centers, plot, false);
            (
                axis,
                Box::new(move |along: &Along, row| match along {
                    Along::Discrete(codes) => band.center(codes[row] as usize),
                    Along::Continuous(_) => f32::NAN,
                }),
            )
        } else {
            let (lo, hi) = read
                .iter()
                .flat_map(|layer| {
                    (0..layer.values.len())
                        .filter(|&row| layer.along.valid(row))
                        .filter_map(|row| match &layer.along {
                            Along::Continuous(values) => Some(values[row]),
                            Along::Discrete(_) => None,
                        })
                })
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                });
            let (lo, hi) = if lo > hi { (0.0, 0.0) } else { (lo, hi) };
            let ticks = continuous(&shared, lo, hi, x_count(plot));
            let scale = Linear {
                domain: ticks.domain,
                range: (plot.x, plot.right()),
            };
            let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
            let axis = layout.bottom(&ticks.labels, &at, plot, false);
            (
                axis,
                Box::new(move |along: &Along, row| match along {
                    Along::Continuous(values) => scale.at(values[row]),
                    Along::Discrete(_) => f32::NAN,
                }),
            )
        };
        (plot, vec![left, bottom], scale, place)
    };

    let band_axis = match horizontal {
        true => (plot.y, plot.bottom()),
        false => (plot.x, plot.right()),
    };
    let band = Band {
        count,
        range: band_axis,
        padding,
    };
    let base = value_scale.at(0.0f64.clamp(
        value_scale.domain.0.min(value_scale.domain.1),
        value_scale.domain.0.max(value_scale.domain.1),
    ));
    let columns = plot.w.ceil() as usize;
    let mut series = Vec::new();
    let mut hover = Hover::default();
    for layer in &read {
        let name = |color: usize| colors.names[color].clone();
        if layer.mark == Mark::Bar {
            let mut bars = vec![Vec::new(); colors.len()];
            for (b, stack) in layer.sums.chunks(colors.len().max(1)).enumerate() {
                let (mut up, mut down) = (0.0, 0.0);
                for (color, &v) in stack.iter().enumerate() {
                    if v.is_nan() || v == 0.0 {
                        continue;
                    }
                    let base = if v > 0.0 { &mut up } else { &mut down };
                    let (from, to) = (value_scale.at(*base), value_scale.at(*base + v));
                    *base += v;
                    let (start, extent) = (band.start(b), band.width());
                    bars[color].push(match horizontal {
                        false => Rect {
                            x: start,
                            y: from.min(to),
                            w: extent,
                            h: (to - from).abs(),
                        },
                        true => Rect {
                            x: from.min(to),
                            y: start,
                            w: (to - from).abs(),
                            h: extent,
                        },
                    });
                }
            }
            let bands = hover.bands.get_or_insert_with(|| Bands {
                band,
                vertical: !horizontal,
                across: match horizontal {
                    true => (plot.x, plot.right()),
                    false => (plot.y, plot.bottom()),
                },
                labels: labels.clone(),
                values: Vec::new(),
            });
            for (color, bars) in bars.into_iter().enumerate() {
                if bars.is_empty() {
                    continue;
                }
                let values = (0..count)
                    .map(|b| layer.sums[b * colors.len() + color])
                    .collect();
                bands.values.push((color, values));
                series.push(Series {
                    name: name(color),
                    color,
                    mark: Mark::Bar,
                    marks: Marks::Bars(bars),
                });
            }
            continue;
        }

        let mut points: Vec<Vec<([f32; 2], u32)>> = vec![Vec::new(); colors.len()];
        for row in (0..layer.values.len()).filter(|&row| layer.along.valid(row)) {
            let Some(color) = colors.of_row(layer.index, row) else {
                continue;
            };
            let v = layer.values[row];
            let y = if v.is_nan() {
                f32::NAN
            } else {
                value_scale.at(v)
            };
            points[color].push(([place(&layer.along, row), y], row as u32));
        }
        for (color, mut points) in points.into_iter().enumerate() {
            if points.is_empty() {
                continue;
            }
            if !points.is_sorted_by(|a, b| a.0[0] <= b.0[0]) {
                points.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]));
            }
            let runs = || {
                points
                    .split(|(p, _)| p[1].is_nan())
                    .filter(|run| !run.is_empty())
                    .map(|run| {
                        let run: Vec<[f32; 2]> = run.iter().map(|(p, _)| *p).collect();
                        if run.len() > columns {
                            decimate::m4(&run)
                        } else {
                            run
                        }
                    })
            };
            let marks = match layer.mark {
                Mark::Line => Marks::Lines(runs().collect()),
                Mark::Area => Marks::Areas(
                    runs()
                        .map(|mut run| {
                            let (first, last) = (run[0][0], run[run.len() - 1][0]);
                            run.extend([[last, base], [first, base]]);
                            run
                        })
                        .collect(),
                ),
                _ => {
                    let mut seen = HashSet::new();
                    let dense = points.len() > columns;
                    Marks::Points(
                        points
                            .iter()
                            .filter(|(p, _)| !p[1].is_nan())
                            .filter(|(p, _)| {
                                !dense || seen.insert((p[0].round() as i32, p[1].round() as i32))
                            })
                            .map(|(p, _)| *p)
                            .collect(),
                    )
                }
            };
            series.push(Series {
                name: name(color),
                color,
                mark: layer.mark,
                marks,
            });
            points.retain(|(p, _)| !p[1].is_nan());
            hover.traces.push(Trace {
                layer: layer.index,
                color,
                points,
            });
        }
    }
    Some((plot, axes, series, hover))
}

/// About one tick to [`X_SPACING`] across `plot`.
fn x_count(plot: Rect) -> usize {
    (plot.w / X_SPACING).floor().max(2.0) as usize
}

fn arcs(layer: &Layer, colors: &Colors, layout: &Layout) -> Option<Body> {
    let theta = layer.encoding.theta.as_ref()?;
    if theta.kind != Kind::Quantitative {
        return None;
    }
    let Column::Number(values) = layer.data.column(&theta.field)? else {
        return None;
    };
    let room = Rect {
        x: PAD,
        y: layout.top,
        w: layout.size[0] - 2.0 * PAD,
        h: layout.size[1] - layout.top - PAD,
    };
    let radius = room.w.min(room.h) / 2.0;
    if radius * 2.0 < MIN_PLOT {
        return None;
    }
    let center = [room.x + room.w / 2.0, room.y + room.h / 2.0];

    let parts: Vec<(usize, f64)> = match colors.layers[0] {
        LayerColor::Field(_) => {
            let mut sums = vec![0.0; colors.len()];
            for (row, &v) in values.iter().enumerate() {
                if let Some(color) = colors.of_row(0, row)
                    && v > 0.0
                {
                    sums[color] += v;
                }
            }
            sums.into_iter()
                .enumerate()
                .filter(|(_, v)| *v > 0.0)
                .collect()
        }
        LayerColor::Fixed(color) => values
            .iter()
            .filter(|v| **v > 0.0)
            .map(|&v| (color, v))
            .collect(),
    };
    let total: f64 = parts.iter().map(|(_, v)| v).sum();
    let mut by_color: Vec<Vec<Slice>> = vec![Vec::new(); colors.len()];
    let mut slices = Vec::new();
    let mut start = 0.0f64;
    for (color, value) in parts {
        let end = start + value / total;
        let slice = Slice {
            center,
            radius,
            start: start as f32 * TAU,
            end: end as f32 * TAU,
        };
        by_color[color].push(slice);
        slices.push((color, slice, value));
        start = end;
    }
    let plot = Rect {
        x: center[0] - radius,
        y: center[1] - radius,
        w: radius * 2.0,
        h: radius * 2.0,
    };
    let series = by_color
        .into_iter()
        .enumerate()
        .filter(|(_, slices)| !slices.is_empty())
        .map(|(color, slices)| Series {
            name: colors.names[color].clone(),
            color,
            mark: Mark::Arc,
            marks: Marks::Slices(slices),
        })
        .collect();
    let hover = Hover {
        slices: Some(Slices { slices, total }),
        ..Hover::default()
    };
    Some((plot, Vec::new(), series, hover))
}
