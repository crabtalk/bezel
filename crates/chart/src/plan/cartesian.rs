//! Bars, lines, areas, trails, points, rules, rects, ticks, texts and images
//! over an x and a y axis, either of which may be discrete or missing.

use crate::{
    data::{self, Column, Data},
    decimate,
    model::{
        Align, Baseline, Channel, Chart, Domain, Encode, Extent, Kind, Layer, Mark, Orient,
        Overlay, Position, Properties, ScaleType, Stack,
    },
    plan::{
        self, Area, Bands, Bar, Body, Font, Hit, Hover, Image, Ink, Link, Marks, Rect, Series,
        Symbol, Text, Trace, arcs,
        axis::{self, Scaled, Ticked},
        domain::{self, Colors, Discrete},
        invalid,
        layout::Layout,
        look::{self, Groups, Look},
        scales::{self, Context, Role},
    },
};
use d3::{
    scale::{Band, Continuous},
    shape::curve::Curve,
};
use gpui::SharedString;
use std::{
    collections::{HashMap, HashSet},
    mem,
    sync::Arc,
};

/// A bar's corners with no corner property.
const BAR_RADIUS: f32 = 2.0;
/// A tick's thickness with no `thickness` property.
const TICK_THICKNESS: f32 = 1.0;
/// Of a band, what a tick spans with no size: Vega-Lite's tick band padding
/// taken out.
const TICK_BAND: f32 = 0.75;
/// A bar's thickness along a continuous axis with no size or far end:
/// Vega-Lite's `continuousBandSize`.
const CONTINUOUS_BAND: f32 = 5.0;
/// A tick's length along a continuous axis with no size: Vega-Lite's step
/// less two.
const TICK_LENGTH: f32 = 18.0;
/// The row of a point an area or line takes where its series has no row.
const IMPUTED: u32 = u32::MAX;

/// Where a layer's rows fall along the axis the layers share their domain on.
enum Along {
    /// Codes into the chart's discrete domain.
    Discrete(Vec<u32>),
    Continuous(Arc<[f64]>),
    /// Every row at one place, in pixels.
    Pixels(Position),
    /// Each row's value in pixels from the plot's edge, with no scale.
    Identity(Arc<[f64]>),
    /// The layer has no channel on the shared axis.
    Missing,
}

