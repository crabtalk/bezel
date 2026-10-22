//! A layer's non-position channels resolved per row: colours, opacities,
//! sizes, dashes, shapes, angles, texts, links and image urls, and the
//! series its rows fall in and the order they are drawn in.

use crate::{
    data::{Column, Data},
    model::{
        Align, Baseline, Cap, Channel, Datum, Domain, Encode, Join, Layer, LineBreak, Mark,
        NamedRange, Range, ScaleType, Shape, Sort,
    },
    plan::{
        Font, Ink, Style,
        domain::{self, Colors, Discrete},
        invalid,
        scales::{self, ColorScale, Colored, Context, Role, Steps},
    },
    value::regexp::RegExp,
};
use d3::{format::number::Number as JsNumber, scale::Continuous};
use gpui::{Hsla, SharedString};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};

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
pub(super) const REDUCED_OPACITY: f32 = 0.7;
/// Canvas's miter limit.
const MITER_LIMIT: f32 = 10.0;
/// What a text cut short ends in, with no `ellipsis` property.
pub(super) const ELLIPSIS: &str = "\u{2026}";

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
    /// No scale: each value as it is.
    Identity(Arc<[f64]>),
    /// A point scale across the range's ends, or an ordinal one round its
    /// values.
    Discrete {
        domain: Discrete,
        range: Vec<f64>,
        ordinal: bool,
    },
    Continuous {
        values: Arc<[f64]>,
        scale: Continuous,
    },
    /// A discretizing scale's steps, each a value of the range.
    Steps {
        values: Arc<[f64]>,
        steps: Steps,
        range: Vec<f64>,
    },
}

impl Amount {
    /// `channel` over `data` in a `mark` layer, onto its range or
    /// `default`. `size` for the size channel, which takes in zero.
    pub(super) fn of(
        channel: &Channel,
        data: &Data,
        default: [f32; 2],
        mark: Mark,
        size: bool,
    ) -> Option<Self> {
        let column = &channel.column(data)?;
        let role = match size {
            true => Role::Size,
            false => Role::Amount,
        };
        let kind = scales::scale_type(channel, role, mark, false);
        if kind == ScaleType::Identity {
            return Some(Amount::Identity(column.numbers()?));
        }
        let range = scales::numbers(&channel.scale, default)?;
        let reverse = channel.scale.reverse == Some(true);
        if kind.is_discrete() {
            let mut range = range;
            if reverse {
                range.reverse();
            }
            return Some(Amount::Discrete {
                domain: Discrete::of(column, &channel.sort).within(&channel.scale),
                range,
                ordinal: kind == ScaleType::Ordinal,
            });
        }
        let Column::Number(values) = column else {
            return None;
        };
        let spanned: Vec<f64> = channel
            .spanned(data)?
            .iter()
            .flat_map(|values| values.iter().copied())
            .collect();
        if kind.is_discretizing() {
            let count = match &channel.scale.range {
                Some(_) => range.len(),
                None => match (kind, &channel.scale.domain) {
                    (ScaleType::Threshold, Some(Domain::Numbers(cuts))) => cuts.len() + 1,
                    (ScaleType::Threshold, _) => 3,
                    _ => scales::STEP_COUNT,
                },
            };
            let mut range = match &channel.scale.range {
                Some(_) => range,
                None => (0..count)
                    .map(|i| {
                        let t = i as f64 / (count.max(2) - 1) as f64;
                        range[0] + (range[range.len() - 1] - range[0]) * t
                    })
                    .collect(),
            };
            if reverse {
                range.reverse();
            }
            let steps = Steps::of(kind, &channel.scale, &spanned, range.len())?;
            return Some(Amount::Steps {
                values: values.clone(),
                steps,
                range,
            });
        }
        let context = Context {
            role,
            mark,
            orient: None,
            both_measures: false,
        };
        let extent = domain::extent(spanned);
        Some(Amount::Continuous {
            values: values.clone(),
            scale: scales::continuous(channel, kind, &context, extent, range),
        })
    }

    /// Value `index` of `count` on a point scale across `range`'s ends, or
    /// round an ordinal one's values.
    fn step(index: usize, count: usize, range: &[f64], ordinal: bool) -> f32 {
        if ordinal {
            return range[index % range.len()] as f32;
        }
        let (first, last) = (range[0], range[range.len() - 1]);
        (match count {
            0 | 1 => (first + last) / 2.0,
            _ => first + (last - first) * index as f64 / (count - 1) as f64,
        }) as f32
    }

