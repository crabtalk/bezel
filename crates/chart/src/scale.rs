//! Where values land along a side of the plot, and the ticks that label it.

use gpui::SharedString;

use crate::time;

/// `domain` mapped linearly onto `range`. `range` may run backwards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Linear {
    pub domain: (f64, f64),
    pub range: (f32, f32),
}

impl Linear {
    pub fn at(&self, value: f64) -> f32 {
        let (d0, d1) = self.domain;
        let t = if d1 == d0 {
            0.5
        } else {
            (value - d0) / (d1 - d0)
        };
        self.range.0 + (self.range.1 - self.range.0) * t as f32
    }
}

/// `count` equal bands across `range`, which runs forwards. `padding` is the
/// share of each band left empty, half on either side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub count: usize,
    pub range: (f32, f32),
    pub padding: f32,
}

impl Band {
    pub fn step(&self) -> f32 {
        (self.range.1 - self.range.0) / self.count.max(1) as f32
    }

    /// Where band `index`'s filled part starts.
    pub fn start(&self, index: usize) -> f32 {
        self.range.0 + self.step() * (index as f32 + self.padding / 2.0)
    }

    /// The filled part's width.
    pub fn width(&self) -> f32 {
        self.step() * (1.0 - self.padding)
    }

    pub fn center(&self, index: usize) -> f32 {
        self.range.0 + self.step() * (index as f32 + 0.5)
    }

    /// The band `position` falls in.
    pub fn index(&self, position: f32) -> Option<usize> {
        let index = ((position - self.range.0) / self.step()).floor();
        (index >= 0.0 && (index as usize) < self.count).then_some(index as usize)
    }
}

/// Tick values and their labels, and the domain they span.
#[derive(Clone, Debug, PartialEq)]
pub struct Ticks {
    pub domain: (f64, f64),
    pub values: Vec<f64>,
    pub labels: Vec<SharedString>,
}

/// About `count` ticks at 1, 2 or 5 times a power of ten, with the domain
/// widened to the ticks either side of `lo..=hi`.
pub fn linear(lo: f64, hi: f64, count: usize) -> Ticks {
    let (lo, hi) = widen(lo, hi, 1.0);
    let (factor, power) = increment(lo, hi, count);
    let value = |index: f64| match power < 0 {
        true => index / (10f64.powi(-power) / factor),
        false => index * factor * 10f64.powi(power),
    };
    let step = value(1.0);
    let (first, last) = ((lo / step).floor(), (hi / step).ceil());
    let values: Vec<f64> = (first as i64..=last as i64)
        .map(|index| value(index as f64))
        .collect();
    let magnitude = lo.abs().max(hi.abs());
    let labels = values
        .iter()
        .map(|&v| number(v, power, magnitude).into())
        .collect();
    Ticks {
        domain: (value(first), value(last)),
        values,
        labels,
    }
}

/// Calendar ticks inside `lo..=hi`, which is the domain as given.
pub fn temporal(lo: f64, hi: f64, count: usize) -> Ticks {
    let (lo, hi) = widen(lo, hi, time::DAY);
    let (values, unit) = time::ticks(lo, hi, count);
    let labels = values
        .iter()
        .map(|&ms| time::label(ms, unit).into())
        .collect();
    Ticks {
        domain: (lo, hi),
        values,
        labels,
    }
}

fn widen(lo: f64, hi: f64, unit: f64) -> (f64, f64) {
    match (lo == hi, lo == 0.0) {
        (false, _) => (lo, hi),
        (true, true) => (0.0, unit),
        (true, false) => (
            lo - unit.max(lo.abs() / 10.0),
            hi + unit.max(hi.abs() / 10.0),
        ),
    }
}

/// The tick step as `factor * 10^power`, `factor` 1, 2 or 5.
fn increment(lo: f64, hi: f64, count: usize) -> (f64, i32) {
    let raw = (hi - lo) / count.max(1) as f64;
    let mut power = raw.log10().floor() as i32;
    let error = raw / 10f64.powi(power);
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    if factor == 10.0 {
        power += 1;
        return (1.0, power);
    }
    (factor, power)
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
