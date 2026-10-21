//! A layer's non-position channels resolved per row: colours, opacities,
//! sizes, dashes, shapes, angles, texts, links and image urls, and the
//! series its rows fall in and the order they are drawn in.

use std::{cmp::Ordering, collections::HashMap, sync::Arc};

use gpui::{Hsla, SharedString};
use js::regex::Regex;

use crate::{
    data::Column,
    model::{
        Align, Baseline, Cap, Channel, Encode, Join, Kind, Layer, LineBreak, Mark, Shape, Sort,
    },
    plan::{
        Font, Ink, Style,
        domain::{self, Colors, Discrete},
    },
    scale::Linear,
};

/// Vega-Lite's ranges for its numeric channels.
pub(super) const OPACITY: [f32; 2] = [0.3, 0.8];
pub(super) const STROKE_WIDTH: [f32; 2] = [1.0, 4.0];
const ANGLE: [f32; 2] = [0.0, 360.0];
/// A point's area: Vega-Lite's least, and the most for a 20-pixel step.
pub(super) const POINT_SIZE: [f32; 2] = [4.0, 361.0];
const BAR_SIZE_MIN: f32 = 2.0;
/// Vega-Lite's `strokeDash` range, which a discrete field's values take in
/// turn.
const DASHES: [&[f32]; 5] = [
    &[1.0, 0.0],
    &[4.0, 2.0],
    &[2.0, 1.0],
    &[1.0, 1.0],
    &[1.0, 2.0, 4.0, 2.0],
];
/// A point's area with no size channel.
pub(super) const POINT: f32 = 30.0;
pub(super) const LINE_WIDTH: f32 = 2.0;
/// A trail's width with no size channel.
const TRAIL_WIDTH: f32 = 2.0;
/// Vega-Lite's font sizes for a text's size channel, and its default.
pub(super) const FONT_SIZE: [f32; 2] = [8.0, 40.0];
const TEXT_SIZE: f32 = 11.0;
/// Vega-Lite's opacity for a point, circle, square or tick with none given.
const REDUCED_OPACITY: f32 = 0.7;
/// Canvas's miter limit.
const MITER_LIMIT: f32 = 10.0;
/// Ticks a continuous legend shows, about.
const LEGEND_TICKS: usize = 4;
/// What a text cut short ends in, with no `ellipsis` property.
const ELLIPSIS: &str = "\u{2026}";

/// One value for every row, or each row's.
#[derive(Clone, Debug)]
pub(super) enum Each<T> {
    All(T),
    Rows(Vec<T>),
}

impl<T: Clone> Each<T> {
    pub(super) fn at(&self, row: usize) -> T {
        match self {
            Each::All(value) => value.clone(),
            Each::Rows(values) => values[row].clone(),
        }
    }
}

/// A numeric channel's scale from its field's domain onto a range.
pub(super) enum Amount {
    /// Evenly across the range, one step per value.
    Discrete { domain: Discrete, range: [f32; 2] },
    Continuous {
        values: Arc<[f64]>,
        kind: Kind,
        linear: Linear,
        ticks: Vec<f64>,
    },
}

impl Amount {
    /// `channel` over `column` onto `range`. `zero` lets a quantitative
    /// domain take in zero as the channel's scale says.
    pub(super) fn of(
        channel: &Channel,
        column: &Column,
        range: [f32; 2],
        zero: bool,
    ) -> Option<Self> {
        if channel.kind.is_discrete() {
            return Some(Amount::Discrete {
                domain: Discrete::of(column, &channel.sort),
                range,
            });
        }
        let Column::Number(values) = column else {
            return None;
        };
        let (lo, hi) = domain::extent(values.iter().copied());
        let mut scale = channel.clone();
        scale.scale.zero &= zero;
        let ticks = domain::continuous(&scale, lo, hi, LEGEND_TICKS);
        Some(Amount::Continuous {
            values: values.clone(),
            kind: channel.kind,
            linear: Linear {
                domain: ticks.domain,
                range: (range[0], range[1]),
            },
            ticks: ticks.values,
        })
    }