    /// `NaN` for a missing value, but where `shown` on a continuous scale:
    /// its zero, where its domain spans zero, or its domain's first value.
    pub(super) fn rows(&self, shown: bool) -> Vec<f32> {
        match self {
            Amount::Identity(values) => values.iter().map(|&v| v as f32).collect(),
            Amount::Discrete {
                domain,
                range,
                ordinal,
            } => domain
                .codes
                .iter()
                .map(|&code| match code {
                    domain::MISSING => f32::NAN,
                    code => Self::step(code as usize, domain.labels.len(), range, *ordinal),
                })
                .collect(),
            Amount::Continuous { values, scale, .. } => values
                .iter()
                .map(|&v| match v.is_nan() && shown {
                    true => scale.at(invalid::zero_or_min(&scale.domain)) as f32,
                    false => scale.at(v) as f32,
                })
                .collect(),
            Amount::Steps {
                values,
                steps,
                range,
                ..
            } => values
                .iter()
                .map(|&v| {
                    steps
                        .index(v)
                        .and_then(|i| range.get(i))
                        .map_or(f32::NAN, |r| *r as f32)
                })
                .collect(),
        }
    }

    /// What `value` maps to: a discrete domain's value by name, any
    /// other's number. `None` off the domain.
    pub(super) fn of_value(&self, value: &Datum) -> Option<f32> {
        let number = || match value {
            Datum::Number(number) => Some(*number),
            Datum::Text(text) => text.parse().ok(),
        };
        match self {
            Amount::Identity(_) => number().map(|v| v as f32),
            Amount::Discrete {
                domain,
                range,
                ordinal,
            } => {
                let name = match value {
                    Datum::Number(number) => JsNumber(*number).to_string().into(),
                    Datum::Text(text) => text.clone(),
                };
                let index = domain.labels.iter().position(|label| *label == name)?;
                Some(Self::step(index, domain.labels.len(), range, *ordinal))
            }
            Amount::Continuous { scale, .. } => Some(scale.at(number()?) as f32),
            Amount::Steps { steps, range, .. } => {
                range.get(steps.index(number()?)?).map(|&v| v as f32)
            }
        }
    }
}

/// A numeric channel's value for each row: a fixed value, or its field
/// through an [`Amount`]. A missing value takes `default`.
fn amounts(
    encode: Option<&Encode<f32>>,
    layer: &Layer,
    range: [f32; 2],
    size: bool,
    default: f32,
) -> Option<Each<f32>> {
    Some(match encode {
        None => Each::All(default),
        Some(Encode::Value(value)) => Each::All(*value),
        Some(Encode::Scaled(channel)) => {
            let rows = Amount::of(channel, &layer.data, range, layer.mark, size)?
                .rows(invalid::shown(layer));
            Each::Rows(
                rows.into_iter()
                    .map(|v| if v.is_nan() { default } else { v.max(0.0) })
                    .collect(),
            )
        }
    })
}

/// A shape or dash channel's values per row: each place in its range,
/// `range` the range at each place, round and round.
fn cycled<T: Clone>(
    channel: &Channel,
    layer: &Layer,
    range: impl Fn(usize) -> T,
    count: usize,
    missing: T,
) -> Option<Each<T>> {
    let kind = scales::scale_type(channel, Role::Symbol, layer.mark, false);
    let places = scales::places(channel, &channel.column(&layer.data)?, kind, count)?;
    Some(Each::Rows(
        places
            .into_iter()
            .map(|place| place.map_or(missing.clone(), &range))
            .collect(),
    ))
}

/// A fill or stroke channel: a fixed colour, or a field's values through
/// its colour scale.
fn inks(encode: Option<&Encode<Hsla>>, layer: &Layer) -> Option<Option<Each<Option<Ink>>>> {
    Some(match encode {
        None => None,
        Some(Encode::Value(color)) => Some(Each::All(Some(Ink::Fixed(*color)))),
        Some(Encode::Scaled(channel)) => Some(Each::Rows(paints(channel, layer)?)),
    })
}

/// Each row's colour through `channel`'s colour scale.
fn paints(channel: &Channel, layer: &Layer) -> Option<Vec<Option<Ink>>> {
    if let Some(rows) = scales::named_inks(channel, layer)? {
        return Some(rows);
    }
    Some(match ColorScale::over(channel, layer)? {
        (palette, Colored::Discrete(domain)) => domain
            .codes
            .iter()
            .map(|&code| match code {
                domain::MISSING => None,
                code => palette.discrete(code as usize),
            })
            .collect(),
        (palette, Colored::Numbers(values)) => {
            let shown = invalid::shown(layer);
            values
                .iter()
                .map(|&v| palette.value(if shown { palette.shown(v) } else { v }))
                .collect()
        }
    })
}

