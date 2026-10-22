//! What a guide ticks on a continuous scale, and how the ticks read: Vega's
//! `tickCount`, `tickValues`, `validTicks` and `tickFormat`.

use crate::model::{Format, TickCount};
use d3::{
    array,
    format::{NumberFormat, number::Number as JsNumber},
    scale::{Continuous, Transform},
    time::{Interval, date::LocalZone},
};
use gpui::SharedString;

/// How many ticks a continuous scale gives: about a count, or one on each
/// step of an interval on a time scale.
#[derive(Clone, Copy, Debug)]
pub enum Count {
    About(f64),
    Every(Interval),
}

impl Count {
    /// Vega's `tickCount(scale, count, minStep)`: at least one per bin edge
    /// of `edges`, and no more than steps of `min_step` fit. `None` for an
    /// interval on a scale not of time.
    pub fn of(
        scale: &Continuous,
        count: TickCount,
        min_step: Option<f64>,
        edges: usize,
    ) -> Option<Self> {
        let count = match count {
            TickCount::Every(unit, step) => {
                let Transform::Time(zone) = &scale.transform else {
                    return None;
                };
                return Some(Count::Every(Interval::every(unit, step, 0.0, zone)));
            }
            TickCount::Count(count) => count.max(edges as f64),
        };
        let Some(min_step) = min_step else {
            return Some(Count::About(count));
        };
        let (lo, hi) = ends(scale);
        let steps = match (hi - lo) / min_step {
            steps if steps == 0.0 || steps.is_nan() => 1.0,
            steps => steps,
        };
        let mut count = count.min(steps.floor() + 1.0);
        let plain = edges == 0
            && matches!(
                scale.transform,
                Transform::Linear | Transform::Pow { .. } | Transform::Symlog { .. }
            );
        if plain && lo < hi {
            while count > 1.0 && array::tick_step(lo, hi, count) < min_step {
                count -= 1.0;
            }
        }
        Some(Count::About(count))
    }

    /// The count `tickFormat` and a log scale's skipping read: an interval
    /// stands for Vega's default of 10.
    pub fn number(self) -> f64 {
        match self {
            Count::About(count) => count,
            Count::Every(_) => 10.0,
        }
    }
}

/// The least and greatest of a scale's domain ends.
fn ends(scale: &Continuous) -> (f64, f64) {
    match scale.domain.as_slice() {
        [first, .., last] => (first.min(*last), first.max(*last)),
        [only] => (*only, *only),
        [] => (f64::NAN, f64::NAN),
    }
}

/// Vega's `tickValues`: `scale`'s ticks, or the edges of its bins it shows.
pub fn values(scale: &Continuous, count: Count, edges: Option<&[f64]>) -> Vec<f64> {
    match (edges, count) {
        (Some(edges), count) => valid(scale, edges.to_vec(), count.number()),
        (None, Count::About(count)) => scale.ticks(count),
        (None, Count::Every(interval)) => {
            let zone = match &scale.transform {
                Transform::Time(zone) => zone.clone(),
                _ => LocalZone::utc(),
            };
            let (first, last) = (scale.domain[0], scale.domain[scale.domain.len() - 1]);
            let (lo, hi) = ends(scale);
            let mut ticks = interval.range(lo, hi + 1.0, 1.0, &zone);
            if last < first {
                ticks.reverse();
            }
            ticks
        }
    }
}

