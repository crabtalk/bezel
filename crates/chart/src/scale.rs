//! The ticks that label a continuous scale, and a value read out.

use gpui::SharedString;
use js::{
    array,
    scale::{Continuous, Transform},
    time as calendar,
};

use crate::{time, transform::Bins};

/// Tick values and their labels.
#[derive(Clone, Debug, PartialEq)]
pub struct Ticks {
    pub values: Vec<f64>,
    pub labels: Vec<SharedString>,
}

impl Ticks {
    /// About `count` of `scale`'s ticks. A time shows the calendar fields
    /// its interval steps through; a log scale's tick is read out whole; any
    /// other number shows as many decimals as the tick step needs, and k, M,
    /// G or T once the domain reaches ten thousand.
    pub fn of(scale: &Continuous, count: usize) -> Self {
        Self::labelled(scale, scale.ticks(count as f64), count)
    }

    /// Vega's ticks on a scale with `bins`: each edge inside the range,
    /// labelled for at least as many ticks as there are edges.
    pub fn binned(scale: &Continuous, bins: Bins, count: usize) -> Self {
        let (lo, hi) = (scale.domain[0], scale.domain[scale.domain.len() - 1]);
        let edges = bins.edges(lo, hi);
        let count = count.max(edges.len());
        let (r0, r1) = (scale.range[0], scale.range[scale.range.len() - 1]);
        let (r0, r1) = (r0.min(r1).floor(), r0.max(r1).ceil());
        let values = edges
            .into_iter()
            .filter(|&v| (r0..=r1).contains(&scale.at(v)))
            .collect();
        Self::labelled(scale, values, count)
    }

    fn labelled(scale: &Continuous, values: Vec<f64>, count: usize) -> Self {
        let (d0, d1) = match scale.domain.as_slice() {
            [first, .., last] => (first.min(*last), first.max(*last)),
            [only] => (*only, *only),
            [] => (f64::NAN, f64::NAN),
        };
        let labels = match &scale.transform {
            Transform::Time(zone) => {
                let unit = calendar::tick_interval(d0, d1, count as f64, zone)
                    .map_or(time::Unit::Day, |interval| interval.unit.into());
                values
                    .iter()
                    .map(|&ms| time::label(ms, unit).into())
                    .collect()
            }
            Transform::Log { .. } => values.iter().map(|&v| value(v).into()).collect(),
            _ => {
                let increment = array::tick_increment(d0, d1, count as f64);
                let power = match increment > 0.0 {
                    true => increment.log10().floor(),
                    false => -(-increment).log10().ceil(),
                };
                let magnitude = d0.abs().max(d1.abs());
                values
                    .iter()
                    .map(|&v| match power.is_finite() {
                        true => number(v, power as i32, magnitude).into(),
                        false => value(v).into(),
                    })
                    .collect()
            }
        };
        Self { values, labels }
    }
}

/// A tick on a step of `10^power`: as many decimals as the step needs, and
/// k, M, G or T once the axis reaches ten thousand.
fn number(value: f64, power: i32, magnitude: f64) -> String {
    const UNITS: [(f64, &str, i32); 4] =
        [(1e12, "T", 12), (1e9, "G", 9), (1e6, "M", 6), (1e3, "k", 3)];
    if magnitude >= 1e4
        && value != 0.0
        && let Some(&(unit, suffix, exponent)) = UNITS.iter().find(|(unit, ..)| magnitude >= *unit)
    {
        let decimals = (exponent - power).max(0) as usize;
        return format!("{}{suffix}", fixed(value / unit, decimals));
    }
    grouped(&fixed(value, (-power).max(0) as usize))
}

/// A value read out whole: integers grouped, others to two decimals or three
/// significant digits, whichever shows more, trailing zeros dropped.
pub fn value(value: f64) -> String {
    if value.is_nan() {
        return "–".into();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return grouped(&fixed(value, 0));
    }
    let significant = 3 - value.abs().log10().floor() as i32 - 1;
    let decimals = significant.clamp(2, 12) as usize;
    let text = fixed(value, decimals);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    grouped(text)
}

fn fixed(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    match text
        .trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        true => text.trim_start_matches('-').to_string(),
        false => text,
    }
}

/// Commas between thousands in the integer part.
fn grouped(text: &str) -> String {
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let (whole, fraction) = rest.split_at(rest.find('.').unwrap_or(rest.len()));
    let mut out = String::with_capacity(text.len() + whole.len() / 3);
    out.push_str(sign);
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out.push_str(fraction);
    out
}
