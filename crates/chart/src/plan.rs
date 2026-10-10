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
    model::{Channel, Chart, Kind, Mark, Scale, Sort},
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

/// One colour's marks. `name` is its value of the colour field.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub name: Option<SharedString>,
    pub marks: Marks,
}

/// What is under the pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub title: SharedString,
    /// Series and its value there.
    pub rows: Vec<(usize, SharedString)>,
    /// Series and the point of it read out.
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
    chart: Chart,
    hover: Hover,
}

#[derive(Clone, Debug)]
enum Hover {
    /// Per series, ascending in x: each point's position and row.
    Along {
        x: Channel,
        y: Channel,
        points: Vec<Vec<([f32; 2], u32)>>,
    },
    /// `values[band * series + s]`, `NaN` for none.
    Bands {
        band: Band,
        vertical: bool,
        across: (f32, f32),
        labels: Vec<SharedString>,
        values: Vec<f64>,
        series: usize,
    },
    Slices {
        slices: Vec<(usize, Slice, f64)>,
        total: f64,
    },
}

/// `chart` laid out at `size`, its labels `line` high and as wide as `measure`
/// says. `None` for an encoding the mark cannot be drawn from: a field the
/// data does not hold, a quantitative field over text, a colour that is not
/// discrete, or a size too small to hold a plot.
pub fn plan(
    chart: &Chart,
    size: [f32; 2],
    line: f32,
    measure: &mut dyn FnMut(&str) -> f32,
) -> Option<Plan> {
    let groups = Groups::of(chart)?;
    let mut layout = Layout {
        size,
        line,
        top: PAD,
        reserved: 0.0,
        measure,
    };
    let title = chart.title.clone().map(|text| layout.line_at(text, PAD));
    let color_title = chart.encoding.color.as_ref().and_then(|c| c.title.clone());
    let (legend_title, legend) = layout.legend(color_title, &groups.names);
    let axis_title = |channel: &Option<Channel>| match chart.mark {
        Mark::Arc => None,
        _ => channel.as_ref()?.title.clone(),
    };
    let x_title = axis_title(&chart.encoding.x);
    if x_title.is_some() {
        layout.reserved = line + GAP;
    }
    let y_title = axis_title(&chart.encoding.y).map(|text| layout.line_at(text, PAD));
    let (plot, mut axes, marks, hover) = match chart.mark {
        Mark::Arc => arcs(chart, &groups, &layout)?,
        Mark::Bar => bars(chart, &groups, &mut layout)?,
        Mark::Line | Mark::Area | Mark::Point => along(chart, &groups, &mut layout)?,
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
    let series = groups
        .names
        .into_iter()
        .zip(marks)
        .map(|(name, marks)| Series { name, marks })
        .collect();
    Some(Plan {
        size,
        plot,
        title,
        legend_title,
        legend,
        axes,
        series,
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
        match &self.hover {
            Hover::Along { x, y, points } => {
                if !self.plot.contains(point) {
                    return None;
                }
                let at = points
                    .iter()
                    .filter_map(|series| nearest(series, point[0]))
                    .min_by(|a, b| {
                        (a.0[0] - point[0])
                            .abs()
                            .total_cmp(&(b.0[0] - point[0]).abs())
                    })?
                    .0[0];
                let hits: Vec<(usize, [f32; 2], u32)> = points
                    .iter()
                    .enumerate()
                    .filter_map(|(s, series)| {
                        let (p, row) = nearest(series, at)?;
                        ((p[0] - at).abs() <= SAME_X).then_some((s, *p, *row))
                    })
                    .collect();
                let first = hits.first()?.2 as usize;
                Some(Tip {
                    title: self.text(x, first)?,
                    rows: hits
                        .iter()
                        .filter_map(|&(s, _, row)| Some((s, self.text(y, row as usize)?)))
                        .collect(),
                    points: hits.iter().map(|&(s, p, _)| (s, p)).collect(),
                    band: None,
                })
            }
            Hover::Bands {
                band,
                vertical,
                across,
                labels,
                values,
                series,
            } => {
                if !self.plot.contains(point) {
                    return None;
                }
                let index = band.index(if *vertical { point[0] } else { point[1] })?;
                let (start, extent) = (band.range.0 + band.step() * index as f32, band.step());
                let rows = (0..*series)
                    .filter_map(|s| {
                        let value = values[index * series + s];
                        (!value.is_nan()).then(|| (s, scale::value(value).into()))
                    })
                    .collect();
                Some(Tip {
                    title: labels[index].clone(),
                    rows,
                    points: Vec::new(),
                    band: Some(match vertical {
                        true => Rect {
                            x: start,
                            y: across.0,
                            w: extent,
                            h: across.1 - across.0,
                        },
                        false => Rect {
                            x: across.0,
                            y: start,
                            w: across.1 - across.0,
                            h: extent,
                        },
                    }),
                })
            }
            Hover::Slices { slices, total } => {
                let (center, radius) = slices.first().map(|(_, s, _)| (s.center, s.radius))?;
                let (dx, dy) = (point[0] - center[0], point[1] - center[1]);
                if dx.hypot(dy) > radius {
                    return None;
                }
                let angle = dx.atan2(-dy).rem_euclid(TAU);
                let (s, _, value) = slices
                    .iter()
                    .find(|(_, slice, _)| angle >= slice.start && angle < slice.end)?;
                let share = value / total * 100.0;
                Some(Tip {
                    title: self.series[*s].name.clone().unwrap_or_default(),
                    rows: vec![(*s, format!("{} ({share:.1}%)", scale::value(*value)).into())],
                    points: Vec::new(),
                    band: None,
                })
            }
        }
    }

    /// Row `row` of `channel`'s field, read as its kind.
    fn text(&self, channel: &Channel, row: usize) -> Option<SharedString> {
        Some(match self.chart.data.column(&channel.field)? {
            Column::Text(text) => text.get(row).clone(),
            Column::Number(values) => match channel.kind {
                Kind::Temporal => time::describe(values[row]).into(),
                _ => scale::value(values[row]).into(),
            },
        })
    }
}

/// The point of `series`, ascending in x, nearest `at` along x.
fn nearest(series: &[([f32; 2], u32)], at: f32) -> Option<&([f32; 2], u32)> {
    let index = series.partition_point(|(p, _)| p[0] < at);
    [index.checked_sub(1), Some(index)]
        .into_iter()
        .flatten()
        .filter_map(|index| series.get(index))
        .min_by(|a, b| (a.0[0] - at).abs().total_cmp(&(b.0[0] - at).abs()))
}

/// The series the colour field splits rows into.
struct Groups {
    names: Vec<Option<SharedString>>,
    codes: Option<Arc<[u32]>>,
}

impl Groups {
    fn of(chart: &Chart) -> Option<Self> {
        let Some(color) = &chart.encoding.color else {
            return Some(Self {
                names: vec![None],
                codes: None,
            });
        };
        if !discrete(color.kind) {
            return None;
        }
        let domain = Discrete::of(chart.data.column(&color.field)?, &color.sort);
        Some(Self {
            names: domain.labels.into_iter().map(Some).collect(),
            codes: Some(domain.codes),
        })
    }

    fn of_row(&self, row: usize) -> Option<usize> {
        match &self.codes {
            None => Some(0),
            Some(codes) => (codes[row] != MISSING).then_some(codes[row] as usize),
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

fn numbers<'a>(chart: &'a Chart, channel: &Channel) -> Option<&'a Arc<[f64]>> {
    match chart.data.column(&channel.field)? {
        Column::Number(values) => Some(values),
        Column::Text(_) => None,
    }
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

type Body = (Rect, Vec<Axis>, Vec<Marks>, Hover);

fn bars(chart: &Chart, groups: &Groups, layout: &mut Layout) -> Option<Body> {
    let (x, y) = (chart.encoding.x.as_ref()?, chart.encoding.y.as_ref()?);
    let (category, value, vertical) = match (discrete(x.kind), discrete(y.kind)) {
        (true, false) if y.kind == Kind::Quantitative => (x, y, true),
        (false, true) if x.kind == Kind::Quantitative => (y, x, false),
        _ => return None,
    };
    let bands = Discrete::of(chart.data.column(&category.field)?, &category.sort);
    let values = numbers(chart, value)?;
    let (count, series) = (bands.labels.len(), groups.len());

    let mut sums = vec![f64::NAN; count * series];
    for (row, &v) in values.iter().enumerate() {
        let (code, Some(s)) = (bands.codes[row], groups.of_row(row)) else {
            continue;
        };
        if code == MISSING || v.is_nan() {
            continue;
        }
        let sum = &mut sums[code as usize * series + s];
        *sum = if sum.is_nan() { v } else { *sum + v };
    }
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for stack in sums.chunks(series.max(1)) {
        let up: f64 = stack.iter().filter(|v| **v > 0.0).sum();
        let down: f64 = stack.iter().filter(|v| **v < 0.0).sum();
        (lo, hi) = (lo.min(down), hi.max(up));
    }

    let (plot, axes, band, scale) = if vertical {
        let count = (layout.plot_height() / Y_SPACING).floor().max(2.0) as usize;
        let ticks = continuous(value, lo, hi, count);
        let labels = layout.measured(&ticks.labels);
        let plot = layout.plot(widest(&labels))?;
        let scale = Linear {
            domain: ticks.domain,
            range: (plot.bottom(), plot.y),
        };
        let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
        let band = Band {
            count: bands.labels.len(),
            range: (plot.x, plot.right()),
            padding: BAND_PADDING,
        };
        let centers: Vec<f32> = (0..band.count).map(|b| band.center(b)).collect();
        let axes = vec![
            layout.left(labels, &at, plot, true),
            layout.bottom(&bands.labels, &centers, plot, false),
        ];
        (plot, axes, band, scale)
    } else {
        let labels = layout.measured(&bands.labels);
        let plot = layout.plot(widest(&labels))?;
        let band = Band {
            count,
            range: (plot.y, plot.bottom()),
            padding: BAND_PADDING,
        };
        let centers: Vec<f32> = (0..count).map(|b| band.center(b)).collect();
        let left = layout.left(labels, &centers, plot, false);
        let count = (plot.w / X_SPACING).floor().max(2.0) as usize;
        let ticks = continuous(value, lo, hi, count);
        let scale = Linear {
            domain: ticks.domain,
            range: (plot.x, plot.right()),
        };
        let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
        let axes = vec![layout.bottom(&ticks.labels, &at, plot, true), left];
        (plot, axes, band, scale)
    };

    let mut marks = vec![Vec::new(); series];
    for (b, stack) in sums.chunks(series.max(1)).enumerate() {
        let (mut up, mut down) = (0.0, 0.0);
        for (s, &v) in stack.iter().enumerate() {
            if v.is_nan() || v == 0.0 {
                continue;
            }
            let base = if v > 0.0 { &mut up } else { &mut down };
            let (from, to) = (scale.at(*base), scale.at(*base + v));
            *base += v;
            let (start, extent) = (band.start(b), band.width());
            marks[s].push(match vertical {
                true => Rect {
                    x: start,
                    y: from.min(to),
                    w: extent,
                    h: (to - from).abs(),
                },
                false => Rect {
                    x: from.min(to),
                    y: start,
                    w: (to - from).abs(),
                    h: extent,
                },
            });
        }
    }
    let across = match vertical {
        true => (plot.y, plot.bottom()),
        false => (plot.x, plot.right()),
    };
    let hover = Hover::Bands {
        band,
        vertical,
        across,
        labels: bands.labels,
        values: sums,
        series,
    };
    Some((
        plot,
        axes,
        marks.into_iter().map(Marks::Bars).collect(),
        hover,
    ))
}

fn along(chart: &Chart, groups: &Groups, layout: &mut Layout) -> Option<Body> {
    let (x, y) = (chart.encoding.x.as_ref()?, chart.encoding.y.as_ref()?);
    if y.kind != Kind::Quantitative {
        return None;
    }
    let ys = numbers(chart, y)?;
    let x_column = chart.data.column(&x.field)?;
    let xs = match (discrete(x.kind), x_column) {
        (true, column) => Err(Discrete::of(column, &x.sort)),
        (false, Column::Number(values)) => Ok(values),
        (false, Column::Text(_)) => return None,
    };
    let valid = |row: usize| {
        let x_valid = match &xs {
            Ok(values) => !values[row].is_nan(),
            Err(domain) => domain.codes[row] != MISSING,
        };
        x_valid && groups.of_row(row).is_some()
    };

    let (lo, hi) = (0..chart.data.len())
        .filter(|&row| valid(row) && !ys[row].is_nan())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), row| {
            (lo.min(ys[row]), hi.max(ys[row]))
        });
    let (lo, hi) = if lo > hi { (0.0, 0.0) } else { (lo, hi) };
    let count = (layout.plot_height() / Y_SPACING).floor().max(2.0) as usize;
    let y_ticks = continuous(y, lo, hi, count);
    let labels = layout.measured(&y_ticks.labels);
    let plot = layout.plot(widest(&labels))?;
    let y_scale = Linear {
        domain: y_ticks.domain,
        range: (plot.bottom(), plot.y),
    };
    let y_at: Vec<f32> = y_ticks.values.iter().map(|&v| y_scale.at(v)).collect();

    let x_count = (plot.w / X_SPACING).floor().max(2.0) as usize;
    let (x_at, x_axis): (Box<dyn Fn(usize) -> f32>, Axis) = match &xs {
        Ok(values) => {
            let (lo, hi) = (0..chart.data.len())
                .filter(|&row| valid(row))
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), row| {
                    (lo.min(values[row]), hi.max(values[row]))
                });
            let (lo, hi) = if lo > hi { (0.0, 0.0) } else { (lo, hi) };
            let ticks = continuous(x, lo, hi, x_count);
            let scale = Linear {
                domain: ticks.domain,
                range: (plot.x, plot.right()),
            };
            let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
            let axis = layout.bottom(&ticks.labels, &at, plot, false);
            let values = Arc::clone(values);
            (Box::new(move |row| scale.at(values[row])), axis)
        }
        Err(domain) => {
            let band = Band {
                count: domain.labels.len(),
                range: (plot.x, plot.right()),
                padding: 0.0,
            };
            let centers: Vec<f32> = (0..band.count).map(|b| band.center(b)).collect();
            let axis = layout.bottom(&domain.labels, &centers, plot, false);
            let codes = domain.codes.clone();
            (Box::new(move |row| band.center(codes[row] as usize)), axis)
        }
    };

    let mut points: Vec<Vec<([f32; 2], u32)>> = vec![Vec::new(); groups.len()];
    for row in (0..chart.data.len()).filter(|&row| valid(row)) {
        let s = groups.of_row(row)?;
        let py = if ys[row].is_nan() {
            f32::NAN
        } else {
            y_scale.at(ys[row])
        };
        points[s].push(([x_at(row), py], row as u32));
    }
    for series in &mut points {
        if !series.is_sorted_by(|a, b| a.0[0] <= b.0[0]) {
            series.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]));
        }
    }

    let base = y_scale.at(0.0f64.clamp(y_ticks.domain.0, y_ticks.domain.1));
    let columns = plot.w.ceil() as usize;
    let marks = points
        .iter()
        .map(|series| {
            let runs = || {
                series
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
            match chart.mark {
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
                    Marks::Points(
                        series
                            .iter()
                            .filter(|(p, _)| !p[1].is_nan())
                            .filter(|(p, _)| {
                                series.len() <= columns
                                    || seen.insert((p[0].round() as i32, p[1].round() as i32))
                            })
                            .map(|(p, _)| *p)
                            .collect(),
                    )
                }
            }
        })
        .collect();

    for series in &mut points {
        series.retain(|(p, _)| !p[1].is_nan());
    }
    let axes = vec![layout.left(labels, &y_at, plot, true), x_axis];
    let hover = Hover::Along {
        x: x.clone(),
        y: y.clone(),
        points,
    };
    Some((plot, axes, marks, hover))
}