/// Vega's `validTicks`: those of `values` inside the scale's range, in the
/// range's order, every other one dropped until at most `count` are left,
/// or else the first and last.
pub fn valid(scale: &Continuous, values: Vec<f64>, count: f64) -> Vec<f64> {
    let (r0, r1) = (scale.range[0], scale.range[scale.range.len() - 1]);
    let (lo, hi) = (r0.min(r1).floor(), r0.max(r1).ceil());
    let mut placed: Vec<(f64, f64)> = values
        .into_iter()
        .map(|v| (v, scale.at(v)))
        .filter(|(_, at)| lo <= *at && *at <= hi)
        .collect();
    placed.sort_by(|a, b| match r1 < r0 {
        true => b.1.total_cmp(&a.1),
        false => a.1.total_cmp(&b.1),
    });
    let mut ticks: Vec<f64> = placed.into_iter().map(|(v, _)| v).collect();
    if count > 0.0 && ticks.len() > 1 {
        let ends = [ticks[0], ticks[ticks.len() - 1]];
        while ticks.len() as f64 > count && ticks.len() >= 3 {
            ticks = ticks.into_iter().step_by(2).collect();
        }
        if ticks.len() < 3 {
            ticks = ends.to_vec();
        }
    }
    ticks
}

/// Vega's `tickFormat` on a continuous scale.
#[derive(Clone, Debug)]
pub enum TickFormat {
    Number(NumberFormat),
    /// A log scale's, which writes nothing for a tick d3's log format skips:
    /// one whose leading digit is past `most`.
    Log {
        format: NumberFormat,
        base: f64,
        most: f64,
    },
    /// A d3 time format on the UTC calendar, or Vega's multi-format.
    Time(Option<SharedString>),
    /// JavaScript's `String`.
    Plain,
}

impl TickFormat {
    /// How `count` ticks on `scale` read: as `format` says, or Vega's own
    /// for the scale: a time scale's multi-format, a log scale's float of
    /// 12 digits skipping some of `ticks` unless they were `given`, any
    /// other's `,f` at the precision the tick step needs.
    pub fn of(
        scale: &Continuous,
        count: f64,
        format: Option<&Format>,
        ticks: usize,
        given: bool,
    ) -> Self {
        let specifier = match format {
            Some(Format::Time(time)) => return TickFormat::Time(Some(time.clone())),
            Some(Format::Number(specifier)) => Some(*specifier),
            None => None,
        };
        match &scale.transform {
            Transform::Time(_) => TickFormat::Time(specifier.map(|s| s.to_string().into())),
            Transform::Log { base } => {
                let format = NumberFormat::float(specifier);
                match given || ticks == 0 {
                    true => TickFormat::Number(format),
                    false => TickFormat::Log {
                        format,
                        base: *base,
                        most: (base * count / ticks as f64).max(1.0),
                    },
                }
            }
            _ => {
                let (first, last) = (scale.domain[0], scale.domain[scale.domain.len() - 1]);
                TickFormat::Number(NumberFormat::span(first, last, count, specifier))
            }
        }
    }

    /// `value` as it reads.
    pub fn label(&self, value: f64) -> String {
        match self {
            TickFormat::Number(format) => format.format(value),
            TickFormat::Log { format, base, most } => {
                let power = base.powf((value.abs().ln() / base.ln()).round());
                let mut lead = value.abs() / power;
                if lead * base < base - 0.5 {
                    lead *= base;
                }
                match lead <= *most {
                    true => format.format(value),
                    false => String::new(),
                }
            }
            TickFormat::Plain => JsNumber(value).to_string(),
            TickFormat::Time(specifier) => {
                let zone = LocalZone::utc();
                let date = zone.date(value);
                match specifier {
                    Some(specifier) => date.format(specifier),
                    None => date.multi_format(),
                }
            }
        }
    }
}

/// Tick values and their labels.
#[derive(Clone, Debug, PartialEq)]
pub struct Ticks {
    pub values: Vec<f64>,
    pub labels: Vec<SharedString>,
}

impl Ticks {
    /// About `count` of `scale`'s ticks, read as Vega reads them with no
    /// format.
    pub fn of(scale: &Continuous, count: f64) -> Self {
        let values = scale.ticks(count);
        let format = TickFormat::of(scale, count, None, values.len(), false);
        let labels = values.iter().map(|&v| format.label(v).into()).collect();
        Self { values, labels }
    }
}
