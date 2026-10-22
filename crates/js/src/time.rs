//! d3-time's intervals in a [`LocalZone`], with d3's `every` rule, and the
//! ticks d3-scale's time scale chooses. In [`LocalZone::utc`] they are d3's
//! `utc` intervals.

use crate::{
    array,
    date::{Date, LocalZone},
    number,
};

const SECOND: f64 = 1000.0;
const MINUTE: f64 = 60.0 * SECOND;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;
const WEEK: f64 = 7.0 * DAY;
const MONTH: f64 = 30.0 * DAY;
const YEAR: f64 = 365.0 * DAY;
const AUTO: [(Unit, f64, f64); 18] = [
    (Unit::Second, 1.0, SECOND),
    (Unit::Second, 5.0, 5.0 * SECOND),
    (Unit::Second, 15.0, 15.0 * SECOND),
    (Unit::Second, 30.0, 30.0 * SECOND),
    (Unit::Minute, 1.0, MINUTE),
    (Unit::Minute, 5.0, 5.0 * MINUTE),
    (Unit::Minute, 15.0, 15.0 * MINUTE),
    (Unit::Minute, 30.0, 30.0 * MINUTE),
    (Unit::Hour, 1.0, HOUR),
    (Unit::Hour, 3.0, 3.0 * HOUR),
    (Unit::Hour, 6.0, 6.0 * HOUR),
    (Unit::Hour, 12.0, 12.0 * HOUR),
    (Unit::Day, 1.0, DAY),
    (Unit::Day, 2.0, 2.0 * DAY),
    (Unit::Week, 1.0, WEEK),
    (Unit::Month, 1.0, MONTH),
    (Unit::Month, 3.0, 3.0 * MONTH),
    (Unit::Year, 1.0, YEAR),
];

/// A d3-time interval's unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl Unit {
    /// How many values the field d3 tests a step of this unit against takes:
    /// 60 seconds or minutes, 24 hours, 31 days, 12 months; infinite for the
    /// rest.
    pub fn cycle(self) -> f64 {
        match self {
            Unit::Second | Unit::Minute => 60.0,
            Unit::Hour => 24.0,
            Unit::Day => 31.0,
            Unit::Month => 12.0,
            _ => f64::INFINITY,
        }
    }

    /// The unit's length in milliseconds, a month as 30 days and a year as
    /// 365.
    pub fn duration(self) -> f64 {
        match self {
            Unit::Millisecond => 1.0,
            Unit::Second => SECOND,
            Unit::Minute => MINUTE,
            Unit::Hour => HOUR,
            Unit::Day => DAY,
            Unit::Week => WEEK,
            Unit::Month => MONTH,
            Unit::Year => YEAR,
        }
    }
}

/// A d3-time interval: `unit`, and `step` of them when `every` gave more
/// than one.
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    pub unit: Unit,
    /// The day weeks start on, 0 for Sunday.
    week_start: f64,
    pub step: f64,
    /// The start of the week the epoch is in, from which `every` counts
    /// weeks.
    epoch: f64,
    /// Whether a day's field is days since the epoch, as d3's `unixDay`
    /// tests a step, rather than the day of the month.
    unix: bool,
}

impl Interval {
    /// The interval of one `unit`, weeks starting on `week_start`, 0 for
    /// Sunday.
    pub fn new(unit: Unit, week_start: f64) -> Self {
        Self {
            unit,
            week_start,
            step: 1.0,
            epoch: 0.0,
            unix: false,
        }
    }

    /// d3's `interval.every(step)`.
    pub fn every(unit: Unit, step: f64, week_start: f64, zone: &LocalZone) -> Self {
        let base = Self::new(unit, week_start);
        if step <= 1.0 {
            return base;
        }
        let mut interval = Self { step, ..base };
        if unit == Unit::Week {
            let mut epoch = zone.date(0.0);
            base.base_floor(&mut epoch);
            interval.epoch = epoch.time();
        }
        interval
    }

    /// The value d3 tests against a step: a tick falls where it is a
    /// multiple of the step.
    fn field(&self, d: Date) -> f64 {
        match self.unit {
            // `getUTCSeconds()`.
            Unit::Second => (d.time() / SECOND).floor().rem_euclid(60.0),
            Unit::Minute => d.minutes(),
            Unit::Hour => d.hours(),
            Unit::Day if self.unix => number::to_int32(d.time() / DAY) as f64,
            Unit::Day => d.date() - 1.0,
            Unit::Month => d.month(),
            _ => f64::NAN,
        }
    }

