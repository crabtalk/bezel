//! Bars, lines, areas, trails, points, rules, rects, ticks, texts and images
//! over an x and a y axis.

use std::{
    collections::{HashMap, HashSet},
    mem,
    sync::Arc,
};

use gpui::SharedString;
use js::curve::Curve;

use crate::{
    data::{Column, Data},
    decimate,
    model::{
        Align, Baseline, Channel, Chart, Encode, Extent, Kind, Layer, Mark, Orient, Overlay,
        Position, Properties,
    },
    plan::{
        self, Area, Bands, Bar, Body, Font, Hit, Hover, Image, Ink, Link, Marks, Rect, Series,
        Symbol, Text, Trace, arcs,
        domain::{self, Colors, Discrete},
        layout::{self, Layout},
        look::{self, Groups, Look},
    },
    scale::{self, Band, Linear},
};

/// A bar's corners with no corner property.
const BAR_RADIUS: f32 = 2.0;
/// A tick's thickness with no `thickness` property.
const TICK_THICKNESS: f32 = 1.0;
/// Of a band, what a tick spans with no size: Vega-Lite's tick band padding
/// taken out.
const TICK_BAND: f32 = 0.75;
/// A tick's length along a continuous axis with no size: Vega-Lite's step
/// less two.
const TICK_LENGTH: f32 = 18.0;

/// Where a layer's rows fall along the axis the layers share their domain on.
enum Along {
    /// Codes into the chart's discrete domain.
    Discrete(Vec<u32>),
    Continuous(Arc<[f64]>),
    /// Every row at one place, in pixels.
    Pixels(Position),
    /// The layer has no channel on the shared axis.
    Missing,
}

impl Along {
    fn valid(&self, row: usize) -> bool {
        match self {
            Along::Discrete(codes) => codes[row] != domain::MISSING,
            Along::Continuous(values) => !values[row].is_nan(),
            Along::Pixels(_) | Along::Missing => true,
        }
    }
}

/// Where a layer's rows end on a secondary channel: values on its
/// primary's scale, or one place in pixels.
enum Second {
    Values(Arc<[f64]>),
    Pixels(Position),
}

impl Second {
    fn of(encode: &Option<Encode<Position>>, data: &Data) -> Option<Option<Self>> {
        Some(match encode {
            None => None,
            Some(Encode::Scaled(channel)) => Some(Second::Values(channel.numbers(data)?)),
            Some(Encode::Value(position)) => Some(Second::Pixels(*position)),
        })
    }

    fn values(&self) -> Option<&Arc<[f64]>> {
        match self {
            Second::Values(values) => Some(values),
            Second::Pixels(_) => None,
        }
    }
}

/// A layer read for laying out.
struct Read<'a> {
    index: usize,
    layer: &'a Layer,
    along: Along,
    /// Where a ranged bar ends along the shared axis.
    along2: Option<Second>,
    /// `NaN` throughout for a layer with no channel on the value axis.
    values: Arc<[f64]>,
    /// Where a ranged bar or area ends along the value axis.
    values2: Option<Second>,
    value: Option<Channel>,
    /// Where every row stands on the value axis, for a position in pixels.
    value_pixels: Option<Position>,
    /// Slices, or text at them.
    polar: bool,
    /// Codes into the chart's offset domain.
    offsets: Option<Vec<u32>>,
    /// A fixed offset along x and y, in pixels.
    shift: [f32; 2],
    /// The offset of a mark's far end along x and y: its `x2Offset` and
    /// `y2Offset` where it has an `x2` or `y2`, else `shift`.
    shift2: [f32; 2],
    groups: Groups,
    ranks: Option<Vec<u32>>,
    /// Bars' totals: `sums[cell * groups + group]`, a cell being a band
    /// and offset, `NaN` for none, with each total's first row.
    sums: Vec<(f64, usize)>,
}

fn scaled(encode: &Option<Encode<Position>>) -> Option<&Channel> {
    encode.as_ref()?.channel()
}

fn pixels(encode: &Option<Encode<Position>>) -> Option<Position> {
    match encode {
        Some(Encode::Value(position)) => Some(*position),
        _ => None,
    }
}

/// A fixed pixel offset, 0 for none, `None` for a field on an axis that has
/// no bands to offset within.
fn shift(encode: &Option<Encode<f32>>) -> Option<f32> {
    match encode {
        None => Some(0.0),
        Some(Encode::Value(pixels)) => Some(*pixels),
        Some(Encode::Scaled(_)) => None,
    }
}

/// Which end of a bar its value reaches, on screen.
#[derive(Clone, Copy)]
enum End {
    Top,
    Bottom,
    Left,
    Right,
    /// A ranged bar, whose every corner caps a value.
    Both,
    /// A bar inside a stack, which caps none.
    Neither,
}

impl Bar {
    /// `rect` with the corners `properties` give a `mark`, its end at `end`.
    fn of(rect: Rect, mark: Mark, properties: &Properties, end: End) -> Self {
        let unset = properties.corner_radius.iter().all(Option::is_none)
            && properties.corner_radius_end.is_none();
        let default = match (mark, unset) {
            (Mark::Bar, true) => BAR_RADIUS,
            _ => 0.0,
        };
        let mut corners = properties.corner_radius.map(|r| r.unwrap_or(default));
        if let Some(radius) = properties.corner_radius_end {
            let ends: &[usize] = match end {
                End::Top => &[0, 1],
                End::Right => &[1, 2],
                End::Bottom => &[2, 3],
                End::Left => &[0, 3],
                End::Both => &[0, 1, 2, 3],
                End::Neither => &[],
            };
            for &corner in ends {
                corners[corner] = radius;
            }
        }
        let most = rect.w.min(rect.h).max(0.0) / 2.0;
        Bar {
            rect,
            corners: corners.map(|r| r.clamp(0.0, most)),
        }
    }
}