    fn step(index: usize, count: usize, range: [f32; 2]) -> f32 {
        match count {
            0 | 1 => (range[0] + range[1]) / 2.0,
            _ => range[0] + (range[1] - range[0]) * index as f32 / (count - 1) as f32,
        }
    }

    /// `NaN` for a missing value.
    pub(super) fn rows(&self) -> Vec<f32> {
        match self {
            Amount::Discrete { domain, range } => domain
                .codes
                .iter()
                .map(|&code| match code {
                    domain::MISSING => f32::NAN,
                    code => Self::step(code as usize, domain.labels.len(), *range),
                })
                .collect(),
            Amount::Continuous { values, linear, .. } => values
                .iter()
                .map(|&v| match v.is_nan() {
                    true => f32::NAN,
                    false => linear.at(v),
                })
                .collect(),
        }
    }

    /// What a legend shows: each value, or ticks across the domain, and
    /// where each lands.
    pub(super) fn entries(&self) -> Vec<(SharedString, f32)> {
        match self {
            Amount::Discrete { domain, range } => domain
                .labels
                .iter()
                .enumerate()
                .map(|(i, label)| (label.clone(), Self::step(i, domain.labels.len(), *range)))
                .collect(),
            Amount::Continuous {
                kind,
                linear,
                ticks,
                ..
            } => ticks
                .iter()
                .map(|&v| (kind.describe(v).into(), linear.at(v)))
                .collect(),
        }
    }
}

/// A numeric channel's value for each row: a fixed value, or its field
/// through an [`Amount`]. A missing value takes `default`.
fn amounts(
    encode: Option<&Encode<f32>>,
    layer: &Layer,
    range: [f32; 2],
    zero: bool,
    default: f32,
) -> Option<Each<f32>> {
    Some(match encode {
        None => Each::All(default),
        Some(Encode::Value(value)) => Each::All(*value),
        Some(Encode::Scaled(channel)) => {
            let column = channel.column(&layer.data)?;
            let rows = Amount::of(channel, &column, range, zero)?.rows();
            Each::Rows(
                rows.into_iter()
                    .map(|v| if v.is_nan() { default } else { v.max(0.0) })
                    .collect(),
            )
        }
    })
}

/// A discrete field's values per row, each `scheme`'s value at its place.
fn cycled<T: Clone>(
    channel: &Channel,
    layer: &Layer,
    scheme: impl Fn(usize) -> T,
    missing: T,
) -> Option<Each<T>> {
    if !channel.kind.is_discrete() {
        return None;
    }
    let domain = Discrete::of(&channel.column(&layer.data)?, &channel.sort);
    Some(Each::Rows(
        domain
            .codes
            .iter()
            .map(|&code| match code {
                domain::MISSING => missing.clone(),
                code => scheme(code as usize),
            })
            .collect(),
    ))
}

/// A fill or stroke channel: a fixed colour, or a discrete field's values
/// in the theme's categorical colours.
fn inks(encode: Option<&Encode<Hsla>>, layer: &Layer) -> Option<Option<Each<Option<Ink>>>> {
    Some(match encode {
        None => None,
        Some(Encode::Value(color)) => Some(Each::All(Some(Ink::Fixed(*color)))),
        Some(Encode::Scaled(channel)) => {
            if !channel.kind.is_discrete() {
                return None;
            }
            let domain = Discrete::of(&channel.column(&layer.data)?, &channel.sort);
            Some(Each::Rows(
                domain
                    .codes
                    .iter()
                    .map(|&code| (code != domain::MISSING).then_some(Ink::Category(code as usize)))
                    .collect(),
            ))
        }
    })
}

/// A field's values as text, a number or time read out.
pub(super) fn texts(channel: &Channel, column: &Column) -> Vec<SharedString> {
    match column {
        Column::Text(text) => text
            .codes
            .iter()
            .map(|&c| text.names[c as usize].clone())
            .collect(),
        Column::Number(values) => values
            .iter()
            .map(|&v| channel.kind.describe(v).into())
            .collect(),
    }
}