    fn base_floor(&self, d: &mut Date) {
        match self.unit {
            Unit::Millisecond => {}
            Unit::Second => d.set_time(d.time() - d.milliseconds()),
            Unit::Minute => d.set_time(d.time() - d.milliseconds() - d.seconds() * SECOND),
            Unit::Hour => d.set_time(
                d.time() - d.milliseconds() - d.seconds() * SECOND - d.minutes() * MINUTE,
            ),
            Unit::Day => start_of_day(d),
            Unit::Week => {
                d.set_date(d.date() - (d.day() + 7.0 - self.week_start) % 7.0);
                start_of_day(d);
            }
            Unit::Month => {
                d.set_date(1.0);
                start_of_day(d);
            }
            Unit::Year => {
                d.set_month_date(0.0, 1.0);
                start_of_day(d);
            }
        }
    }

    fn base_offset(&self, d: &mut Date, step: f64) {
        match self.unit {
            Unit::Millisecond => d.set_time(d.time() + step),
            Unit::Second => d.set_time(d.time() + step * SECOND),
            Unit::Minute => d.set_time(d.time() + step * MINUTE),
            Unit::Hour => d.set_time(d.time() + step * HOUR),
            Unit::Day => d.set_date(d.date() + step),
            Unit::Week => d.set_date(d.date() + step * 7.0),
            Unit::Month => d.set_month(d.month() + step),
            Unit::Year => d.set_full_year(d.full_year() + step),
        }
    }

    fn test(&self, d: Date) -> bool {
        self.field(d) % self.step == 0.0
    }

    /// `interval.floor(d)`, in place.
    pub fn floor(&self, d: &mut Date) {
        if self.step <= 1.0 {
            return self.base_floor(d);
        }
        let step = self.step;
        match self.unit {
            Unit::Millisecond => d.set_time((d.time() / step).floor() * step),
            Unit::Year => {
                d.set_full_year((d.full_year() / step).floor() * step);
                self.base_floor(d);
            }
            Unit::Week => {
                self.base_floor(d);
                let back = -modulo(span(d.at(self.epoch), *d, WEEK), step);
                self.base_offset(d, back);
            }
            _ => {
                // Date overflow makes field(d) NaN, which can never match the
                // interval.
                self.base_floor(d);
                while d.time().is_finite() && !self.test(*d) {
                    d.set_time(d.time() - 1.0);
                    self.base_floor(d);
                }
            }
        }
    }

    /// `interval.offset(d, n)`, in place.
    pub fn offset(&self, d: &mut Date, n: f64) {
        if self.step <= 1.0 {
            return self.base_offset(d, n);
        }
        let step = self.step;
        match self.unit {
            Unit::Millisecond => d.set_time(d.time() + n * step),
            Unit::Year => d.set_full_year(d.full_year() + n * step),
            Unit::Week => self.base_offset(d, n * step),
            _ => {
                let unit = if n < 0.0 { -1.0 } else { 1.0 };
                let mut i = 1.0;
                while i <= n.abs() && d.time().is_finite() {
                    loop {
                        self.base_offset(d, unit);
                        if !(d.time().is_finite() && !self.test(*d)) {
                            break;
                        }
                    }
                    i += 1.0;
                }
            }
        }
    }

    /// d3's `interval.count(start, end)` of one unit: the boundaries after
    /// `start` up to `end`, both floored.
    pub fn count(&self, start: Date, end: Date) -> f64 {
        let (mut start, mut end) = (start, end);
        self.base_floor(&mut start);
        self.base_floor(&mut end);
        match self.unit {
            Unit::Day => span(start, end, DAY),
            Unit::Week => span(start, end, WEEK),
            Unit::Month => {
                (end.full_year() - start.full_year()) * 12.0 + end.month() - start.month()
            }
            Unit::Year => end.full_year() - start.full_year(),
            unit => ((end.time() - start.time()) / unit.duration()).floor(),
        }
    }

    /// `interval.ceil(d)`, in place.
    pub fn ceil(&self, d: &mut Date) {
        *d = self.ceil_at(d.time(), d.zone());
    }

