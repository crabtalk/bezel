//! Each channel's scale as Vega-Lite defaults it and Vega builds it: its
//! type, domain and range for positions, numbers, colours, shapes and
//! dashes, through bezel-js's d3-scale.

use crate::{
    color,
    data::Column,
    model::{
        Channel, Domain, Kind, Layer, Mark, NamedRange, Nice, Orient, Position, Range, Scale,
        ScaleType, Scheme,
    },
    plan::{
        Ink,
        domain::{self, Discrete},
    },
    scale::Ticks,
};
use gpui::SharedString;
use js::{
    color::Rgb,
    date::LocalZone,
    interpolate::Interpolate,
    scale::{
        Band, BinOrdinal, Continuous, Diverging, Quantile, Quantize, Sequential, Threshold,
        Transform,
    },
    scheme,
    time::Interval,
};
use std::sync::Arc;

/// Vega's `DEFAULT_COUNT`: colours a quantile or quantize scale takes from a
/// scheme with no count.
const SCHEME_COUNT: usize = 5;
/// Vega-Lite's `quantileCount` and `quantizeCount`, for a numeric range.
pub(super) const STEP_COUNT: usize = 4;
/// Vega-Lite's `pointPadding`.
const POINT_PADDING: f64 = 0.5;
/// Vega-Lite's band paddings within a band, by mark, and with an offset
/// nested in it.
const BAR_PADDING: f64 = 0.1;
const TICK_PADDING: f64 = 0.25;
const NESTED_PADDING: f64 = 0.2;

/// A Vega-Lite scale channel, as far as its defaults differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    X,
    Y,
    Offset,
    Theta,
    Radius,
    Color,
    Size,
    /// Opacity, stroke width and angle.
    Amount,
    /// Shape and stroke dash.
    Symbol,
}

/// The scale type Vega-Lite gives `channel` on `role` in a `mark` layer: its
/// own where the channel and field type take it, else the default. `nested`
/// for a position with an offset in its bands.
pub(super) fn scale_type(channel: &Channel, role: Role, mark: Mark, nested: bool) -> ScaleType {
    let kind = channel.kind;
    let default = match kind {
        Kind::Nominal | Kind::Ordinal => match role {
            Role::Color | Role::Symbol => ScaleType::Ordinal,
            Role::X | Role::Y | Role::Offset
                if nested
                    || matches!(
                        mark,
                        Mark::Rect | Mark::Bar | Mark::Image | Mark::Rule | Mark::Tick
                    ) =>
            {
                ScaleType::Band
            }
            Role::Theta | Role::Radius if mark == Mark::Arc => ScaleType::Band,
            _ => ScaleType::Point,
        },
        Kind::Temporal => match role {
            Role::Symbol => ScaleType::Ordinal,
            _ => ScaleType::Time,
        },
        Kind::Quantitative => match role {
            Role::Symbol => ScaleType::Ordinal,
            Role::Color if channel.scale.bins.is_some() => ScaleType::BinOrdinal,
            _ => ScaleType::Linear,
        },
    };
    let Some(written) = channel.scale.kind else {
        return default;
    };
    if written == ScaleType::Identity {
        return written;
    }
    let channel_takes = match role {
        Role::X | Role::Y | Role::Offset | Role::Theta | Role::Radius => {
            written.is_continuous()
                || written == ScaleType::Band
                || (written == ScaleType::Point && !nested)
        }
        Role::Size | Role::Amount => {
            written.is_continuous()
                || written.is_discretizing() && written != ScaleType::BinOrdinal
                || written.is_discrete()
        }
        Role::Color => written != ScaleType::Band,
        Role::Symbol => {
            written == ScaleType::Ordinal
                || written.is_discretizing() && written != ScaleType::BinOrdinal
        }
    };
    let field_takes = match kind {
        Kind::Nominal | Kind::Ordinal => written.is_discrete() || written == ScaleType::BinOrdinal,
        Kind::Temporal => matches!(written, ScaleType::Time | ScaleType::Utc),
        Kind::Quantitative => {
            written.is_continuous() && !matches!(written, ScaleType::Time | ScaleType::Utc)
                || written.is_discretizing() && written != ScaleType::BinOrdinal
                || written == ScaleType::BinOrdinal && channel.scale.bins.is_some()
        }
    };
    match channel_takes && field_takes {
        true => written,
        false => default,
    }
}