/// A layer's rows as painted, by everything but position.
pub(super) struct Look {
    mark: Mark,
    /// Whether the colour channel paints the fill rather than the stroke.
    filled: bool,
    fill: Option<Each<Option<Ink>>>,
    stroke_ink: Option<Each<Option<Ink>>>,
    opacity: Each<f32>,
    fill_opacity: Each<f32>,
    stroke_opacity: Each<f32>,
    /// A point's area, a line's, rule's or trail's width, a bar's or tick's
    /// thickness, a text's font size. `NaN` for the mark's own.
    pub(super) size: Each<f32>,
    stroke_width: Each<f32>,
    dash: Each<Option<Arc<[f32]>>>,
    pub(super) shape: Each<Shape>,
    pub(super) angle: Each<f32>,
    pub(super) href: Option<Each<Option<SharedString>>>,
    pub(super) text: Each<SharedString>,
    pub(super) url: Option<Each<Option<SharedString>>>,
    stroke: Stroke,
}

/// A layer's stroke properties that no channel carries.
struct Stroke {
    dash_offset: f32,
    cap: Cap,
    join: Join,
    miter_limit: f32,
}

impl Look {
    /// `None` for a channel over a field `layer` lacks, or one it cannot
    /// scale: a continuous shape, dash, fill or stroke.
    pub(super) fn of(layer: &Layer, band: f32) -> Option<Self> {
        let e = &layer.encoding;
        let p = &layer.properties;
        let size =
            |range: [f32; 2], default: f32| amounts(e.size.as_ref(), layer, range, true, default);
        let size = match layer.mark {
            Mark::Point | Mark::Circle | Mark::Square => size(POINT_SIZE, POINT)?,
            Mark::Line | Mark::Area | Mark::Rule => size(STROKE_WIDTH, f32::NAN)?,
            Mark::Trail => size(STROKE_WIDTH, TRAIL_WIDTH)?,
            Mark::Bar | Mark::Tick => size([BAR_SIZE_MIN, band - 1.0], f32::NAN)?,
            Mark::Text => size(FONT_SIZE, p.font_size.unwrap_or(TEXT_SIZE))?,
            Mark::Rect | Mark::Image | Mark::Arc => Each::All(f32::NAN),
        };
        let dash = match &e.stroke_dash {
            None => Each::All(None),
            Some(Encode::Value(dash)) => Each::All(solid(dash)),
            Some(Encode::Scaled(channel)) => cycled(channel, layer, dash, None)?,
        };
        let shape = match (layer.mark, &e.shape) {
            (Mark::Circle, _) => Each::All(Shape::Circle),
            (Mark::Square, _) => Each::All(Shape::Square),
            (_, None) => Each::All(Shape::Circle),
            (_, Some(Encode::Value(shape))) => Each::All(*shape),
            (_, Some(Encode::Scaled(channel))) => {
                cycled(channel, layer, Shape::nth, Shape::Circle)?
            }
        };
        let strings =
            |encode: &Option<Encode<SharedString>>| -> Option<Option<Each<Option<SharedString>>>> {
                Some(match encode {
                    None => None,
                    Some(Encode::Value(text)) => Some(Each::All(Some(text.clone()))),
                    Some(Encode::Scaled(channel)) => Some(Each::Rows(
                        texts(channel, &channel.column(&layer.data)?)
                            .into_iter()
                            .map(|text| (!text.is_empty()).then_some(text))
                            .collect(),
                    )),
                })
            };
        // Lines broken where the layer's `line_break` says, one to a line
        // of the text.
        let regex = match &p.line_break {
            Some(LineBreak::Pattern { source, flags }) => Some(Regex::try_new(source, flags).ok()?),
            _ => None,
        };
        let broken = |text: SharedString| -> SharedString {
            match (&p.line_break, &regex) {
                (_, Some(regex)) => regex.split_str(&text).join("\n").into(),
                (Some(LineBreak::Text(separator)), _) => text
                    .split(separator.as_ref())
                    .collect::<Vec<_>>()
                    .join("\n")
                    .into(),
                _ => text,
            }
        };
        let text = match &e.text {
            None => Each::All(SharedString::default()),
            Some(Encode::Value(text)) => Each::All(broken(text.clone())),
            Some(Encode::Scaled(channel)) => Each::Rows(
                texts(channel, &channel.column(&layer.data)?)
                    .into_iter()
                    .map(broken)
                    .collect(),
            ),
        };
        let reduced = matches!(
            layer.mark,
            Mark::Point | Mark::Circle | Mark::Square | Mark::Tick
        ) && !matches!(e.fill_opacity, Some(Encode::Value(_)));
        let opacity = if reduced { REDUCED_OPACITY } else { 1.0 };
        Some(Self {
            mark: layer.mark,
            filled: p.filled.unwrap_or(layer.mark.filled()),
            fill: inks(e.fill.as_ref(), layer)?,
            stroke: Stroke {
                dash_offset: p.stroke_dash_offset,
                cap: p.stroke_cap,
                join: p.stroke_join,
                miter_limit: p.stroke_miter_limit.unwrap_or(MITER_LIMIT),
            },
            stroke_ink: inks(e.stroke.as_ref(), layer)?,
            opacity: amounts(e.opacity.as_ref(), layer, OPACITY, false, opacity)?,
            fill_opacity: amounts(e.fill_opacity.as_ref(), layer, OPACITY, false, 1.0)?,
            stroke_opacity: amounts(e.stroke_opacity.as_ref(), layer, OPACITY, false, 1.0)?,
            size,
            stroke_width: amounts(
                e.stroke_width.as_ref(),
                layer,
                STROKE_WIDTH,
                false,
                f32::NAN,
            )?,
            dash,
            shape,
            angle: amounts(e.angle.as_ref(), layer, ANGLE, false, 0.0)?,
            href: strings(&e.href)?,
            text,
            url: strings(&e.url)?,
        })
    }