    /// `interval.ceil(time)` of a time value.
    fn ceil_at<'z>(&self, time: f64, zone: &'z LocalZone) -> Date<'z> {
        let mut d = zone.date(time - 1.0);
        self.floor(&mut d);
        self.offset(&mut d, 1.0);
        self.floor(&mut d);
        d
    }

    /// `d` floored to this interval, `d` left as it is.
    pub(crate) fn floored<'z>(&self, d: Date<'z>) -> Date<'z> {
        let mut floored = d.at(d.time());
        self.floor(&mut floored);
        floored
    }

    /// d3's `interval.range(start, stop, step)`: every `step`th boundary from
    /// the first at or after `start`, before `stop`.
    pub fn range(&self, start: f64, stop: f64, step: f64, zone: &LocalZone) -> Vec<f64> {
        let step = step.floor();
        let mut d = self.ceil_at(start, zone);
        let mut out = Vec::new();
        if !(d.time() < stop && step > 0.0) {
            return out;
        }
        loop {
            let previous = d.time();
            out.push(previous);
            self.offset(&mut d, step);
            self.floor(&mut d);
            if !(previous < d.time() && d.time() < stop) {
                return out;
            }
        }
    }

    /// The interval's times from `start` to `stop` inclusive, or `None` when
    /// there would be more than `limit`.
    pub fn ticks(&self, start: f64, stop: f64, limit: f64, zone: &LocalZone) -> Option<Vec<f64>> {
        let mut d = self.ceil_at(start, zone);
        let mut out = Vec::new();
        let mut previous = f64::NEG_INFINITY;
        while d.time() <= stop && d.time() > previous {
            if out.len() as f64 >= limit {
                return None;
            }
            previous = d.time();
            out.push(previous);
            self.offset(&mut d, 1.0);
            self.floor(&mut d);
        }
        Some(out)
    }
}

fn start_of_day(d: &mut Date) {
    d.set_hours(0.0, 0.0, 0.0, 0.0);
}

/// Whole `unit`s of milliseconds between two floored dates, discounting a
/// change of UTC offset between them.
fn span(start: Date, end: Date, unit: f64) -> f64 {
    ((end.time() - start.time() - (end.timezone_offset() - start.timezone_offset()) * MINUTE)
        / unit)
        .floor()
}

fn modulo(a: f64, b: f64) -> f64 {
    ((a % b) + b) % b
}

/// d3-time's `tickInterval(start, stop, count)`: the interval about
/// `count` ticks from `start` to `stop` fall on. `None` where it has none.
/// In [`LocalZone::utc`], days count from the epoch, as `unixDay`'s do.
pub fn tick_interval(start: f64, stop: f64, count: f64, zone: &LocalZone) -> Option<Interval> {
    let target = (stop - start).abs() / count;
    let i = AUTO.partition_point(|(.., duration)| *duration <= target);
    let every = |unit: Unit, step: f64| {
        let step = step.floor();
        (step.is_finite() && step > 0.0).then(|| Interval::every(unit, step, 0.0, zone))
    };
    if i == AUTO.len() {
        return every(
            Unit::Year,
            array::tick_step(start / YEAR, stop / YEAR, count),
        );
    }
    if i == 0 {
        return every(
            Unit::Millisecond,
            array::tick_step(start, stop, count).max(1.0),
        );
    }
    let (unit, step, _) = AUTO[if target / AUTO[i - 1].2 < AUTO[i].2 / target {
        i - 1
    } else {
        i
    }];
    let interval = every(unit, step)?;
    Some(Interval {
        unix: unit == Unit::Day && *zone == LocalZone::utc(),
        ..interval
    })
}

/// d3-time's `timeTicks(start, stop, count)`, or `utcTicks` in
/// [`LocalZone::utc`]: about `count` round times from `start` to `stop`
/// inclusive, in their order.
pub fn ticks(start: f64, stop: f64, count: f64, zone: &LocalZone) -> Vec<f64> {
    let reverse = stop < start;
    let (start, stop) = match reverse {
        true => (stop, start),
        false => (start, stop),
    };
    let mut ticks = match tick_interval(start, stop, count, zone) {
        Some(interval) => interval.range(start, stop + 1.0, 1.0, zone),
        None => Vec::new(),
    };
    if reverse {
        ticks.reverse();
    }
    ticks
}