/// Whether `channel` on a `kind` scale takes its values as its domain, one
/// colour each: an ordinal, band or point scale, or bin-ordinal over a field
/// with no bins.
pub(super) fn listed(kind: ScaleType, channel: &Channel) -> bool {
    kind.is_discrete() || (kind == ScaleType::BinOrdinal && channel.scale.bins.is_none())
}

/// Bezel reads every time on the UTC calendar, so `time` and `utc` are one.
fn transform(kind: ScaleType, scale: &Scale) -> Transform {
    match kind {
        ScaleType::Log => Transform::Log {
            base: scale.base.unwrap_or(10.0),
        },
        ScaleType::Pow => Transform::Pow {
            exponent: scale.exponent.unwrap_or(1.0),
        },
        ScaleType::Sqrt => Transform::Pow { exponent: 0.5 },
        ScaleType::Symlog => Transform::Symlog {
            constant: scale.constant.unwrap_or(1.0),
        },
        ScaleType::Time | ScaleType::Utc => Transform::Time(LocalZone::utc()),
        _ => Transform::Linear,
    }
}

/// What Vega-Lite's `zero` rule needs to know of a channel's layer.
#[derive(Clone, Copy, Debug)]
pub(super) struct Context {
    pub(super) role: Role,
    pub(super) mark: Mark,
    pub(super) orient: Option<Orient>,
    /// Whether x and y are both quantitative, which makes a bar, area, line
    /// or trail vertical.
    pub(super) both_measures: bool,
}

impl Context {
    /// Vega-Lite's `zero` for a continuous scale with no domain of its own.
    fn zero(&self, channel: &Channel, kind: ScaleType) -> bool {
        if matches!(
            kind,
            ScaleType::Log
                | ScaleType::Time
                | ScaleType::Utc
                | ScaleType::Quantile
                | ScaleType::Threshold
        ) {
            return false;
        }
        if let Some(zero) = channel.scale.zero {
            return zero;
        }
        if matches!(channel.scale.domain, Some(Domain::Numbers(_))) {
            return false;
        }
        let binned = channel.scale.bins.is_some();
        match self.role {
            Role::Size => channel.kind == Kind::Quantitative && !kind.is_discretizing(),
            Role::X | Role::Y | Role::Theta | Role::Radius if binned => false,
            Role::X | Role::Y | Role::Theta | Role::Radius => {
                let paths = matches!(self.mark, Mark::Bar | Mark::Area | Mark::Line | Mark::Trail);
                let orient = self
                    .orient
                    .or(self.both_measures.then_some(Orient::Vertical));
                let dimension = match self.role {
                    Role::X => orient == Some(Orient::Vertical),
                    Role::Y => orient == Some(Orient::Horizontal),
                    _ => false,
                };
                !(paths && dimension)
            }
            _ => false,
        }
    }
}