    /// Row `row`'s style, `color` its colour channel's ink.
    pub(super) fn style(&self, row: usize, color: Ink) -> Style {
        let fill = match &self.fill {
            Some(fill) => fill.at(row),
            None => self.filled.then_some(color),
        };
        let stroke = match &self.stroke_ink {
            Some(stroke) => stroke.at(row),
            // An area draws a line along its top in its fill's colour.
            None if self.mark == Mark::Area => fill.or((!self.filled).then_some(color)),
            None => (!self.filled).then_some(color),
        };
        let fill = match self.mark {
            Mark::Line | Mark::Rule => None,
            _ => fill,
        };
        let width = self.stroke_width.at(row);
        let size = self.size.at(row);
        let stroke_width = match (width.is_nan(), self.mark) {
            (false, _) => width,
            (true, Mark::Line | Mark::Area | Mark::Rule) if !size.is_nan() => size,
            (true, Mark::Line | Mark::Area) => LINE_WIDTH,
            (true, _) => 1.0,
        };
        Style {
            fill,
            stroke,
            opacity: self.opacity.at(row),
            fill_opacity: self.fill_opacity.at(row),
            stroke_opacity: self.stroke_opacity.at(row),
            stroke_width,
            dash: self.dash.at(row),
            dash_offset: self.stroke.dash_offset,
            cap: self.stroke.cap,
            join: self.stroke.join,
            miter_limit: self.stroke.miter_limit,
        }
    }

    pub(super) fn href(&self, row: usize) -> Option<SharedString> {
        self.href.as_ref()?.at(row)
    }
}

impl Font {
    /// A text layer's font, from its properties.
    pub(super) fn of(layer: &Layer) -> Self {
        let p = &layer.properties;
        Font {
            family: p.font.clone(),
            weight: p.font_weight.unwrap_or_default(),
            style: p.font_style,
            align: p.align.unwrap_or(Align::Center),
            baseline: p.baseline.unwrap_or(Baseline::Middle),
            line_height: p.line_height,
            limit: p.limit,
            ellipsis: p
                .ellipsis
                .clone()
                .unwrap_or(SharedString::new_static(ELLIPSIS)),
            dir: p.dir,
        }
    }
}

/// The `index`-th of Vega-Lite's dashes, round and round. `None` is solid.
pub(super) fn dash(index: usize) -> Option<Arc<[f32]>> {
    solid(DASHES[index % DASHES.len()])
}

/// `None` for a dash with no gap.
fn solid(dash: &[f32]) -> Option<Arc<[f32]>> {
    (dash.iter().skip(1).step_by(2).any(|gap| *gap > 0.0) && dash.iter().any(|d| *d > 0.0))
        .then(|| dash.into())
}