/// A field's values as text, a number or time read out, null as `null`.
pub(super) fn texts(channel: &Channel, column: &Column) -> Vec<SharedString> {
    match column {
        Column::Number(values) => values
            .iter()
            .map(|&v| channel.kind.describe(v).into())
            .collect(),
        column => (0..column.len()).map(|row| column.label(row)).collect(),
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
    /// `None` for a channel over a field `layer` lacks, or a range its
    /// channel does not take.
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
            // With no scale, each row's text is its lengths, separated by
            // commas.
            Some(Encode::Scaled(channel)) if channel.scale.kind == Some(ScaleType::Identity) => {
                Each::Rows(
                    texts(channel, &channel.column(&layer.data)?)
                        .iter()
                        .map(|text| {
                            let dash: Option<Vec<f32>> =
                                text.split(',').map(|n| n.trim().parse().ok()).collect();
                            solid(&dash?)
                        })
                        .collect(),
                )
            }
            Some(Encode::Scaled(channel)) => {
                let dashes = Dashes::of(channel)?;
                cycled(channel, layer, |i| dashes.nth(i), dashes.len(), None)?
            }
        };
        let shape = match (layer.mark, &e.shape) {
            (Mark::Circle, _) => Each::All(Shape::Circle),
            (Mark::Square, _) => Each::All(Shape::Square),
            (_, None) => Each::All(Shape::Circle),
            (_, Some(Encode::Value(shape))) => Each::All(shape.clone()),
            (_, Some(Encode::Scaled(channel)))
                if channel.scale.kind == Some(ScaleType::Identity) =>
            {
                Each::Rows(
                    texts(channel, &channel.column(&layer.data)?)
                        .into_iter()
                        .map(|name| name.parse().unwrap_or_default())
                        .collect(),
                )
            }
            (_, Some(Encode::Scaled(channel))) => {
                let shapes = Shapes::of(channel)?;
                cycled(
                    channel,
                    layer,
                    |i| shapes.nth(i),
                    shapes.len(),
                    Shape::Circle,
                )?
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
            Some(LineBreak::Pattern { source, flags }) => Some(RegExp::new(source, flags).ok()?),
            _ => None,
        };
        let broken = |text: SharedString| -> SharedString {
            match (&p.line_break, &regex) {
                (_, Some(regex)) => regex
                    .split(&text)
                    .into_iter()
                    .map(|piece| piece.unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("\n")
                    .into(),
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

/// A shape channel's range: its own, or Vega's symbols.
pub(super) enum Shapes {
    Symbols,
    Listed(Vec<Shape>),
}

impl Shapes {
    /// `None` for a range of something other than shapes.
    pub(super) fn of(channel: &Channel) -> Option<Self> {
        match &channel.scale.range {
            _ if channel.scale.kind == Some(ScaleType::Identity) => None,
            None | Some(Range::Named(NamedRange::Symbol)) => Some(Shapes::Symbols),
            Some(Range::Shapes(shapes)) if !shapes.is_empty() => {
                Some(Shapes::Listed(shapes.clone()))
            }
            Some(_) => None,
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Shapes::Symbols => Shape::SCHEME.len(),
            Shapes::Listed(shapes) => shapes.len(),
        }
    }

    /// The `index`-th, round and round.
    pub(super) fn nth(&self, index: usize) -> Shape {
        match self {
            Shapes::Symbols => Shape::nth(index),
            Shapes::Listed(shapes) => shapes[index % shapes.len()].clone(),
        }
    }
}

/// A dash channel's range: its own, or Vega-Lite's dashes.
pub(super) struct Dashes(Vec<Vec<f32>>);

impl Dashes {
    /// `None` for a range of something other than dashes.
    pub(super) fn of(channel: &Channel) -> Option<Self> {
        match &channel.scale.range {
            None => Some(Dashes(DASHES.iter().map(|dash| dash.to_vec()).collect())),
            Some(Range::Dashes(dashes)) if !dashes.is_empty() => Some(Dashes(dashes.clone())),
            Some(_) => None,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    /// The `index`-th, round and round. `None` is solid.
    pub(super) fn nth(&self, index: usize) -> Option<Arc<[f32]>> {
        solid(&self.0[index % self.0.len()])
    }
}

/// `None` for a dash with no gap.
pub(super) fn solid(dash: &[f32]) -> Option<Arc<[f32]>> {
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
                Column::Text(text) => text.get(a).cmp(&text.get(b)),
                Column::Value(values) => values[a].compare(&values[b]).unwrap_or(Ordering::Equal),
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