/// A continuous scale over values spanning `extent`, its bins' start to
/// stop where it has bins, onto `range`, as Vega-Lite defaults and Vega
/// fits it: zero taken in, the domain's middle spliced in, padded by pixels
/// on x or y, and nice.
pub(super) fn continuous(
    channel: &Channel,
    kind: ScaleType,
    context: &Context,
    extent: (f64, f64),
    range: Vec<f64>,
) -> Continuous {
    let scale = &channel.scale;
    let mut domain = match (&scale.domain, scale.bins) {
        (Some(Domain::Numbers(values)), _) if values.len() >= 2 => values.clone(),
        (_, Some(bins)) if bins.start.is_finite() && bins.stop.is_finite() => {
            vec![bins.start, bins.stop]
        }
        _ => vec![extent.0, extent.1],
    };
    let n = domain.len() - 1;
    if context.zero(channel, kind) {
        domain[0] = domain[0].min(0.0);
        domain[n] = domain[n].max(0.0);
    }
    domain[0] = scale.domain_min.unwrap_or(domain[0]);
    domain[n] = scale.domain_max.unwrap_or(domain[n]);
    if let Some(mid) = scale.domain_mid {
        let at = if mid > domain[n] {
            n + 1
        } else if mid < domain[0] {
            0
        } else {
            n
        };
        domain.insert(at, mid);
    }
    let range = match scale.reverse == Some(true) {
        true => range.into_iter().rev().collect(),
        false => range,
    };
    let mut out = Continuous {
        clamp: scale.clamp,
        round: scale.round,
        ..Continuous::new(transform(kind, scale), domain, range)
    };
    let position = matches!(context.role, Role::X | Role::Y);
    if let Some(pixels) = scale.padding.filter(|p| position && *p > 0.0)
        && out.domain[0] != out.domain[out.domain.len() - 1]
    {
        out.pad(pixels);
    }
    let nice = match scale.nice {
        Some(nice) => nice,
        None if position
            && !matches!(scale.domain, Some(Domain::Numbers(_)))
            && scale.bins.is_none()
            && scale.domain_min.is_none()
            && scale.domain_max.is_none()
            && !matches!(kind, ScaleType::Time | ScaleType::Utc) =>
        {
            Nice::Ticks(10.0)
        }
        None => Nice::Off,
    };
    match nice {
        Nice::Ticks(count) => out.nice(count),
        Nice::Interval { unit, step } => {
            if let Transform::Time(zone) = &out.transform {
                let interval = Interval::every(unit, step, 0.0, zone);
                out.nice_to(&interval);
            }
        }
        Nice::Off => {}
    }
    out
}

/// A band or point scale of `count` values across `range` with Vega-Lite's
/// paddings for `kind` in a `mark` layer: `nested` with an offset in its
/// bands, `offset` for the offset's own.
pub(super) fn band(
    channel: &Channel,
    kind: ScaleType,
    mark: Mark,
    nested: bool,
    offset: bool,
    count: usize,
    range: [f64; 2],
) -> Band {
    let scale = &channel.scale;
    let range = match scale.reverse == Some(true) {
        true => [range[1], range[0]],
        false => range,
    };
    let mut band = match kind {
        ScaleType::Point => Band::point(
            count,
            range,
            scale
                .padding
                .or(scale.padding_outer)
                .unwrap_or(match offset {
                    true => 0.5,
                    false => POINT_PADDING,
                }),
        ),
        _ => {
            let inner = match (offset, nested) {
                (true, _) => 0.0,
                (false, true) => NESTED_PADDING,
                (false, false) => match mark {
                    Mark::Bar => BAR_PADDING,
                    Mark::Tick => TICK_PADDING,
                    _ => 0.0,
                },
            };
            let inner = scale.padding.or(scale.padding_inner).unwrap_or(inner);
            let outer = match (offset, nested) {
                (true, _) => 0.0,
                (false, true) => NESTED_PADDING,
                (false, false) => inner / 2.0,
            };
            Band {
                padding_inner: inner,
                padding_outer: scale.padding.or(scale.padding_outer).unwrap_or(outer),
                ..Band::new(count, range)
            }
        }
    };
    band.round = scale.round;
    band
}

/// The scheme a named range stands for in Vega's default config, and the
/// part of it taken. `None` for the theme's categorical colours.
fn named(range: NamedRange) -> Option<Scheme> {
    let scheme = |name: &str| -> Scheme {
        name.parse::<scheme::Scheme>()
            .expect("a scheme Vega registers")
            .into()
    };
    match range {
        NamedRange::Category | NamedRange::Symbol => None,
        NamedRange::Ordinal | NamedRange::Ramp => Some(scheme("blues")),
        NamedRange::Heatmap => Some(scheme("yellowgreenblue")),
        NamedRange::Diverging => Some(Scheme {
            extent: Some([1.0, 0.0]),
            ..scheme("blueorange")
        }),
    }
}

/// Where a colour scale's colours come from.
#[derive(Clone, Debug)]
enum Source {
    /// The theme's categorical colours.
    Theme,
    Scheme(Scheme),
    Colors(Vec<Rgb>),
}

/// How a continuous colour runs from the start of its range to the end.
#[derive(Clone, Debug)]
pub(super) enum Ramp {
    Scheme {
        scheme: Scheme,
        reverse: bool,
        interpolate: Interpolate,
    },
    Colors(Vec<Rgb>, Interpolate),
}