pub(super) fn cartesian(chart: &Chart, colors: &Colors, layout: &mut Layout) -> Option<Body> {
    // The layers that place both x and y set the axes; y is the shared axis
    // when each has a discrete y over a continuous x.
    let placed: Vec<&Layer> = chart
        .layers
        .iter()
        .filter(|layer| scaled(&layer.encoding.x).is_some() && scaled(&layer.encoding.y).is_some())
        .collect();
    let first = &placed.first()?.encoding;
    let horizontal = placed.iter().all(|layer| {
        let encoding = &layer.encoding;
        scaled(&encoding.y).is_some_and(|y| y.kind.is_discrete())
            && scaled(&encoding.x).is_some_and(|x| !x.kind.is_discrete())
    });
    let (shared, value) = match horizontal {
        true => (scaled(&first.y)?, scaled(&first.x)?),
        false => (scaled(&first.x)?, scaled(&first.y)?),
    };
    let is_discrete = shared.kind.is_discrete();

    let mut labels: Vec<SharedString> = Vec::new();
    let mut index: HashMap<SharedString, u32> = HashMap::new();
    let mut offset_labels: Vec<SharedString> = Vec::new();
    let mut offset_index: HashMap<SharedString, u32> = HashMap::new();
    let mut read = Vec::with_capacity(chart.layers.len());
    for (layer_index, layer) in chart.layers.iter().enumerate() {
        let e = &layer.encoding;
        let mark = layer.mark;
        let (along_e, along2_e, value_e, value2_e, offset) = match horizontal {
            true => (&e.y, &e.y2, &e.x, &e.x2, &e.y_offset),
            false => (&e.x, &e.x2, &e.y, &e.y2, &e.x_offset),
        };
        let (along, value) = (scaled(along_e), scaled(value_e));
        let (along_pixels, value_pixels) = (pixels(along_e), pixels(value_e));
        let has_along = along.is_some() || along_pixels.is_some();
        let has_value = value.is_some() || value_pixels.is_some();
        // Slices, and text at them, stand at the plot's middle without x
        // or y.
        let polar = mark == Mark::Arc || (mark == Mark::Text && e.theta.is_some());
        let partial = matches!(
            mark,
            Mark::Rule
                | Mark::Rect
                | Mark::Text
                | Mark::Tick
                | Mark::Point
                | Mark::Circle
                | Mark::Square
                | Mark::Image
        );
        if !polar && (!has_along || !has_value) && (!partial || (!has_along && !has_value)) {
            return None;
        }
        let ranged = value2_e.is_some();
        let mut shift = match horizontal {
            true => [shift(&e.x_offset)?, 0.0],
            false => [0.0, shift(&e.y_offset)?],
        };
        if along.is_some_and(|along| along.kind.is_discrete() != is_discrete)
            || value.is_some_and(|value| value.kind.is_discrete())
            || (along2_e.is_some() && !matches!(mark, Mark::Bar | Mark::Rule | Mark::Rect))
            || (ranged && !matches!(mark, Mark::Bar | Mark::Area | Mark::Rule | Mark::Rect))
            || (!has_along && along2_e.is_some())
            || (along.is_some() && is_discrete && along2_e.is_some())
            || (!has_value && ranged)
            || (value.is_some_and(|v| v.kind == Kind::Temporal) && mark == Mark::Bar && !ranged)
            || (mark == Mark::Bar && !is_discrete && along2_e.is_none())
            || (mark == Mark::Bar && along_pixels.is_some() && along2_e.is_none())
            || (mark == Mark::Rect
                && !((along.is_none() || is_discrete || along2_e.is_some())
                    && (value.is_none() || ranged)))
        {
            return None;
        }
        let along = match (along, along_pixels) {
            (Some(along), _) => match (is_discrete, along.column(&layer.data)?) {
                (true, column) => Along::Discrete(
                    Discrete::of(&column, &along.sort).into_shared(&mut labels, &mut index),
                ),
                (false, Column::Number(values)) => Along::Continuous(values),
                (false, Column::Text(_)) => return None,
            },
            (None, Some(position)) => Along::Pixels(position),
            (None, None) => Along::Missing,
        };
        let offsets = match offset {
            None => None,
            Some(Encode::Value(pixels)) => {
                shift[usize::from(horizontal)] += *pixels;
                None
            }
            Some(Encode::Scaled(channel)) => {
                if !is_discrete
                    || !channel.kind.is_discrete()
                    || !matches!(along, Along::Discrete(_))
                {
                    return None;
                }
                let domain = Discrete::of(&channel.column(&layer.data)?, &channel.sort);
                Some(domain.into_shared(&mut offset_labels, &mut offset_index))
            }
        };
        let p = &layer.properties;
        let shift2 = [
            e.x2.as_ref().map_or(shift[0], |_| p.x2_offset),
            e.y2.as_ref().map_or(shift[1], |_| p.y2_offset),
        ];
        let values = match value {
            Some(value) => value.numbers(&layer.data)?,
            None => vec![f64::NAN; layer.data.len()].into(),
        };
        read.push(Read {
            index: layer_index,
            layer,
            along,
            along2: Second::of(along2_e, &layer.data)?,
            values,
            values2: Second::of(value2_e, &layer.data)?,
            value: value.cloned(),
            value_pixels,
            polar,
            offsets,
            shift,
            shift2,
            groups: Groups::of(layer, layer_index, colors)?,
            ranks: look::ranks(layer)?,
            sums: Vec::new(),
        });
    }

    let count = labels.len();
    let offsets = offset_labels.len().max(1);
    let mut extent = Vec::new();
    for layer in &mut read {
        let rows = (0..layer.values.len())
            .filter(|&row| layer.along.valid(row) && layer.groups.codes[row] != domain::MISSING);
        if layer.layer.mark != Mark::Bar || layer.values2.is_some() || layer.value_pixels.is_some()
        {
            let values2 = layer.values2.as_ref().and_then(Second::values);
            for row in rows {
                extent.push(layer.values[row]);
                if let Some(values2) = values2 {
                    extent.push(values2[row]);
                }
            }
            continue;
        }
        let Along::Discrete(codes) = &layer.along else {
            return None;
        };
        let groups = layer.groups.len().max(1);
        let mut sums = vec![(f64::NAN, 0); count * offsets * groups];
        for row in rows {
            let v = layer.values[row];
            if v.is_nan() {
                continue;
            }
            let offset = layer.offsets.as_ref().map_or(0, |codes| codes[row]);
            if offset == domain::MISSING {
                continue;
            }
            let cell = codes[row] as usize * offsets + offset as usize;
            let (sum, first) = &mut sums[cell * groups + layer.groups.codes[row] as usize];
            if sum.is_nan() {
                (*sum, *first) = (v, row);
            } else {
                *sum += v;
            }
        }
        for stack in sums.chunks(groups) {
            let up: f64 = stack.iter().map(|(v, _)| *v).filter(|v| *v > 0.0).sum();
            let down: f64 = stack.iter().map(|(v, _)| *v).filter(|v| *v < 0.0).sum();
            extent.extend([up, down, 0.0]);
        }
        layer.sums = sums;
    }
    let (lo, hi) = domain::extent(extent);

    let barred = read.iter().any(|layer| layer.layer.mark == Mark::Bar);
    let padding = match barred && is_discrete {
        true => plan::BAND_PADDING,
        false => 0.0,
    };
    let (plot, axes, value_scale, along_scale) = if horizontal {
        let measured = layout.measured(&labels);
        let plot = layout.plot(layout::widest(&measured));
        let band = Band {
            count,
            range: (plot.y, plot.bottom()),
            padding,
        };
        let centers: Vec<f32> = (0..count).map(|b| band.center(b)).collect();
        let left = layout.left(measured, &centers, plot, false);
        let ticks = domain::continuous(value, lo, hi, x_count(plot));
        let scale = Linear {
            domain: ticks.domain,
            range: (plot.x, plot.right()),
        };
        let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
        let axes = vec![layout.bottom(&ticks.labels, &at, plot, true), left];
        (plot, axes, scale, None)
    } else {
        let count_y = (layout.plot_height() / plan::Y_SPACING).floor().max(2.0) as usize;
        let ticks = domain::continuous(value, lo, hi, count_y);
        let measured = layout.measured(&ticks.labels);
        let plot = layout.plot(layout::widest(&measured));
        let scale = Linear {
            domain: ticks.domain,
            range: (plot.bottom(), plot.y),
        };
        let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
        let left = layout.left(measured, &at, plot, true);
        let (bottom, along_scale) = if is_discrete {
            let band = Band {
                count,
                range: (plot.x, plot.right()),
                padding,
            };
            let centers: Vec<f32> = (0..count).map(|b| band.center(b)).collect();
            (layout.bottom(&labels, &centers, plot, false), None)
        } else {
            let (lo, hi) = domain::extent(read.iter().flat_map(|layer| {
                let Along::Continuous(values) = &layer.along else {
                    return Vec::new();
                };
                let along2 = layer.along2.as_ref().and_then(Second::values);
                values
                    .iter()
                    .chain(along2.iter().flat_map(|v| v.iter()))
                    .copied()
                    .collect()
            }));
            let ticks = domain::continuous(shared, lo, hi, x_count(plot));
            let scale = Linear {
                domain: ticks.domain,
                range: (plot.x, plot.right()),
            };
            let at: Vec<f32> = ticks.values.iter().map(|&v| scale.at(v)).collect();
            (layout.bottom(&ticks.labels, &at, plot, false), Some(scale))
        };
        (plot, vec![left, bottom], scale, along_scale)
    };

    // The plot's extent along the shared axis, and up the value axis.
    let (along_range, value_range) = match horizontal {
        true => ((plot.y, plot.bottom()), (plot.x, plot.right())),
        false => ((plot.x, plot.right()), (plot.bottom(), plot.y)),
    };
    let along_mid = (along_range.0 + along_range.1) / 2.0;
    let value_mid = (value_range.0 + value_range.1) / 2.0;
    let band = Band {
        count,
        range: along_range,
        padding,
    };
    // The filled part of band `b` at offset `o`: its start and width.
    let cell = |b: usize, o: u32| -> (f32, f32) {
        match offset_labels.is_empty() {
            true => (band.start(b), band.width()),
            false => {
                let sub = Band {
                    count: offset_labels.len(),
                    range: (band.start(b), band.start(b) + band.width()),
                    padding: 0.0,
                };
                (sub.start(o as usize), sub.width())
            }
        }
    };
    let cell_width = match offset_labels.is_empty() {
        true => band.width(),
        false => band.width() / offset_labels.len() as f32,
    };
    // The cell a row falls in along the shared axis: its start and width,
    // `None` off the domain.
    let cell_of = |layer: &Read, row: usize| -> Option<(f32, f32)> {
        let Along::Discrete(codes) = &layer.along else {
            return None;
        };
        let offset = layer.offsets.as_ref().map_or(0, |codes| codes[row]);
        if codes[row] == domain::MISSING || offset == domain::MISSING {
            return None;
        }
        Some(cell(codes[row] as usize, offset))
    };
    // Which of x and y the shared axis and the value axis are.
    let (along_axis, value_axis) = match horizontal {
        true => (1, 0),
        false => (0, 1),
    };
    // A position in pixels on x or y, on screen.
    let pixel = |position: Position, axis: usize| match axis {
        0 => plot.x + position.pixels(plot.w),
        _ => plot.y + position.pixels(plot.h),
    };
    let place = |layer: &Read, row: usize| -> f32 {
        match (&layer.along, along_scale) {
            (Along::Discrete(_), _) => {
                cell_of(layer, row).map_or(f32::NAN, |(start, width)| start + width / 2.0)
            }
            (Along::Continuous(values), Some(scale)) => scale.at(values[row]),
            (Along::Continuous(_), None) => f32::NAN,
            (Along::Pixels(position), _) => pixel(*position, along_axis),
            (Along::Missing, _) => along_mid,
        }
    };
    // Where a row's `x2` or `y2` puts it along the shared axis, `NaN` for a
    // missing value; `None` without one.
    let place2 = |layer: &Read, row: usize| -> Option<f32> {
        Some(match (layer.along2.as_ref()?, along_scale) {
            (Second::Values(values), Some(scale)) if !values[row].is_nan() => scale.at(values[row]),
            (Second::Values(_), _) => f32::NAN,
            (Second::Pixels(position), _) => pixel(*position, along_axis),
        })
    };
    let at = |v: f64| match v.is_nan() {
        true => f32::NAN,
        false => value_scale.at(v),
    };
    // Where a row stands up the values, `NaN` for a missing value; `None`
    // for a layer without a value position.
    let value_at = |layer: &Read, row: usize| -> Option<f32> {
        match (&layer.value, layer.value_pixels) {
            (Some(_), _) => Some(at(layer.values[row])),
            (None, Some(position)) => Some(pixel(position, value_axis)),
            (None, None) => None,
        }
    };
    let value2_at = |layer: &Read, row: usize| -> Option<f32> {
        Some(match layer.values2.as_ref()? {
            Second::Values(values) => at(values[row]),
            Second::Pixels(position) => pixel(*position, value_axis),
        })
    };
    let base = value_scale.at(0.0f64.clamp(
        value_scale.domain.0.min(value_scale.domain.1),
        value_scale.domain.0.max(value_scale.domain.1),
    ));
    // Where the shared axis starts: zero, or its least, on a continuous
    // scale; the left or bottom edge on a discrete one.
    let along_base = match along_scale {
        Some(scale) => scale.at(0.0f64.clamp(
            scale.domain.0.min(scale.domain.1),
            scale.domain.0.max(scale.domain.1),
        )),
        None if horizontal => plot.bottom(),
        None => plot.x,
    };
    // Where along and up the values a point is, on screen.
    let point = |along: f32, value: f32| match horizontal {
        true => [value, along],
        false => [along, value],
    };
    // A rect from `along.0` to `along.1` across and `from` to `to` up the
    // values.
    let rect = |along: (f32, f32), from: f32, to: f32| match horizontal {
        false => Rect {
            x: along.0.min(along.1),
            y: from.min(to),
            w: (along.1 - along.0).abs(),
            h: (to - from).abs(),
        },
        true => Rect {
            x: from.min(to),
            y: along.0.min(along.1),
            w: (to - from).abs(),
            h: (along.1 - along.0).abs(),
        },
    };
    // `rect` for a layer, `along.0` and `to` its primary positions moved by
    // its offsets and the others by its far ends'.
    let placed = |layer: &Read, along: (f32, f32), from: f32, to: f32| {
        let (shift, shift2) = (layer.shift, layer.shift2);
        rect(
            (along.0 + shift[along_axis], along.1 + shift2[along_axis]),
            from + shift2[value_axis],
            to + shift[value_axis],
        )
    };
    // The end of a bar from `from` to `to` up the values.
    let end = |from: f32, to: f32| match (horizontal, to < from) {
        (false, true) => End::Top,
        (false, false) => End::Bottom,
        (true, true) => End::Left,
        (true, false) => End::Right,
    };
    let columns = plot.w.ceil() as usize;
    let mut series = Vec::new();
    let mut hover = Hover::default();
    let mut links = Vec::new();
    // A pie with no `radius`: Vega-Lite's half the plot's shorter side.
    let outer = plot.w.min(plot.h) / 2.0;
    for layer in &read {
        let mark = layer.layer.mark;
        let properties = &layer.layer.properties;
        if layer.polar {
            // Each row's pie centred where its x and y place it.
            let center = |row: usize| {
                if !layer.along.valid(row) {
                    return None;
                }
                let value = value_at(layer, row).unwrap_or(value_mid);
                let [x, y] = point(place(layer, row), value);
                let [dx, dy] = layer.shift;
                (!x.is_nan() && !y.is_nan()).then_some([x + dx, y + dy])
            };
            arcs::polar(
                layer.layer,
                layer.index,
                colors,
                &center,
                outer,
                plot,
                (&mut series, &mut hover, &mut links),
            )?;
            continue;
        }
        let look = Look::of(layer.layer, band.step() / offsets as f32)?;
        let groups = &layer.groups;
        let ink = |group: usize| colors.colors[groups.groups[group].0].ink;
        let style = |group: usize| look.style(groups.groups[group].1, ink(group));
        let curve = properties.interpolate.curve(
            properties.tension,
            properties
                .orient
                .map_or(horizontal, |o| o == Orient::Horizontal),
        );
        let new_series = |group: usize, marks: Marks| Series {
            name: colors.colors[groups.groups[group].0].name.clone(),
            color: groups.groups[group].0,
            mark,
            marks,
            style: style(group),
            curve,
            clip: properties.clip.then_some(plot),
        };
        let valid = |row: usize| groups.codes[row] != domain::MISSING && layer.along.valid(row);
        // The extent across its band a band-relative width takes.
        let across = match horizontal {
            true => properties.height,
            false => properties.width,
        };

        if mark == Mark::Bar {
            let mut bars: Vec<Vec<Bar>> = vec![Vec::new(); groups.len()];
            let mut readouts = vec![vec![None; count]; groups.len()];
            // The bar's thickness from `size`, centred in its cell, or the
            // cell's width.
            let thick = |start: f32, width: f32, row: usize| {
                let size = look.size.at(row);
                let size = match (size.is_nan(), across) {
                    (false, _) => size,
                    (true, Some(extent)) => extent.of(width),
                    (true, None) => return (start, start + width),
                };
                let center = start + width / 2.0;
                (center - size / 2.0, center + size / 2.0)
            };
            let push = |bars: &mut Vec<Vec<Bar>>,
                        bar: Bar,
                        group: usize,
                        row: usize,
                        links: &mut Vec<Link>| {
                if let Some(href) = look.href(row) {
                    links.push(Link {
                        hit: Hit::Rect(bar.rect),
                        href,
                    });
                }
                bars[group].push(bar);
                (group, bars[group].len() - 1)
            };
            if layer.values2.is_some() || layer.value_pixels.is_some() {
                // Each row from its value to its `x2` or `y2`, or to the
                // base for a value in pixels.
                for row in 0..layer.values.len() {
                    let group = groups.codes[row];
                    if !valid(row) {
                        continue;
                    }
                    let (Some(v), v2) = (value_at(layer, row), value2_at(layer, row)) else {
                        continue;
                    };
                    let (far, end) = match v2 {
                        Some(v2) => (v2, End::Both),
                        None => (base, end(base, v)),
                    };
                    if v.is_nan() || far.is_nan() {
                        continue;
                    }
                    let along = match &layer.along {
                        Along::Discrete(codes) => {
                            let Some((start, width)) = cell_of(layer, row) else {
                                continue;
                            };
                            let values2 = layer.values2.as_ref().and_then(Second::values);
                            if let (Some(channel), Some(values2)) = (&layer.value, values2) {
                                let kind = channel.kind;
                                let (v, v2) = (layer.values[row], values2[row]);
                                readouts[group as usize][codes[row] as usize] = Some(
                                    format!("{} – {}", kind.describe(v), kind.describe(v2)).into(),
                                );
                            }
                            thick(start, width, row)
                        }
                        _ => match place2(layer, row) {
                            Some(along2) => (place(layer, row), along2),
                            None => continue,
                        },
                    };
                    if along.0.is_nan() || along.1.is_nan() {
                        continue;
                    }
                    let bar = Bar::of(placed(layer, along, far, v), mark, properties, end);
                    push(&mut bars, bar, group as usize, row, &mut links);
                }
            } else if layer.along2.is_some() {
                for row in 0..layer.values.len() {
                    let group = groups.codes[row];
                    let v = at(layer.values[row]);
                    let along = (place(layer, row), place2(layer, row).unwrap_or(f32::NAN));
                    if group == domain::MISSING
                        || v.is_nan()
                        || along.0.is_nan()
                        || along.1.is_nan()
                    {
                        continue;
                    }
                    let bar = Bar::of(
                        placed(layer, along, base, v),
                        mark,
                        properties,
                        end(base, v),
                    );
                    push(&mut bars, bar, group as usize, row, &mut links);
                }
            } else {
                let mut stacking: Vec<usize> = (0..groups.len()).collect();
                if let Some(ranks) = &layer.ranks {
                    let mut first = vec![u32::MAX; groups.len()];
                    for (row, &group) in groups.codes.iter().enumerate() {
                        if group != domain::MISSING {
                            first[group as usize] = first[group as usize].min(ranks[row]);
                        }
                    }
                    stacking.sort_by_key(|&g| first[g]);
                }
                let mut totals = vec![vec![f64::NAN; count]; groups.len()];
                let width = groups.len().max(1);
                for (c, stack) in layer.sums.chunks(width).enumerate() {
                    let (b, o) = (c / offsets, (c % offsets) as u32);
                    let (mut up, mut down) = (0.0, 0.0);
                    // The last bar each way, and where it ends.
                    let mut outer: [Option<((usize, usize), End)>; 2] = [None, None];
                    for &group in &stacking {
                        let (v, row) = stack[group];
                        if v.is_nan() {
                            continue;
                        }
                        let total = &mut totals[group][b];
                        *total = if total.is_nan() { v } else { *total + v };
                        if v == 0.0 {
                            continue;
                        }
                        let stacked = if v > 0.0 { &mut up } else { &mut down };
                        let (from, to) = (value_scale.at(*stacked), value_scale.at(*stacked + v));
                        *stacked += v;
                        let (start, extent) = cell(b, o);
                        let rect = placed(layer, thick(start, extent, row), from, to);
                        // A stack takes its end corners at its ends alone.
                        let bar = Bar::of(rect, mark, properties, End::Neither);
                        outer[usize::from(v < 0.0)] =
                            Some((push(&mut bars, bar, group, row, &mut links), end(from, to)));
                    }
                    if properties.corner_radius_end.is_some() {
                        for ((group, index), end) in outer.into_iter().flatten() {
                            let rect = bars[group][index].rect;
                            bars[group][index] = Bar::of(rect, mark, properties, end);
                        }
                    }
                }
                for (readouts, totals) in readouts.iter_mut().zip(totals) {
                    for (readout, total) in readouts.iter_mut().zip(totals) {
                        if !total.is_nan() {
                            *readout = Some(scale::value(total).into());
                        }
                    }
                }
            }
            if is_discrete {
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
                for (group, readouts) in readouts.into_iter().enumerate() {
                    if readouts.iter().any(Option::is_some) {
                        bands.values.push((groups.groups[group].0, readouts));
                    }
                }
            }
            for (group, bars) in bars.into_iter().enumerate() {
                if !bars.is_empty() {
                    series.push(new_series(group, Marks::Bars(bars)));
                }
            }
            continue;
        }

        if matches!(mark, Mark::Rect | Mark::Tick | Mark::Rule) {
            let mut bars: Vec<Vec<Bar>> = vec![Vec::new(); groups.len()];
            let mut rules: Vec<Vec<[[f32; 2]; 2]>> = vec![Vec::new(); groups.len()];
            for row in 0..layer.values.len() {
                if !valid(row) {
                    continue;
                }
                let group = groups.codes[row] as usize;
                let value = value_at(layer, row);
                let has_value = value.is_some();
                let v = value.unwrap_or(f32::NAN);
                if has_value && v.is_nan() {
                    continue;
                }
                let along = place(layer, row);
                let along2 = place2(layer, row);
                let v2 = value2_at(layer, row);
                if along.is_nan() || along2.is_some_and(f32::is_nan) || v2.is_some_and(f32::is_nan)
                {
                    continue;
                }
                let hit = match mark {
                    Mark::Rule => {
                        let moved = |p: [f32; 2], by: [f32; 2]| [p[0] + by[0], p[1] + by[1]];
                        let (shift, shift2) = (layer.shift, layer.shift2);
                        let ends = match (has_value, matches!(layer.along, Along::Missing)) {
                            (false, _) => {
                                [point(along, value_range.0), point(along, value_range.1)]
                                    .map(|p| moved(p, shift))
                            }
                            (true, true) => [point(along_range.0, v), point(along_range.1, v)]
                                .map(|p| moved(p, shift)),
                            (true, false) => {
                                let far = match (along2, v2) {
                                    (None, None) => point(along, base),
                                    (a2, v2) => point(a2.unwrap_or(along), v2.unwrap_or(v)),
                                };
                                [moved(point(along, v), shift), moved(far, shift2)]
                            }
                        };
                        rules[group].push(ends);
                        Hit::Line(
                            ends.to_vec(),
                            look.style(row, ink(group)).stroke_width / 2.0 + plan::SLOP,
                        )
                    }
                    Mark::Rect => {
                        let span = match (&layer.along, along2) {
                            (Along::Missing, _) => along_range,
                            (_, Some(along2)) => (along, along2),
                            (Along::Pixels(_), None) => (along, along_base),
                            _ => {
                                let Some((start, width)) = cell_of(layer, row) else {
                                    continue;
                                };
                                match across {
                                    Some(extent) => {
                                        let size = extent.of(width);
                                        (along - size / 2.0, along + size / 2.0)
                                    }
                                    None => (start, start + width),
                                }
                            }
                        };
                        let (far, near) = match (v2, has_value) {
                            (Some(v2), _) => (v2, v),
                            (None, true) => (base, v),
                            (None, false) => value_range,
                        };
                        let bar =
                            Bar::of(placed(layer, span, far, near), mark, properties, End::Both);
                        bars[group].push(bar);
                        Hit::Rect(bar.rect)
                    }
                    _ => {
                        let thickness = properties.thickness.unwrap_or(TICK_THICKNESS);
                        let size = look.size.at(row);
                        let v = if has_value { v } else { value_mid };
                        let (span, across_values) = match &layer.along {
                            Along::Continuous(_) | Along::Pixels(_) => {
                                let length = if size.is_nan() { TICK_LENGTH } else { size };
                                (
                                    (along - thickness / 2.0, along + thickness / 2.0),
                                    (v - length / 2.0, v + length / 2.0),
                                )
                            }
                            _ => {
                                let room = match &layer.along {
                                    Along::Missing => (along_range.1 - along_range.0).abs(),
                                    _ => cell_width,
                                };
                                let room = match padding > 0.0 {
                                    true => room,
                                    false => room * TICK_BAND,
                                };
                                let length = match (size.is_nan(), across) {
                                    (false, _) => size,
                                    (true, Some(extent)) => extent.of(room),
                                    (true, None) => room,
                                };
                                (
                                    (along - length / 2.0, along + length / 2.0),
                                    (v - thickness / 2.0, v + thickness / 2.0),
                                )
                            }
                        };
                        let bar = Bar::of(
                            placed(layer, span, across_values.0, across_values.1),
                            mark,
                            properties,
                            End::Both,
                        );
                        bars[group].push(bar);
                        Hit::Rect(bar.rect)
                    }
                };
                if let Some(href) = look.href(row) {
                    links.push(Link { hit, href });
                }
            }
            for group in 0..groups.len() {
                let marks = match mark {
                    Mark::Rule => Marks::Rules(mem::take(&mut rules[group])),
                    _ => Marks::Bars(mem::take(&mut bars[group])),
                };
                let empty = match &marks {
                    Marks::Rules(rules) => rules.is_empty(),
                    Marks::Bars(bars) => bars.is_empty(),
                    _ => true,
                };
                if !empty {
                    series.push(new_series(group, marks));
                }
            }
            continue;
        }

        // Each series' points, each with its bottom for an area, and row.
        let mut points: Vec<Vec<([f32; 2], f32, u32)>> = vec![Vec::new(); groups.len()];
        let order: Vec<usize> = match &layer.ranks {
            Some(ranks) => {
                let mut rows: Vec<usize> = (0..layer.values.len()).collect();
                rows.sort_by_key(|&row| ranks[row]);
                rows
            }
            None => (0..layer.values.len()).collect(),
        };
        for row in order {
            if !valid(row) {
                continue;
            }
            let group = groups.codes[row];
            let along = place(layer, row);
            let value = value_at(layer, row).unwrap_or(value_mid);
            let bottom = value2_at(layer, row).unwrap_or(base) + layer.shift2[value_axis];
            let [dx, dy] = layer.shift;
            let at = point(along, value);
            points[group as usize].push(([at[0] + dx, at[1] + dy], bottom, row as u32));
        }
        // Overlays, drawn from this layer's data and channels under their
        // own.
        let overlay = |overlay: &Overlay, mark: Mark| -> Option<(Look, Mark, Curve)> {
            let mut encoding = overlay.encoding.over(&layer.layer.encoding);
            (encoding.x2, encoding.y2) = (None, None);
            let synthetic = Layer {
                data: layer.layer.data.clone(),
                mark,
                properties: overlay.properties.clone(),
                encoding,
            };
            let curve = overlay.properties.interpolate.curve(
                overlay.properties.tension,
                overlay
                    .properties
                    .orient
                    .map_or(horizontal, |o| o == Orient::Horizontal),
            );
            Some((Look::of(&synthetic, band.step())?, mark, curve))
        };
        let line_overlay = match &properties.line {
            Some(line) if mark == Mark::Area => Some(overlay(line, Mark::Line)?),
            _ => None,
        };
        let point_overlay = match &properties.point {
            Some(point) => Some(overlay(point, Mark::Point)?),
            None => None,
        };
        // Rows read out under the pointer place both positions on scales.
        let traced = layer.value.is_some()
            && matches!(layer.along, Along::Discrete(_) | Along::Continuous(_))
            && !matches!(mark, Mark::Text | Mark::Image);
        for (group, mut points) in points.into_iter().enumerate() {
            if points.is_empty() {
                continue;
            }
            let paths = matches!(mark, Mark::Line | Mark::Area | Mark::Trail);
            if layer.ranks.is_none()
                && paths
                && !points.is_sorted_by(|a, b| a.0[along_axis] <= b.0[along_axis])
            {
                points.sort_by(|a, b| a.0[along_axis].total_cmp(&b.0[along_axis]));
            }
            let first = groups.groups[group].1;
            let series_href = look.href(first);
            let runs = |points: &[([f32; 2], f32, u32)]| -> Vec<Vec<([f32; 2], f32, u32)>> {
                points
                    .split(|(p, bottom, _)| p[1].is_nan() || p[0].is_nan() || bottom.is_nan())
                    .filter(|run| !run.is_empty())
                    .map(<[_]>::to_vec)
                    .collect()
            };
            let thin = |run: Vec<[f32; 2]>| match run.len() > columns && layer.ranks.is_none() {
                true => decimate::m4(&run),
                false => run,
            };
            let tops = || -> Vec<Vec<[f32; 2]>> {
                runs(&points)
                    .into_iter()
                    .map(|run| thin(run.into_iter().map(|(p, ..)| p).collect()))
                    .collect()
            };
            let marks = match mark {
                Mark::Line => {
                    let lines = tops();
                    if let Some(href) = &series_href {
                        let reach = style(group).stroke_width / 2.0 + plan::SLOP;
                        links.extend(lines.iter().map(|run| Link {
                            hit: Hit::Line(run.clone(), reach),
                            href: href.clone(),
                        }));
                    }
                    Marks::Lines(lines)
                }
                Mark::Area => {
                    let areas: Vec<Area> = runs(&points)
                        .into_iter()
                        .map(|run| {
                            let top = thin(run.iter().map(|(p, ..)| *p).collect());
                            let corner = |p: [f32; 2]| match horizontal {
                                true => [base, p[1]],
                                false => [p[0], base],
                            };
                            let base = match &layer.values2 {
                                None => vec![corner(run[0].0), corner(run[run.len() - 1].0)],
                                Some(_) => run
                                    .iter()
                                    .map(|(p, bottom, _)| match horizontal {
                                        true => [*bottom, p[1]],
                                        false => [p[0], *bottom],
                                    })
                                    .collect(),
                            };
                            Area { top, base }
                        })
                        .collect();
                    if let Some(href) = &series_href {
                        links.extend(areas.iter().map(|area| Link {
                            hit: Hit::Polygon(area.outline()),
                            href: href.clone(),
                        }));
                    }
                    Marks::Areas(areas)
                }
                Mark::Trail => {
                    let trails: Vec<Vec<([f32; 2], f32)>> = runs(&points)
                        .into_iter()
                        .map(|run| {
                            run.into_iter()
                                .map(|(p, _, row)| (p, look.size.at(row as usize)))
                                .collect()
                        })
                        .collect();
                    if let Some(href) = &series_href {
                        links.extend(trails.iter().map(|run| {
                            let widest = run.iter().map(|(_, w)| *w).fold(0.0, f32::max);
                            Link {
                                hit: Hit::Line(
                                    run.iter().map(|(p, _)| *p).collect(),
                                    widest / 2.0 + plan::SLOP,
                                ),
                                href: href.clone(),
                            }
                        }));
                    }
                    Marks::Trails(trails)
                }
                Mark::Text => {
                    let font = Font::of(layer.layer);
                    let texts: Vec<Text> = points
                        .iter()
                        .filter(|(p, ..)| !p[0].is_nan() && !p[1].is_nan())
                        .filter_map(|&(at, _, row)| {
                            let row = row as usize;
                            let text = look.text.at(row);
                            if text.is_empty() {
                                return None;
                            }
                            let size = look.size.at(row);
                            let at = [at[0] + properties.dx, at[1] + properties.dy];
                            if let Some(href) = look.href(row) {
                                links.push(Link {
                                    hit: Hit::Disc(at, size),
                                    href,
                                });
                            }
                            Some(Text { at, text, size })
                        })
                        .collect();
                    Marks::Texts(texts, font)
                }
                Mark::Image => {
                    let images: Vec<Image> = points
                        .iter()
                        .filter(|(p, ..)| !p[0].is_nan() && !p[1].is_nan())
                        .filter_map(|&(at, _, row)| {
                            let url = look.url.as_ref()?.at(row as usize)?;
                            let size =
                                |extent: Option<Extent>| extent.map(|extent| extent.of(cell_width));
                            let image = Image {
                                at,
                                url,
                                width: size(properties.width),
                                height: size(properties.height),
                                align: properties.align.unwrap_or(Align::Center),
                                baseline: properties.baseline.unwrap_or(Baseline::Middle),
                                aspect: properties.aspect.unwrap_or(true),
                            };
                            if let Some(href) = look.href(row as usize) {
                                let reach =
                                    image.width.unwrap_or(0.0).max(image.height.unwrap_or(0.0))
                                        / 2.0;
                                links.push(Link {
                                    hit: Hit::Disc(at, reach + plan::SLOP),
                                    href,
                                });
                            }
                            Some(image)
                        })
                        .collect();
                    Marks::Images(images)
                }
                _ => Marks::Points(symbols(&points, &look, &ink(group), columns, &mut links)),
            };
            series.push(new_series(group, marks));
            if let Some((look, mark, curve)) = &line_overlay {
                series.push(Series {
                    mark: *mark,
                    marks: Marks::Lines(tops()),
                    style: look.style(first, ink(group)),
                    curve: *curve,
                    ..new_series(group, Marks::Lines(Vec::new()))
                });
            }
            if let Some((look, mark, _)) = &point_overlay {
                let symbols = points
                    .iter()
                    .filter(|(p, ..)| !p[0].is_nan() && !p[1].is_nan())
                    .map(|&(at, _, row)| symbol(at, row as usize, look, ink(group)))
                    .collect();
                series.push(Series {
                    mark: *mark,
                    marks: Marks::Points(symbols),
                    style: look.style(first, ink(group)),
                    ..new_series(group, Marks::Points(Vec::new()))
                });
            }
            if !traced {
                continue;
            }
            let mut trace: Vec<([f32; 2], u32)> = points
                .into_iter()
                .filter(|(p, ..)| !p[value_axis].is_nan())
                .map(|(p, _, row)| (p, row))
                .collect();
            if !trace.is_sorted_by(|a, b| a.0[along_axis] <= b.0[along_axis]) {
                trace.sort_by(|a, b| a.0[along_axis].total_cmp(&b.0[along_axis]));
            }
            hover.traces.push(Trace {
                layer: layer.index,
                color: groups.groups[group].0,
                axis: along_axis,
                points: trace,
            });
        }
    }
    Some((plot, axes, series, hover, links))
}