impl Along {
    fn valid(&self, row: usize) -> bool {
        match self {
            Along::Discrete(codes) => codes[row] != domain::MISSING,
            Along::Continuous(values) | Along::Identity(values) => !values[row].is_nan(),
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

/// A stacked layer's rows up the values, Vega's Stack: where each starts
/// and ends, `NaN` for a row in no stack, and where a path's series that
/// has no row in a stack stands in it.
struct Stacked {
    start: Vec<f64>,
    end: Vec<f64>,
    /// A series, a row of the stack it has none in, and its level there.
    imputed: Vec<(usize, usize, f64)>,
}

/// A layer read for laying out.
struct Read<'a> {
    index: usize,
    layer: &'a Layer,
    along: Along,
    /// Where a ranged bar ends along the shared axis.
    along2: Option<Second>,
    /// `NaN` throughout for a layer with no channel on the value axis. On a
    /// discrete value axis, codes into its domain.
    values: Arc<[f64]>,
    /// Where a ranged bar or area ends along the value axis.
    values2: Option<Second>,
    value: Option<Channel>,
    /// Where every row stands on the value axis, for a position in pixels.
    value_pixels: Option<Position>,
    /// Whether `value` has no scale: its values are pixels from the plot's
    /// edge.
    identity: bool,
    /// Each row's offset in pixels from its band's start, for an offset
    /// with no scale.
    offset_pixels: Option<Arc<[f64]>>,
    /// Whether the shared axis's values are dates, which stack by the
    /// second.
    dates: bool,
    /// What a continuous shared axis spans, where its scale's domain is
    /// unaggregated.
    along_spanned: Option<Vec<Arc<[f64]>>>,
    /// Slices, or text at them.
    polar: bool,
    /// Codes into the chart's offset domain.
    offsets: Option<Vec<u32>>,
    /// A fixed offset along x and y, in pixels.
    shift: [f32; 2],
    /// The offset of a mark's far end along x and y: its `x2Offset` and
    /// `y2Offset` where it has an `x2` or `y2`.
    shift2: [f32; 2],
    groups: Groups,
    ranks: Option<Vec<u32>>,
    stacked: Option<Stacked>,
    /// The rows a path breaks at.
    breaks: Option<Vec<bool>>,
    /// Whether it draws an invalid value at its scale's zero or least.
    shown: bool,
}

impl Read<'_> {
    /// Whether row `row` has a place along the shared axis: a missing value
    /// has one where the layer shows invalid values.
    fn placed(&self, row: usize) -> bool {
        self.along.valid(row) || (self.shown && matches!(self.along, Along::Continuous(_)))
    }

    /// The stack row `row` falls in along the shared axis: its band and
    /// offset, or its place.
    fn stack_key(&self, row: usize) -> u64 {
        match &self.along {
            Along::Discrete(codes) => {
                let offset = self.offsets.as_ref().map_or(0, |codes| codes[row]);
                (u64::from(codes[row]) << 32) | u64::from(offset)
            }
            Along::Continuous(values) => data::key(values[row], self.dates),
            Along::Identity(values) => data::key(values[row], false),
            Along::Pixels(_) | Along::Missing => 0,
        }
    }

    /// Where row `row` stands up the values: its value, or its stack's end.
    fn value(&self, row: usize) -> f64 {
        match &self.stacked {
            Some(stacked) => stacked.end[row],
            None => self.values[row],
        }
    }
}

/// What places values along the value axis.
enum Values {
    Continuous(Continuous),
    /// Codes into its domain.
    Discrete(Band),
    Missing,
}

/// Whether `channel`'s scale spans values other than its own: those its
/// rows aggregate, or a data domain's.
fn unaggregated(channel: &Channel) -> bool {
    matches!(
        channel.scale.domain,
        Some(Domain::Unaggregated(_) | Domain::Data(_))
    )
}

/// A channel through a scale.
fn scaled(encode: &Option<Encode<Position>>) -> Option<&Channel> {
    encode
        .as_ref()?
        .channel()
        .filter(|channel| channel.scale.kind != Some(ScaleType::Identity))
}

/// A channel with no scale.
fn identity<V>(encode: &Option<Encode<V>>) -> Option<&Channel> {
    encode
        .as_ref()?
        .channel()
        .filter(|channel| channel.scale.kind == Some(ScaleType::Identity))
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

/// A stack's last bar one way: how far it reaches, its series and place
/// there, and its end.
type Outer = (f64, (usize, usize), End);

/// The marks Vega-Lite stacks when their position asks it to.
fn stackable(mark: Mark) -> bool {
    matches!(
        mark,
        Mark::Bar
            | Mark::Area
            | Mark::Rule
            | Mark::Point
            | Mark::Circle
            | Mark::Square
            | Mark::Line
            | Mark::Text
            | Mark::Tick
    )
}

/// One place in a stack: a row, or a series with no row there, standing at
/// zero, and a row of the stack.
#[derive(Clone, Copy)]
enum Entry {
    Row(usize),
    Imputed(usize, usize),
}

/// Vega's Stack of `layer`'s rows by `offset`: one stack per place along
/// the shared axis, each in its series' order, down a vertical stack's
/// series and along a horizontal one's, or by `order`. A path's series with
/// no row in a stack stands in it at zero.
fn stack(layer: &Read, offset: Stack, vertical: bool, impute: bool) -> Stacked {
    let rows = layer.values.len();
    let groups = &layer.groups;
    let key = |row: usize| -> Option<u64> {
        if groups.codes[row] == domain::MISSING || !layer.placed(row) || layer.values[row].is_nan()
        {
            return None;
        }
        if layer
            .offsets
            .as_ref()
            .is_some_and(|codes| codes[row] == domain::MISSING)
        {
            return None;
        }
        Some(layer.stack_key(row))
    };
    let mut index: HashMap<u64, usize> = HashMap::new();
    let mut stacks: Vec<Vec<Entry>> = Vec::new();
    for row in 0..rows {
        if let Some(key) = key(row) {
            let at = *index.entry(key).or_insert_with(|| {
                stacks.push(Vec::new());
                stacks.len() - 1
            });
            stacks[at].push(Entry::Row(row));
        }
    }
    if impute {
        for stack in &mut stacks {
            let present: HashSet<u32> = stack
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Row(row) => Some(groups.codes[*row]),
                    Entry::Imputed(..) => None,
                })
                .collect();
            let Some(&Entry::Row(sample)) = stack.first() else {
                continue;
            };
            for series in 0..groups.len() {
                if !present.contains(&(series as u32)) {
                    stack.push(Entry::Imputed(series, sample));
                }
            }
        }
    }
    let series_rank = |series: usize| match vertical {
        true => groups.len() - 1 - series,
        false => series,
    };
    let rank = |entry: &Entry| -> (u64, usize) {
        match (entry, &layer.ranks) {
            (Entry::Row(row), Some(ranks)) => (u64::from(ranks[*row]), *row),
            (Entry::Row(row), None) => (series_rank(groups.codes[*row] as usize) as u64, *row),
            (Entry::Imputed(..), Some(_)) => (u64::MAX, usize::MAX),
            (Entry::Imputed(series, _), None) => (series_rank(*series) as u64, usize::MAX),
        }
    };
    for stack in &mut stacks {
        stack.sort_by_key(rank);
    }
    let value = |entry: &Entry| match entry {
        Entry::Row(row) => layer.values[*row],
        Entry::Imputed(..) => 0.0,
    };
    let sums: Vec<f64> = stacks
        .iter()
        .map(|stack| stack.iter().map(|e| value(e).abs()).sum())
        .collect();
    let max = sums.iter().copied().fold(0.0, f64::max);
    let mut stacked = Stacked {
        start: vec![f64::NAN; rows],
        end: vec![f64::NAN; rows],
        imputed: Vec::new(),
    };
    for (stack, sum) in stacks.iter().zip(sums) {
        let mut set = |entry: &Entry, y0: f64, y1: f64| match *entry {
            Entry::Row(row) => (stacked.start[row], stacked.end[row]) = (y0, y1),
            Entry::Imputed(series, sample) => stacked.imputed.push((series, sample, y1)),
        };
        match offset {
            Stack::Center => {
                let mut last = (max - sum) / 2.0;
                for entry in stack {
                    let y0 = last;
                    last += value(entry).abs();
                    set(entry, y0, last);
                }
            }
            Stack::Normalize => {
                let scale = 1.0 / sum;
                let (mut last, mut total) = (0.0, 0.0);
                for entry in stack {
                    let y0 = last;
                    total += value(entry).abs();
                    last = scale * total;
                    set(entry, y0, last);
                }
            }
            _ => {
                let (mut up, mut down) = (0.0, 0.0);
                for entry in stack {
                    let v = value(entry);
                    let last = if v < 0.0 { &mut down } else { &mut up };
                    let y0 = *last;
                    *last += v;
                    set(entry, y0, *last);
                }
            }
        }
    }
    stacked
}