fn arcs(chart: &Chart, groups: &Groups, layout: &Layout) -> Option<Body> {
    let theta = chart.encoding.theta.as_ref()?;
    if theta.kind != Kind::Quantitative {
        return None;
    }
    let values = numbers(chart, theta)?;
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

    let parts: Vec<(usize, f64)> = match groups.codes {
        Some(_) => {
            let mut sums = vec![0.0; groups.len()];
            for (row, &v) in values.iter().enumerate() {
                if let Some(s) = groups.of_row(row)
                    && v > 0.0
                {
                    sums[s] += v;
                }
            }
            sums.into_iter()
                .enumerate()
                .filter(|(_, v)| *v > 0.0)
                .collect()
        }
        None => values
            .iter()
            .filter(|v| **v > 0.0)
            .map(|&v| (0, v))
            .collect(),
    };
    let total: f64 = parts.iter().map(|(_, v)| v).sum();
    let mut marks = vec![Vec::new(); groups.len()];
    let mut slices = Vec::new();
    let mut start = 0.0f64;
    for (s, value) in parts {
        let end = start + value / total;
        let slice = Slice {
            center,
            radius,
            start: start as f32 * TAU,
            end: end as f32 * TAU,
        };
        marks[s].push(slice);
        slices.push((s, slice, value));
        start = end;
    }
    let plot = Rect {
        x: center[0] - radius,
        y: center[1] - radius,
        w: radius * 2.0,
        h: radius * 2.0,
    };
    let hover = Hover::Slices { slices, total };
    Some((
        plot,
        Vec::new(),
        marks.into_iter().map(Marks::Slices).collect(),
        hover,
    ))
}