impl Ramp {
    fn at(&self, t: f64) -> Rgb {
        match self {
            Ramp::Scheme {
                scheme,
                reverse,
                interpolate,
            } => scheme.at(t, *reverse, *interpolate),
            Ramp::Colors(colors, interpolate) => interpolate.piecewise(colors, t),
        }
    }
}

/// A continuous colour scale's map from a value to its place in the ramp.
#[derive(Clone, Debug)]
pub(super) enum Fraction {
    Sequential(Sequential),
    Diverging(Diverging),
    /// A time scale onto evenly spread places in the ramp, its colours a
    /// scheme's sampled once for each of its domain's values, or its range's.
    Piecewise(Continuous),
}

/// What a value of a discretizing scale indexes.
#[derive(Clone, Debug)]
pub(super) enum Steps {
    Quantile(Quantile),
    Quantize(Quantize),
    Threshold(Threshold),
    /// A binned field's bins, by their edges.
    BinOrdinal(BinOrdinal),
}

impl Steps {
    /// `kind`'s steps over `values` into `count`, `domain` a quantize
    /// scale's ends or a threshold scale's thresholds.
    pub(super) fn of(kind: ScaleType, scale: &Scale, values: &[f64], count: usize) -> Option<Self> {
        let written = match &scale.domain {
            Some(Domain::Numbers(numbers)) => Some(numbers.clone()),
            _ => None,
        };
        Some(match kind {
            ScaleType::Quantile => Steps::Quantile(Quantile::new(values, count)),
            ScaleType::Quantize => {
                let (lo, hi) = domain::extent(values.iter().copied());
                let ends = written.unwrap_or(vec![lo, hi]);
                let zero = scale.zero == Some(true);
                let (mut lo, mut hi) = (ends[0], ends[ends.len() - 1]);
                if zero {
                    (lo, hi) = (lo.min(0.0), hi.max(0.0));
                }
                lo = scale.domain_min.unwrap_or(lo);
                hi = scale.domain_max.unwrap_or(hi);
                let mut quantize = Quantize::new([lo, hi], count);
                if let Some(Nice::Ticks(ticks)) = scale.nice {
                    let mut linear =
                        Continuous::new(Transform::Linear, vec![lo, hi], vec![0.0, 1.0]);
                    linear.nice(ticks);
                    quantize = Quantize::new([linear.domain[0], linear.domain[1]], count);
                }
                Steps::Quantize(quantize)
            }
            ScaleType::Threshold => Steps::Threshold(Threshold {
                domain: written.unwrap_or_default(),
                count,
            }),
            ScaleType::BinOrdinal => Steps::BinOrdinal(BinOrdinal {
                bins: edges(scale)?,
                count,
            }),
            _ => return None,
        })
    }

    pub(super) fn index(&self, x: f64) -> Option<usize> {
        match self {
            Steps::Quantile(scale) => scale.index(x),
            Steps::Quantize(scale) => scale.index(x),
            Steps::Threshold(scale) => scale.index(x),
            Steps::BinOrdinal(scale) => scale.index(x),
        }
    }

    /// Each step's label: the values it takes in.
    pub(super) fn labels(&self, kind: Kind, count: usize) -> Vec<SharedString> {
        if let Steps::BinOrdinal(scale) = self {
            return ranges(kind, &scale.bins);
        }
        let cuts: &[f64] = match self {
            Steps::Quantile(scale) => &scale.thresholds,
            Steps::Quantize(scale) => &scale.thresholds,
            Steps::Threshold(scale) => &scale.domain,
            Steps::BinOrdinal(scale) => &scale.bins,
        };
        let cuts = &cuts[..cuts.len().min(count.saturating_sub(1))];
        (0..=cuts.len())
            .map(|i| {
                let text = match (i.checked_sub(1).map(|i| cuts[i]), cuts.get(i)) {
                    (None, Some(hi)) => format!("< {}", kind.describe(*hi)),
                    (Some(lo), None) => format!("≥ {}", kind.describe(lo)),
                    (Some(lo), Some(hi)) => {
                        format!("{} – {}", kind.describe(lo), kind.describe(*hi))
                    }
                    (None, None) => String::new(),
                };
                text.into()
            })
            .collect()
    }