/// Row `row`'s symbol at `at`.
fn symbol(at: [f32; 2], row: usize, look: &Look, ink: Ink) -> Symbol {
    Symbol {
        at,
        shape: look.shape.at(row),
        size: look.size.at(row),
        angle: look.angle.at(row),
        style: look.style(row, ink),
    }
}

/// A series' points as symbols, at most one to a pixel when there are more
/// than `columns`, with the links of those that have one.
fn symbols(
    points: &[([f32; 2], f32, u32)],
    look: &Look,
    ink: &Ink,
    columns: usize,
    links: &mut Vec<Link>,
) -> Vec<Symbol> {
    let mut seen = HashSet::new();
    let dense = points.len() > columns;
    points
        .iter()
        .filter(|(p, ..)| !p[0].is_nan() && !p[1].is_nan())
        .filter(|(p, ..)| !dense || seen.insert((p[0].round() as i32, p[1].round() as i32)))
        .map(|&(at, _, row)| {
            let symbol = symbol(at, row as usize, look, *ink);
            if let Some(href) = look.href(row as usize) {
                links.push(Link {
                    hit: Hit::Disc(symbol.at, symbol.size.max(0.0).sqrt() / 2.0 + plan::SLOP),
                    href,
                });
            }
            symbol
        })
        .collect()
}

/// About one tick to [`plan::X_SPACING`] across `plot`.
fn x_count(plot: Rect) -> usize {
    (plot.w / plan::X_SPACING).floor().max(2.0) as usize
}
