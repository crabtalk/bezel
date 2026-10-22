//! Times as milliseconds since the Unix epoch, read on the UTC calendar.

use js::{
    date::{self, LocalZone},
    time,
};

pub const SECOND: f64 = 1_000.0;
pub const MINUTE: f64 = 60.0 * SECOND;
pub const HOUR: f64 = 60.0 * MINUTE;
pub const DAY: f64 = 24.0 * HOUR;

/// The finest calendar field a tick label shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Second,
    Minute,
    Day,
    Month,
    Year,
}

/// The field that ticks on a d3-time interval of this unit show.
impl From<time::Unit> for Unit {
    fn from(unit: time::Unit) -> Self {
        match unit {
            time::Unit::Millisecond | time::Unit::Second => Unit::Second,
            time::Unit::Minute | time::Unit::Hour => Unit::Minute,
            time::Unit::Day | time::Unit::Week => Unit::Day,
            time::Unit::Month => Unit::Month,
            time::Unit::Year => Unit::Year,
        }
    }
}

/// The whole seconds of `ms`, which are all JavaScript's `String(date)`
/// shows of a date.
pub fn second(ms: f64) -> f64 {
    (ms / SECOND).floor()
}

/// `Date.parse(text)` with UTC as the local zone: a string without an
/// offset is UTC. `None` where it is `NaN`.
pub fn parse(text: &str) -> Option<f64> {
    let time = LocalZone::utc().parse(text).time();
    (!time.is_nan()).then_some(time)
}

/// `new Date(year, month, day, h, m, s, ms)` with UTC as the local zone, as
/// [`parse`] reads a string without an offset. `None` where it is `NaN`.
pub fn local(year: f64, month: f64, day: f64, h: f64, m: f64, s: f64, ms: f64) -> Option<f64> {
    let time = LocalZone::utc().local(year, month, day, h, m, s, ms).time();
    (!time.is_nan()).then_some(time)
}

/// `ms` as a tick label showing fields down to `unit`.
pub fn label(ms: f64, unit: Unit) -> String {
    let (year, month, day, hour, minute, second) = fields(ms);
    let name = &date::MONTHS[month as usize - 1][..3];
    match unit {
        Unit::Year => format!("{year}"),
        Unit::Month if month == 1 => format!("{year}"),
        Unit::Month => name.to_string(),
        Unit::Day => format!("{name} {day}"),
        Unit::Minute => format!("{hour:02}:{minute:02}"),
        Unit::Second => format!("{hour:02}:{minute:02}:{second:02}"),
    }
}

/// `ms` in full: the date, then the time of day unless it is midnight.
pub fn describe(ms: f64) -> String {
    let (year, month, day, hour, minute, second) = fields(ms);
    match (hour, minute, second) {
        (0, 0, 0) => format!("{year}-{month:02}-{day:02}"),
        (_, _, 0) => format!("{year}-{month:02}-{day:02} {hour:02}:{minute:02}"),
        _ => format!("{year}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"),
    }
}

fn fields(ms: f64) -> (i64, i64, i64, u32, u32, u32) {
    let day = (ms / DAY).floor();
    let (year, month, date) = date::civil_from_days(day as i64);
    let rest = ((ms - day * DAY) / SECOND).floor() as u32;
    (year, month, date, rest / 3_600, rest / 60 % 60, rest % 60)
}