    /// How many values its range holds by default: a threshold scale's
    /// one more than its thresholds.
    fn count(kind: ScaleType, scale: &Scale, default: usize) -> usize {
        match (kind, &scale.domain) {
            (ScaleType::Threshold, Some(Domain::Numbers(cuts))) => cuts.len() + 1,
            (ScaleType::Threshold, _) => 3,
            (ScaleType::BinOrdinal, _) => edges(scale).map_or(0, |edges| edges.len() - 1),
            _ => default,
        }
    }
}

/// The edges of a scale's bins, as Vega's `bins` lists them: from their start
/// to their stop. `None` without bins that start and stop.
pub(super) fn edges(scale: &Scale) -> Option<Vec<f64>> {
    let bins = scale
        .bins
        .filter(|b| b.start.is_finite() && b.stop.is_finite())?;
    Some(bins.edges(bins.start, bins.stop)).filter(|edges| edges.len() >= 2)
}

/// Each bin between `edges` as Vega's symbol legend labels it.
pub(super) fn ranges(kind: Kind, edges: &[f64]) -> Vec<SharedString> {
    edges
        .windows(2)
        .map(|pair| format!("{} – {}", kind.describe(pair[0]), kind.describe(pair[1])).into())
        .collect()
}

/// A colour channel's scale.
#[derive(Clone, Debug)]
pub(super) enum ColorScale {
    /// One ink per discrete value, in domain order, cycled.
    Discrete(Vec<Ink>),
    Continuous {
        fraction: Fraction,
        ramp: Ramp,
    },
    Steps {
        steps: Steps,
        inks: Vec<Ink>,
    },
}

/// Each row's colour where `channel` names it rather than scaling to it: its
/// own value with no scale, or its range field's, which Vega-Lite asks to
/// be one to one with the channel's field. `None` where it scales.
pub(super) fn named_inks(channel: &Channel, layer: &Layer) -> Option<Option<Vec<Option<Ink>>>> {
    let column = match &channel.scale.range {
        _ if channel.scale.kind == Some(ScaleType::Identity) => channel.column(&layer.data)?,
        Some(Range::Field(field)) => layer.data.column(field)?.clone(),
        _ => return Some(None),
    };
    let Column::Text(text) = column else {
        return None;
    };
    Some(Some(
        (0..text.codes.len())
            .map(|row| {
                let name = text.get(row);
                Some(Ink::Fixed(color::hsla(name.parse().ok()?)))
            })
            .collect(),
    ))
}

/// What a colour channel reads in a layer.
pub(super) enum Colored {
    Discrete(Discrete),
    Numbers(Arc<[f64]>),
}

/// A legend entry: its ink and label.
pub(super) type Entry = (Ink, SharedString);