/// Rows split into series: each row's series, `domain::MISSING` for one with no
/// colour, and each series' colour and first row.
pub(super) struct Groups {
    pub(super) codes: Vec<u32>,
    pub(super) groups: Vec<(usize, usize)>,
}

impl Groups {
    /// Paths split by every channel Vega-Lite groups a line by, and marks
    /// styled as their series by each of those but a size; symbols by
    /// colour and detail and key alone.
    pub(super) fn of(layer: &Layer, index: usize, colors: &Colors) -> Option<Self> {
        let e = &layer.encoding;
        let mut channels: Vec<&Channel> = e.detail.iter().chain(&e.key).collect();
        if !layer.mark.is_symbol() {
            let numbers = [
                &e.opacity,
                &e.fill_opacity,
                &e.stroke_opacity,
                &e.stroke_width,
            ];
            channels.extend(numbers.into_iter().filter_map(|c| c.as_ref()?.channel()));
            channels.extend(
                [&e.fill, &e.stroke]
                    .into_iter()
                    .filter_map(|c| c.as_ref()?.channel()),
            );
            channels.extend(e.stroke_dash.as_ref().and_then(Encode::channel));
            if matches!(layer.mark, Mark::Line | Mark::Area | Mark::Rule | Mark::Arc) {
                channels.extend(e.size.as_ref().and_then(Encode::channel));
            }
        }
        let columns: Vec<Vec<u32>> = channels
            .into_iter()
            .map(|channel| {
                Some(
                    Discrete::of(&channel.column(&layer.data)?, &Sort::Data)
                        .codes
                        .to_vec(),
                )
            })
            .collect::<Option<_>>()?;
        let mut index_of: HashMap<(usize, Vec<u32>), u32> = HashMap::new();
        let mut groups = Vec::new();
        let codes = (0..layer.data.len())
            .map(|row| {
                let Some(color) = colors.of_row(index, row) else {
                    return domain::MISSING;
                };
                let key = (color, columns.iter().map(|codes| codes[row]).collect());
                *index_of.entry(key).or_insert_with(|| {
                    groups.push((color, row));
                    groups.len() as u32 - 1
                })
            })
            .collect::<Vec<u32>>();
        // Series follow their colours' order, then the order first met.
        let mut order: Vec<usize> = (0..groups.len()).collect();
        order.sort_by_key(|&g| groups[g]);
        let mut rank = vec![0u32; groups.len()];
        for (new, &old) in order.iter().enumerate() {
            rank[old] = new as u32;
        }
        Some(Self {
            codes: codes
                .into_iter()
                .map(|code| match code {
                    domain::MISSING => domain::MISSING,
                    code => rank[code as usize],
                })
                .collect(),
            groups: order.into_iter().map(|g| groups[g]).collect(),
        })
    }

    pub(super) fn len(&self) -> usize {
        self.groups.len()
    }
}

/// Each row's place when sorted by the layer's `order` channels, `None`
/// without one.
pub(super) fn ranks(layer: &Layer) -> Option<Option<Vec<u32>>> {
    if layer.encoding.order.is_empty() {
        return Some(None);
    }
    let columns: Vec<(&Channel, Column)> = layer
        .encoding
        .order
        .iter()
        .map(|channel| Some((channel, channel.column(&layer.data)?)))
        .collect::<Option<_>>()?;
    let compare = |a: usize, b: usize| {
        for (channel, column) in &columns {
            let order = match column {
                Column::Number(values) => match (values[a].is_nan(), values[b].is_nan()) {
                    (false, false) => values[a].total_cmp(&values[b]),
                    (nan_a, nan_b) => nan_a.cmp(&nan_b),
                },
                Column::Text(text) => text.get(a).cmp(text.get(b)),
            };
            let order = match channel.sort {
                Sort::Descending => order.reverse(),
                _ => order,
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    };
    let mut rows: Vec<usize> = (0..layer.data.len()).collect();
    rows.sort_by(|&a, &b| compare(a, b));
    let mut rank = vec![0u32; rows.len()];
    for (place, &row) in rows.iter().enumerate() {
        rank[row] = place as u32;
    }
    Some(Some(rank))
}
