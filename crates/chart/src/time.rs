//! Times as milliseconds since the Unix epoch, read on the UTC calendar.

use js::date::{self, LocalZone};

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

/// Ticks on calendar boundaries inside `lo..=hi`, about `count` of them, and
/// the unit to label them with. Weeks start on Monday.
pub fn ticks(lo: f64, hi: f64, count: usize) -> (Vec<f64>, Unit) {
    const FIXED: [(f64, Unit); 15] = [
        (SECOND, Unit::Second),
        (5.0 * SECOND, Unit::Second),
        (15.0 * SECOND, Unit::Second),
        (30.0 * SECOND, Unit::Second),
        (MINUTE, Unit::Minute),
        (5.0 * MINUTE, Unit::Minute),
        (15.0 * MINUTE, Unit::Minute),
        (30.0 * MINUTE, Unit::Minute),
        (HOUR, Unit::Minute),
        (3.0 * HOUR, Unit::Minute),
        (6.0 * HOUR, Unit::Minute),
        (12.0 * HOUR, Unit::Minute),
        (DAY, Unit::Day),
        (2.0 * DAY, Unit::Day),
        (7.0 * DAY, Unit::Day),
    ];
    const MONTH: f64 = 30.44 * DAY;
    let target = (hi - lo) / count.max(1) as f64;
    let mut ticks = Vec::new();

    if let Some(&(step, unit)) = FIXED.iter().find(|(step, _)| *step >= target) {
        // 1970-01-05 was a Monday.
        let offset = if step == 7.0 * DAY { 4.0 * DAY } else { 0.0 };
        let mut at = ((lo - offset) / step).ceil() * step + offset;
        while at <= hi {
            ticks.push(at);
            at += step;
        }
        return (ticks, unit);
    }

    let (year, month, _) = date::civil_from_days((lo / DAY).floor() as i64);
    if let Some(step) = [1, 3, 6]
        .into_iter()
        .find(|&k: &i64| k as f64 * MONTH >= target)
    {
        let mut index = year * 12 + month - 1;
        if month_start(index) < lo {
            index += 1;
        }
        index = (index + step - 1).div_euclid(step) * step;
        while month_start(index) <= hi {
            ticks.push(month_start(index));
            index += step;
        }
        return (ticks, Unit::Month);
    }

    let years = target / (365.25 * DAY);
    let mut step = 10f64.powf(years.log10().floor()).max(1.0);
    for factor in [1.0, 2.0, 5.0, 10.0] {
        if step * factor >= years {
            step *= factor;
            break;
        }
    }
    let step = step as i64;
    let mut year = if (date::days_from_civil(year, 1, 1) as f64 * DAY) < lo {
        year + 1
    } else {
        year
    };
    year = (year + step - 1).div_euclid(step) * step;
    while (date::days_from_civil(year, 1, 1) as f64 * DAY) <= hi {
        ticks.push(date::days_from_civil(year, 1, 1) as f64 * DAY);
        year += step;
    }
    (ticks, Unit::Year)
}

fn month_start(index: i64) -> f64 {
    date::days_from_civil(index.div_euclid(12), index.rem_euclid(12) + 1, 1) as f64 * DAY
}