impl ColorScale {
    /// The colours `channel` takes in a `mark` layer: `discrete` its domain
    /// where it is discrete, `values` every value it colours where not. A
    /// nominal field with no `range` or `scheme`, or the `category` range,
    /// takes the theme's categorical colours; any other takes its own, or
    /// Vega's default scheme. `None` for a range of something other than
    /// colours.
    pub(super) fn of(
        channel: &Channel,
        mark: Mark,
        discrete: Option<usize>,
        values: &[f64],
    ) -> Option<Self> {
        let scale = &channel.scale;
        let kind = scale_type(channel, Role::Color, mark, false);
        let reverse = scale.reverse == Some(true);
        let source = match (&scale.scheme, &scale.range) {
            (Some(scheme), _) => Source::Scheme(scheme.clone()),
            (None, Some(Range::Colors(colors))) => {
                Source::Colors(colors.iter().copied().map(color::rgb).collect())
            }
            (None, Some(Range::Named(name))) => named(*name).map_or(Source::Theme, Source::Scheme),
            (None, Some(_)) => return None,
            (None, None) => {
                let name = match kind {
                    ScaleType::Ordinal if channel.kind == Kind::Nominal => NamedRange::Category,
                    ScaleType::Ordinal => NamedRange::Ordinal,
                    _ if scale.domain_mid.is_some() => NamedRange::Diverging,
                    _ if mark == Mark::Rect => NamedRange::Heatmap,
                    _ => NamedRange::Ramp,
                };
                named(name).map_or(Source::Theme, Source::Scheme)
            }
        };
        let interpolate = scale.interpolate.unwrap_or(match channel.kind {
            Kind::Nominal => Interpolate::Rgb { gamma: 1.0 },
            _ => Interpolate::Hcl,
        });
        let list = |count: usize, ordinal: bool| -> Vec<Ink> {
            let mut inks: Vec<Ink> = match &source {
                Source::Theme => (0..count).map(Ink::Category).collect(),
                Source::Scheme(scheme) => {
                    let count = scheme.count.unwrap_or(count);
                    scheme
                        .discrete(count, ordinal, interpolate)
                        .into_iter()
                        .map(|c| Ink::Fixed(color::hsla(c)))
                        .collect()
                }
                Source::Colors(colors) => {
                    colors.iter().map(|c| Ink::Fixed(color::hsla(*c))).collect()
                }
            };
            if reverse {
                inks.reverse();
            }
            inks
        };
        if listed(kind, channel) {
            let count = discrete?;
            return Some(ColorScale::Discrete(list(
                match count {
                    0 => SCHEME_COUNT,
                    count => count,
                },
                true,
            )));
        }
        if kind.is_discretizing() {
            let count = Steps::count(kind, scale, SCHEME_COUNT);
            let inks = list(count, false);
            let steps = Steps::of(kind, scale, values, inks.len())?;
            return Some(ColorScale::Steps { steps, inks });
        }
        // The theme has no ramp: a continuous scale on the categorical range
        // takes Vega's.
        let source = match source {
            Source::Theme => Source::Scheme(
                "tableau10"
                    .parse::<scheme::Scheme>()
                    .expect("a scheme Vega registers")
                    .into(),
            ),
            source => source,
        };
        let (lo, hi) = domain::extent(values.iter().copied());
        let context = Context {
            role: Role::Color,
            mark,
            orient: None,
            both_measures: false,
        };
        let shape = continuous(channel, kind, &context, (lo, hi), vec![0.0, 1.0]);
        let colors = |count: usize| -> Vec<Rgb> {
            let mut colors = match &source {
                Source::Theme => Vec::new(),
                Source::Scheme(scheme) => scheme.colors(count, interpolate),
                Source::Colors(colors) => colors.clone(),
            };
            if reverse {
                colors.reverse();
            }
            colors
        };
        if matches!(kind, ScaleType::Time | ScaleType::Utc) {
            let colors = colors(shape.domain.len());
            let last = colors.len().checked_sub(1)?;
            let places = (0..=last).map(|i| i as f64 / last.max(1) as f64).collect();
            return Some(ColorScale::Continuous {
                fraction: Fraction::Piecewise(Continuous {
                    range: places,
                    ..shape
                }),
                ramp: Ramp::Colors(colors, interpolate),
            });
        }
        let ramp = match source {
            Source::Scheme(scheme) if scheme.is_continuous() => Ramp::Scheme {
                scheme,
                reverse,
                interpolate,
            },
            _ => Ramp::Colors(colors(usize::MAX), interpolate),
        };
        let fraction = match shape.domain[..] {
            [a, b, c, ..] => Fraction::Diverging(Diverging {
                transform: shape.transform,
                domain: [a, b, c],
                clamp: scale.clamp,
            }),
            [a, b] => Fraction::Sequential(Sequential {
                transform: shape.transform,
                domain: [a, b],
                clamp: scale.clamp,
            }),
            _ => return None,
        };
        Some(ColorScale::Continuous { fraction, ramp })
    }

    /// `channel`'s colours over one layer's data, and what it reads there.
    pub(super) fn over(channel: &Channel, layer: &Layer) -> Option<(Self, Colored)> {
        let kind = scale_type(channel, Role::Color, layer.mark, false);
        let column = channel.column(&layer.data)?;
        if listed(kind, channel) {
            let domain = Discrete::of(&column, &channel.sort).within(&channel.scale);
            let palette = Self::of(channel, layer.mark, Some(domain.labels.len()), &[])?;
            return Some((palette, Colored::Discrete(domain)));
        }
        let Column::Number(values) = column else {
            return None;
        };
        let palette = Self::of(channel, layer.mark, None, &values)?;
        Some((palette, Colored::Numbers(values)))
    }