/// The context Vega-Lite's defaults read for `channel`, a position of
/// `layer`.
fn context(layer: &Layer, role: Role) -> Context {
    let quantitative = |encode: &Option<Encode<Position>>| {
        scaled(encode).is_some_and(|channel| channel.kind == Kind::Quantitative)
    };
    Context {
        role,
        mark: layer.mark,
        orient: layer.properties.orient,
        both_measures: quantitative(&layer.encoding.x) && quantitative(&layer.encoding.y),
    }
}

/// A position range in pixels from `start`, the plot's edge: the scale's
/// own, `size` long, or `default`, its ends moved to `rangeMin` and
/// `rangeMax` pixels from `start`.
fn range(channel: &Channel, start: f32, size: f32, default: [f32; 2]) -> Option<Vec<f64>> {
    let start = f64::from(start);
    let scale = &channel.scale;
    Some(match scales::positions(scale, size)? {
        Some(own) => own.into_iter().map(|p| start + p).collect(),
        None => vec![
            scale.range_min.map_or(f64::from(default[0]), |p| start + p),
            scale.range_max.map_or(f64::from(default[1]), |p| start + p),
        ],
    })
}

/// `titles` are x's and y's axis titles.
pub(super) fn cartesian(
    chart: &Chart,
    colors: &Colors,
    layout: &mut Layout,
    titles: [Option<SharedString>; 2],
) -> Option<Body> {
    // The layers that place both x and y set the axes; y is the shared axis
    // when each has a discrete y over a continuous x, or, with no orient
    // given, is a bar spanning y to y2 over a continuous x. Without such a
    // layer,
    // the one axis there is: a continuous x is the value axis, a discrete y
    // the shared one.
    let placed: Vec<&Layer> = chart
        .layers
        .iter()
        .filter(|layer| scaled(&layer.encoding.x).is_some() && scaled(&layer.encoding.y).is_some())
        .collect();
    let first = |pick: fn(&Layer) -> &Option<Encode<Position>>| {
        chart
            .layers
            .iter()
            .find_map(|layer| Some((layer, scaled(pick(layer))?)))
    };
    let (x, y) = match placed.first() {
        Some(layer) => (
            Some((*layer, scaled(&layer.encoding.x)?)),
            Some((*layer, scaled(&layer.encoding.y)?)),
        ),
        None => (first(|l| &l.encoding.x), first(|l| &l.encoding.y)),
    };
    let discrete =
        |axis: Option<(&Layer, &Channel)>| axis.is_some_and(|(_, c)| c.kind.is_discrete());
    // A bar's, area's, line's or trail's own orient decides between two
    // continuous axes, or for one alone.
    let orient = chart
        .layers
        .iter()
        .filter(|layer| {
            matches!(
                layer.mark,
                Mark::Bar | Mark::Area | Mark::Line | Mark::Trail
            )
        })
        .find_map(|layer| layer.properties.orient);
    let continuous =
        |axis: Option<(&Layer, &Channel)>| axis.is_some_and(|(_, c)| !c.kind.is_discrete());
    let horizontal = match (placed.is_empty(), x, y) {
        (false, ..)
            if placed.iter().all(|layer| {
                let encoding = &layer.encoding;
                [&encoding.x, &encoding.y]
                    .iter()
                    .all(|axis| scaled(axis).is_some_and(|c| !c.kind.is_discrete()))
            }) =>
        {
            match orient {
                Some(orient) => orient == Orient::Horizontal,
                None => placed.iter().all(|layer| {
                    let encoding = &layer.encoding;
                    layer.mark == Mark::Bar && encoding.y2.is_some() && encoding.x2.is_none()
                }),
            }
        }
        (false, ..) => placed.iter().all(|layer| {
            let encoding = &layer.encoding;
            scaled(&encoding.y).is_some_and(|y| y.kind.is_discrete())
                && scaled(&encoding.x).is_some_and(|x| !x.kind.is_discrete())
        }),
        // Nothing on a scale: positions with no scale, or nothing to draw.
        (true, None, None) => {
            let unscaled = |layer: &Layer| {
                identity(&layer.encoding.x).is_some() || identity(&layer.encoding.y).is_some()
            };
            if !chart.layers.iter().any(unscaled) {
                return None;
            }
            false
        }
        (true, Some(_), None) => match (continuous(x), orient) {
            (true, Some(orient)) => orient == Orient::Horizontal,
            _ => !discrete(x),
        },
        (true, None, Some(_)) => match (continuous(y), orient) {
            (true, Some(orient)) => orient == Orient::Horizontal,
            _ => discrete(y),
        },
        (true, Some(_), Some(_)) => discrete(y) && !discrete(x),
    };
    let (shared, value) = match horizontal {
        true => (y, x),
        false => (x, y),
    };
    let is_discrete = discrete(shared);
    let value_discrete = discrete(value);

    let mut labels: Vec<SharedString> = Vec::new();
    let mut index: HashMap<SharedString, u32> = HashMap::new();
    let mut value_labels: Vec<SharedString> = Vec::new();
    let mut value_index: HashMap<SharedString, u32> = HashMap::new();
    let mut offset_labels: Vec<SharedString> = Vec::new();
    let mut offset_index: HashMap<SharedString, u32> = HashMap::new();
    let mut offset_channel: Option<(&Layer, &Channel)> = None;
    let mut offset_identity = false;
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
        let (along_identity, value_identity) = (identity(along_e), identity(value_e));
        let has_along = along.is_some() || along_pixels.is_some() || along_identity.is_some();
        let has_value = value.is_some() || value_pixels.is_some() || value_identity.is_some();
        // Slices, and text at them, stand at the plot's middle without x
        // or y.
        let polar = mark == Mark::Arc || (mark == Mark::Text && e.theta.is_some());
        let partial = matches!(
            mark,
            Mark::Bar
                | Mark::Rule
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
            || value.is_some_and(|value| value.kind.is_discrete() != value_discrete)
            || (along2_e.is_some()
                && !matches!(mark, Mark::Bar | Mark::Rule | Mark::Rect | Mark::Tick))
            || (ranged && !matches!(mark, Mark::Bar | Mark::Area | Mark::Rule | Mark::Rect))
            || (ranged && value_discrete)
            || (!has_along && along2_e.is_some())
            || (along.is_some() && is_discrete && along2_e.is_some())
            || (!has_value && ranged)
            || (value.is_some_and(|v| v.kind == Kind::Temporal) && mark == Mark::Bar && !ranged)
            || (mark == Mark::Bar
                && (along_pixels.is_some() || along_identity.is_some())
                && along2_e.is_none())
            || (mark == Mark::Rect
                && !((along.is_none() || is_discrete || along2_e.is_some())
                    && (value.is_none() || ranged || value_discrete)))
        {
            return None;
        }
        let dates = along
            .and_then(Channel::field)
            .is_some_and(|field| layer.data.is_time(field));
        let along_spanned = match along {
            Some(channel) if unaggregated(channel) => Some(channel.spanned(&layer.data)?),
            _ => None,
        };
        let along = match (along, along_pixels) {
            (Some(along), _) => match (is_discrete, along.column(&layer.data)?) {
                (true, column) => Along::Discrete(
                    Discrete::of(&column, &along.sort)
                        .within(&along.scale)
                        .into_shared(&mut labels, &mut index),
                ),
                (false, Column::Number(values)) => Along::Continuous(values),
                (false, Column::Text(_) | Column::Value(_)) => return None,
            },
            (None, Some(position)) => Along::Pixels(position),
            (None, None) if let Some(channel) = along_identity => {
                Along::Identity(channel.numbers(&layer.data)?)
            }
            // An offset with no position to offset from splits the plot.
            (None, None) if shared.is_none() && matches!(offset, Some(Encode::Scaled(_))) => {
                Along::Discrete(vec![0; layer.data.len()])
            }
            (None, None) => Along::Missing,
        };
        // An offset with no scale moves each row from its band's start; on
        // a continuous axis it is dropped.
        let offset_pixels = match identity(offset) {
            Some(channel) if matches!(along, Along::Discrete(_)) => {
                offset_identity = true;
                Some(channel.numbers(&layer.data)?)
            }
            _ => None,
        };
        let offsets = match offset {
            None => None,
            Some(Encode::Scaled(channel)) if channel.scale.kind == Some(ScaleType::Identity) => {
                None
            }
            Some(Encode::Value(pixels)) => {
                shift[usize::from(horizontal)] += *pixels;
                None
            }
            Some(Encode::Scaled(channel)) => {
                if !(is_discrete || shared.is_none())
                    || !channel.kind.is_discrete()
                    || !matches!(along, Along::Discrete(_))
                {
                    return None;
                }
                offset_channel.get_or_insert((layer, channel));
                let domain = Discrete::of(&channel.column(&layer.data)?, &channel.sort)
                    .within(&channel.scale);
                Some(domain.into_shared(&mut offset_labels, &mut offset_index))
            }
        };
        let p = &layer.properties;
        let shift2 = [
            e.x2.as_ref().map_or(shift[0], |_| p.x2_offset),
            e.y2.as_ref().map_or(shift[1], |_| p.y2_offset),
        ];
        let values: Arc<[f64]> = match value {
            Some(value) if value_discrete => Discrete::of(&value.column(&layer.data)?, &value.sort)
                .within(&value.scale)
                .into_shared(&mut value_labels, &mut value_index)
                .into_iter()
                .map(|code| match code {
                    domain::MISSING => f64::NAN,
                    code => f64::from(code),
                })
                .collect(),
            Some(value) => value.numbers(&layer.data)?,
            None => match value_identity {
                Some(channel) => channel.numbers(&layer.data)?,
                None => vec![f64::NAN; layer.data.len()].into(),
            },
        };
        read.push(Read {
            index: layer_index,
            layer,
            along,
            along2: Second::of(along2_e, &layer.data)?,
            values,
            values2: Second::of(value2_e, &layer.data)?,
            value: value.or(value_identity).cloned(),
            value_pixels,
            identity: value_identity.is_some(),
            offset_pixels,
            dates,
            along_spanned,
            polar,
            offsets,
            shift,
            shift2,
            groups: Groups::of(layer, layer_index, colors)?,
            ranks: look::ranks(layer)?,
            stacked: None,
            breaks: invalid::breaks(layer),
            shown: invalid::shown(layer),
        });
    }

    // Stacks, then the value axis's extent across them.
    for layer in &mut read {
        let Some(channel) = &layer.value else {
            continue;
        };
        let mark = layer.layer.mark;
        if value_discrete
            || layer.polar
            || layer.values2.is_some()
            || channel.kind != Kind::Quantitative
        {
            continue;
        }
        let offset = match channel.stack {
            Stack::Auto if matches!(mark, Mark::Bar | Mark::Area) => Stack::Zero,
            Stack::Auto | Stack::Off => continue,
            offset if stackable(mark) => offset,
            _ => continue,
        };
        let impute = matches!(mark, Mark::Area | Mark::Line);
        layer.stacked = Some(stack(layer, offset, !horizontal, impute));
    }
    let mut extent = Vec::new();
    for layer in read.iter().filter(|layer| !layer.identity) {
        let rows = (0..layer.values.len())
            .filter(|&row| layer.placed(row) && layer.groups.codes[row] != domain::MISSING);
        match &layer.stacked {
            Some(stacked) => {
                for row in rows {
                    extent.extend([stacked.start[row], stacked.end[row]]);
                }
                extent.extend(stacked.imputed.iter().map(|(.., level)| *level));
            }
            None => {
                let values2 = layer.values2.as_ref().and_then(Second::values);
                let spanned = match &layer.value {
                    // Values of no row's.
                    Some(channel) if let Some(Domain::Data(values)) = &channel.scale.domain => {
                        extent.extend(values.iter());
                        Vec::new()
                    }
                    Some(channel) if unaggregated(channel) => channel.spanned(&layer.layer.data)?,
                    _ => vec![layer.values.clone()],
                };
                for row in rows {
                    extent.extend(spanned.iter().map(|values| values[row]));
                    if let Some(values2) = values2 {
                        extent.push(values2[row]);
                    }
                }
            }
        }
    }
    let extent = domain::extent(extent);
    let along_extent = domain::extent(read.iter().flat_map(|layer| {
        let Along::Continuous(values) = &layer.along else {
            return Vec::new();
        };
        let along2 = layer.along2.as_ref().and_then(Second::values);
        let spanned = layer
            .along_spanned
            .as_deref()
            .unwrap_or(std::slice::from_ref(values));
        spanned
            .iter()
            .flat_map(|values| values.iter())
            .chain(along2.iter().flat_map(|v| v.iter()))
            .copied()
            .collect()
    }));

    let count = labels.len();
    let offsets = offset_labels.len().max(1);
    let nested = offset_channel.is_some() || offset_identity;
    // A discrete position's band or point scale over `range`.
    let band_of = |(layer, channel): (&Layer, &Channel),
                   role: Role,
                   nested: bool,
                   count: usize,
                   range: [f32; 2]| {
        let kind = scales::scale_type(channel, role, layer.mark, nested);
        scales::band(
            channel,
            kind,
            layer.mark,
            nested,
            false,
            count,
            [f64::from(range[0]), f64::from(range[1])],
        )
    };
    let (along_role, value_role) = match horizontal {
        true => (Role::Y, Role::X),
        false => (Role::X, Role::Y),
    };
    // A continuous position's scale over `range`.
    let continuous_of = |(layer, channel): (&Layer, &Channel), role: Role, extent, range| {
        let kind = scales::scale_type(channel, role, layer.mark, false);
        scales::continuous(channel, kind, &context(layer, role), extent, range)
    };

    let [x_title, y_title] = titles;
    let (shared_title, value_title) = match horizontal {
        true => (y_title, x_title),
        false => (x_title, y_title),
    };
    let (plot, (values_scale, along_band, along_scale), axes) = layout.fit(|layout, plot| {
        let across = [plot.x, plot.right()];
        let down = [plot.y, plot.bottom()];
        // A continuous position's range: x rightward, y upward.
        let continuous_range = |channel: &Channel, x: bool| match x {
            true => range(channel, plot.x, plot.w, [plot.x, plot.right()]),
            false => range(channel, plot.y, plot.h, [plot.bottom(), plot.y]),
        };
        let (along_band, along_scale) = match shared {
            Some(shared) if is_discrete => {
                let range = if horizontal { down } else { across };
                (
                    Some(band_of(shared, along_role, nested, count, range)),
                    None,
                )
            }
            Some(shared) => {
                let range = continuous_range(shared.1, !horizontal)?;
                let scale = continuous_of(shared, along_role, along_extent, range);
                (None, Some(scale))
            }
            None => (None, None),
        };
        let values_scale = match value {
            Some(value) if value_discrete => {
                let range = if horizontal { across } else { down };
                let band = band_of(value, value_role, false, value_labels.len(), range);
                Values::Discrete(band)
            }
            Some(value) => {
                let range = continuous_range(value.1, horizontal)?;
                Values::Continuous(continuous_of(value, value_role, extent, range))
            }
            None => Values::Missing,
        };
        let mut axes = Vec::new();
        if let Some((layer, channel)) = value {
            let ticked = match &values_scale {
                Values::Discrete(band) => Ticked::Bands(band, &value_labels),
                Values::Continuous(scale) => Ticked::Continuous(scale),
                Values::Missing => unreachable!("a value channel has a scale"),
            };
            let scaled = Scaled {
                channel,
                mark: layer.mark,
                x: horizontal,
                ticked,
                title: value_title.clone(),
            };
            axes.extend(axis::axis(layout, &scaled, plot));
        }
        if let Some((layer, channel)) = shared {
            let ticked = match (&along_band, &along_scale) {
                (Some(band), _) => Ticked::Bands(band, &labels),
                (_, Some(scale)) => Ticked::Continuous(scale),
                (None, None) => unreachable!("a shared channel has a scale"),
            };
            let scaled = Scaled {
                channel,
                mark: layer.mark,
                x: !horizontal,
                ticked,
                title: shared_title.clone(),
            };
            axes.extend(axis::axis(layout, &scaled, plot));
        }
        Some(((values_scale, along_band, along_scale), axes))
    })?;

    // The plot's extent along the shared axis, and up the value axis.
    let (along_range, value_range) = match horizontal {
        true => ((plot.y, plot.bottom()), (plot.x, plot.right())),
        false => ((plot.x, plot.right()), (plot.bottom(), plot.y)),
    };
    let along_mid = (along_range.0 + along_range.1) / 2.0;
    let value_mid = (value_range.0 + value_range.1) / 2.0;
    let band = along_band.unwrap_or(Band::new(
        1,
        [f64::from(along_range.0), f64::from(along_range.1)],
    ));
    let offset_band = |start: f64, width: f64| match offset_channel {
        Some((layer, channel)) => {
            let kind = scales::scale_type(channel, Role::Offset, layer.mark, false);
            scales::band(
                channel,
                kind,
                layer.mark,
                false,
                true,
                offset_labels.len(),
                [start, start + width],
            )
        }
        None => Band::new(1, [start, start + width]),
    };
    // The filled part of band `b` at offset `o`: its start and width.
    let cell = |b: usize, o: u32| -> (f32, f32) {
        let (start, width) = (band.at(b), band.bandwidth());
        match offset_labels.is_empty() {
            true => (start as f32, width as f32),
            false => {
                let sub = offset_band(start, width);
                (sub.at(o as usize) as f32, sub.bandwidth() as f32)
            }
        }
    };
    let cell_width = match offset_labels.is_empty() {
        true => band.bandwidth() as f32,
        false => offset_band(0.0, band.bandwidth()).bandwidth() as f32,
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
    // Pixels from the plot's edge on x or y, on screen.
    let identity = |pixels: f64, axis: usize| pixel(Position::Pixels(pixels as f32), axis);
    let place = |layer: &Read, row: usize| -> f32 {
        match (&layer.along, &along_scale) {
            (Along::Discrete(_), _) => match &layer.offset_pixels {
                Some(offsets) => {
                    cell_of(layer, row).map_or(f32::NAN, |(start, _)| start + offsets[row] as f32)
                }
                None => cell_of(layer, row).map_or(f32::NAN, |(start, width)| start + width / 2.0),
            },
            (Along::Continuous(values), Some(scale)) if values[row].is_nan() && layer.shown => {
                scale.at(invalid::zero_or_min(&scale.domain)) as f32
            }
            (Along::Continuous(values), Some(scale)) => scale.at(values[row]) as f32,
            (Along::Continuous(_), None) => f32::NAN,
            (Along::Identity(values), _) => identity(values[row], along_axis),
            (Along::Pixels(position), _) => pixel(*position, along_axis),
            (Along::Missing, _) => along_mid,
        }
    };
    // Where a row's `x2` or `y2` puts it along the shared axis, `NaN` for a
    // missing value; `None` without one.
    let place2 = |layer: &Read, row: usize| -> Option<f32> {
        Some(match (layer.along2.as_ref()?, &along_scale) {
            (Second::Values(values), _) if matches!(layer.along, Along::Identity(_)) => {
                identity(values[row], along_axis)
            }
            (Second::Values(values), Some(scale)) if !values[row].is_nan() => {
                scale.at(values[row]) as f32
            }
            (Second::Values(_), _) => f32::NAN,
            (Second::Pixels(position), _) => pixel(*position, along_axis),
        })
    };
    // Where `layer`'s value `v` stands up the values.
    let at = |layer: &Read, v: f64| -> f32 {
        let v = match (&values_scale, v.is_nan() && layer.shown && !layer.identity) {
            (Values::Continuous(scale), true) => invalid::zero_or_min(&scale.domain),
            _ => v,
        };
        if v.is_nan() {
            return f32::NAN;
        }
        if layer.identity {
            return identity(v, value_axis);
        }
        match &values_scale {
            Values::Continuous(scale) => scale.at(v) as f32,
            Values::Discrete(band) => (band.at(v as usize) + band.bandwidth() / 2.0) as f32,
            Values::Missing => f32::NAN,
        }
    };
    // The cell a row falls in up a discrete value axis, from one edge to the
    // other; `None` on a continuous one.
    let value_cell = |layer: &Read, row: usize| -> Option<(f32, f32)> {
        let Values::Discrete(band) = &values_scale else {
            return None;
        };
        let code = layer.values[row];
        (!code.is_nan()).then(|| {
            let start = band.at(code as usize);
            (start as f32, (start + band.bandwidth()) as f32)
        })
    };
    // Where a row stands up the values, `NaN` for a missing value; `None`
    // for a layer without a value position.
    let value_at = |layer: &Read, row: usize| -> Option<f32> {
        match (&layer.value, layer.value_pixels) {
            (Some(_), _) => Some(at(layer, layer.value(row))),
            (None, Some(position)) => Some(pixel(position, value_axis)),
            (None, None) => None,
        }
    };
    let value2_at = |layer: &Read, row: usize| -> Option<f32> {
        if let Some(stacked) = &layer.stacked {
            return Some(at(layer, stacked.start[row]));
        }
        Some(match layer.values2.as_ref()? {
            Second::Values(values) => at(layer, values[row]),
            Second::Pixels(position) => pixel(*position, value_axis),
        })
    };
    // Zero on a continuous scale, held inside its domain; the axis's start
    // on any other.
    let zero = |scale: &Continuous| {
        let (first, last) = (scale.domain[0], scale.domain[scale.domain.len() - 1]);
        scale.at(0.0f64.clamp(first.min(last), first.max(last))) as f32
    };
    let base = match &values_scale {
        Values::Continuous(scale) => zero(scale),
        _ => value_range.0,
    };
    // Where the shared axis starts: zero, or its least, on a continuous
    // scale; the left or bottom edge on a discrete one.
    let along_base = match &along_scale {
        Some(scale) => zero(scale),
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
        // With no scale, values start at the plot's edge.
        let base = match layer.identity {
            true => identity(0.0, value_axis),
            false => base,
        };
        let at = |v: f64| at(layer, v);
        if layer.polar {
            // Each row's pie centred where its x and y place it.
            let center = |row: usize| {
                if !layer.placed(row) {
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
        let look = Look::of(layer.layer, band.step() as f32 / offsets as f32)?;
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
        let valid = |row: usize| groups.codes[row] != domain::MISSING && layer.placed(row);
        // The extent across its band a band-relative width takes.
        let across = match horizontal {
            true => properties.height,
            false => properties.width,
        };

        if mark == Mark::Bar {
            let mut bars: Vec<Vec<Bar>> = vec![Vec::new(); groups.len()];
            let mut readouts = vec![vec![None; count.max(1)]; groups.len()];
            let mut totals = vec![vec![f64::NAN; count.max(1)]; groups.len()];
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
            // Where a row's bar runs along the shared axis.
            let span = |row: usize| -> Option<(f32, f32)> {
                Some(match &layer.along {
                    // Vega-Lite's bar from its band's start moved by an
                    // offset with no scale, back to the start.
                    Along::Discrete(_) if let Some(offsets) = &layer.offset_pixels => {
                        let (start, _) = cell_of(layer, row)?;
                        (start + offsets[row] as f32, start)
                    }
                    Along::Discrete(_) => {
                        let (start, width) = cell_of(layer, row)?;
                        thick(start, width, row)
                    }
                    Along::Missing => along_range,
                    // Along a continuous axis, a bar is Vega-Lite's
                    // continuous band size across without an `x2` or `y2`.
                    _ => match place2(layer, row) {
                        Some(along2) => (place(layer, row), along2),
                        None => {
                            let at = place(layer, row);
                            let size = look.size.at(row);
                            let size = if size.is_nan() { CONTINUOUS_BAND } else { size };
                            (at - size / 2.0, at + size / 2.0)
                        }
                    },
                })
            };
            // Each stack's last bar each way: how far it reaches, and where
            // it is.
            let mut outer: HashMap<(u64, bool), Outer> = HashMap::new();
            let kind = layer.value.as_ref().map(|channel| channel.kind);
            for row in 0..layer.values.len() {
                if !valid(row) {
                    continue;
                }
                let group = groups.codes[row] as usize;
                let Some(along) = span(row) else {
                    continue;
                };
                let (far, near, cap) = match (&layer.stacked, value_cell(layer, row)) {
                    (_, Some((lo, hi))) => (lo, hi, End::Both),
                    (Some(stacked), None) => {
                        let (from, to) = (at(stacked.start[row]), at(stacked.end[row]));
                        (from, to, End::Neither)
                    }
                    (None, None) => match (value_at(layer, row), value2_at(layer, row)) {
                        (None, _) => (value_range.0, value_range.1, End::Both),
                        (Some(v), Some(v2)) => (v2, v, End::Both),
                        (Some(v), None) => (base, v, end(base, v)),
                    },
                };
                if along.0.is_nan() || along.1.is_nan() || far.is_nan() || near.is_nan() {
                    continue;
                }
                if let (Along::Discrete(codes), Some(kind)) = (&layer.along, kind) {
                    let b = codes[row] as usize;
                    let v = layer.values[row];
                    match layer.values2.as_ref().and_then(Second::values) {
                        _ if value_discrete => {
                            readouts[group][b] = Some(value_labels[v as usize].clone())
                        }
                        Some(values2) => {
                            readouts[group][b] = Some(
                                format!("{} – {}", kind.describe(v), kind.describe(values2[row]))
                                    .into(),
                            )
                        }
                        None => {
                            let total = &mut totals[group][b];
                            *total = if total.is_nan() { v } else { *total + v };
                        }
                    }
                }
                if let Some(stacked) = &layer.stacked
                    && stacked.start[row] == stacked.end[row]
                {
                    continue;
                }
                let bar = Bar::of(placed(layer, along, far, near), mark, properties, cap);
                if let Some(href) = look.href(row) {
                    links.push(Link {
                        hit: Hit::Rect(bar.rect),
                        href,
                    });
                }
                bars[group].push(bar);
                let index = (group, bars[group].len() - 1);
                if let Some(stacked) = &layer.stacked {
                    let key = layer.stack_key(row);
                    let (start, stop) = (stacked.start[row], stacked.end[row]);
                    let reach = stop.abs();
                    let last = outer.entry((key, stop < start)).or_insert((
                        f64::NEG_INFINITY,
                        index,
                        End::Neither,
                    ));
                    if reach >= last.0 {
                        *last = (reach, index, end(far, near));
                    }
                }
            }
            if properties.corner_radius_end.is_some() {
                for (_, (group, index), cap) in outer.into_values() {
                    let rect = bars[group][index].rect;
                    bars[group][index] = Bar::of(rect, mark, properties, cap);
                }
            }
            for (readouts, totals) in readouts.iter_mut().zip(totals) {
                for (readout, total) in readouts.iter_mut().zip(totals) {
                    if !total.is_nan() {
                        *readout = Some(Kind::Quantitative.describe(total).into());
                    }
                }
            }
            if is_discrete {
                let bands = hover.bands.get_or_insert_with(|| Bands {
                    cells: (0..count)
                        .map(|b| {
                            let step = band.step();
                            let start = band.at(b) - (step - band.bandwidth()) / 2.0;
                            (start as f32, step as f32)
                        })
                        .collect(),
                    vertical: !horizontal,
                    across: match horizontal {
                        true => (plot.x, plot.right()),
                        false => (plot.y, plot.bottom()),
                    },
                    labels: labels
                        .iter()
                        .map(|label| {
                            let channel = shared.map(|(_, channel)| channel);
                            let format = channel
                                .and_then(|channel| channel.axis.as_deref())
                                .and_then(|axis| axis.format.as_ref());
                            let kind = channel.map_or(Kind::Nominal, |channel| channel.kind);
                            axis::discrete(label, kind, format).1.into()
                        })
                        .collect(),
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
                            (Along::Pixels(_) | Along::Identity(_), None) => (along, along_base),
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
                        let (far, near) = match (v2, has_value, value_cell(layer, row)) {
                            (Some(v2), ..) => (v2, v),
                            (None, true, Some(cell)) => cell,
                            (None, true, None) => (base, v),
                            (None, false, _) => value_range,
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
                            // A tick from x to x2 runs along them.
                            _ if along2.is_some() => (
                                (along, along2.unwrap_or(along)),
                                (v - thickness / 2.0, v + thickness / 2.0),
                            ),
                            Along::Continuous(_) | Along::Pixels(_) | Along::Identity(_) => {
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
                                let room = match band.padding_inner > 0.0 {
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
            let value = match layer.breaks.as_ref().is_some_and(|breaks| breaks[row]) {
                true => f32::NAN,
                false => value_at(layer, row).unwrap_or(value_mid),
            };
            let bottom = value2_at(layer, row).unwrap_or(base) + layer.shift2[value_axis];
            let [dx, dy] = layer.shift;
            let at = point(along, value);
            points[group as usize].push(([at[0] + dx, at[1] + dy], bottom, row as u32));
        }
        // A stacked path's series stands at its level where it has no row.
        for &(group, sample, level) in layer.stacked.iter().flat_map(|s| &s.imputed) {
            let [dx, dy] = layer.shift;
            let top = point(place(layer, sample), at(level));
            let bottom = at(level) + layer.shift2[value_axis];
            points[group].push(([top[0] + dx, top[1] + dy], bottom, IMPUTED));
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
            Some((Look::of(&synthetic, band.step() as f32)?, mark, curve))
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
                            let base = match layer.values2.is_some() || layer.stacked.is_some() {
                                false => vec![corner(run[0].0), corner(run[run.len() - 1].0)],
                                true => run
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
                    .filter(|(p, _, row)| !p[0].is_nan() && !p[1].is_nan() && *row != IMPUTED)
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
                .filter(|(p, _, row)| !p[value_axis].is_nan() && *row != IMPUTED)
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