    /// The ink of discrete value `code`.
    pub(super) fn discrete(&self, code: usize) -> Option<Ink> {
        match self {
            ColorScale::Discrete(inks) if !inks.is_empty() => Some(inks[code % inks.len()]),
            _ => None,
        }
    }

    /// The ink of `value` on a continuous or discretizing scale.
    pub(super) fn value(&self, value: f64) -> Option<Ink> {
        match self {
            ColorScale::Discrete(_) => None,
            ColorScale::Steps { steps, inks } => Some(*inks.get(steps.index(value)?)?),
            ColorScale::Continuous { fraction, ramp } => {
                let shade = match fraction {
                    Fraction::Sequential(scale) => ramp.at(scale.at(value)),
                    Fraction::Diverging(scale) => ramp.at(scale.at(value)),
                    Fraction::Piecewise(scale) => ramp.at(scale.at(value)),
                };
                (!value.is_nan()).then(|| Ink::Fixed(color::hsla(shade)))
            }
        }
    }

    /// A continuous or discretizing scale's legend: ticks across the domain
    /// in their colours, or each step.
    pub(super) fn entries(&self, kind: Kind, count: usize) -> Vec<Entry> {
        match self {
            ColorScale::Discrete(_) => Vec::new(),
            ColorScale::Steps { steps, inks } => steps
                .labels(kind, inks.len())
                .into_iter()
                .zip(inks)
                .map(|(label, ink)| (*ink, label))
                .collect(),
            ColorScale::Continuous { fraction, .. } => {
                let scale = match fraction {
                    Fraction::Sequential(scale) => Continuous::new(
                        scale.transform.clone(),
                        scale.domain.to_vec(),
                        vec![0.0, 1.0],
                    ),
                    Fraction::Diverging(scale) => Continuous::new(
                        scale.transform.clone(),
                        vec![scale.domain[0], scale.domain[2]],
                        vec![0.0, 1.0],
                    ),
                    Fraction::Piecewise(scale) => scale.clone(),
                };
                let ticks = Ticks::of(&scale, count);
                ticks
                    .values
                    .iter()
                    .filter_map(|&v| Some((self.value(v)?, kind.describe(v).into())))
                    .collect()
            }
        }
    }
}

/// Where each row of a discrete range's channel falls in its range: a
/// discrete field's place in its domain, or a discretizing scale's step.
/// `count` is the range's length, `kind` the scale type.
pub(super) fn places(
    channel: &Channel,
    column: &Column,
    kind: ScaleType,
    count: usize,
) -> Option<Vec<Option<usize>>> {
    if kind.is_discretizing() {
        let Column::Number(values) = column else {
            return None;
        };
        let steps = Steps::of(kind, &channel.scale, values, count)?;
        return Some(values.iter().map(|&v| steps.index(v)).collect());
    }
    let domain = Discrete::of(column, &channel.sort).within(&channel.scale);
    Some(
        domain
            .codes
            .iter()
            .map(|&code| (code != domain::MISSING).then_some(code as usize))
            .collect(),
    )
}

/// A numeric channel's range: its own, or `default`.
pub(super) fn numbers(scale: &Scale, default: [f32; 2]) -> Option<Vec<f64>> {
    match &scale.range {
        None => Some(vec![
            scale.range_min.unwrap_or(f64::from(default[0])),
            scale.range_max.unwrap_or(f64::from(default[1])),
        ]),
        Some(Range::Numbers(numbers)) if !numbers.is_empty() => Some(numbers.clone()),
        Some(_) => None,
    }
}

/// An x or y range of positions, in pixels from the plot's edge `size`
/// long. `None` for a range of something else.
pub(super) fn positions(scale: &Scale, size: f32) -> Option<Option<Vec<f64>>> {
    match &scale.range {
        None => Some(None),
        Some(Range::Positions(positions)) if positions.len() >= 2 => Some(Some(
            positions
                .iter()
                .map(|p: &Position| f64::from(p.pixels(size)))
                .collect(),
        )),
        Some(Range::Numbers(numbers)) if numbers.len() >= 2 => Some(Some(numbers.clone())),
        Some(_) => None,
    }
}
