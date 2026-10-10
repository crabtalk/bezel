//! Times as milliseconds since the Unix epoch, read on the UTC calendar.

pub const SECOND: f64 = 1_000.0;
pub const MINUTE: f64 = 60.0 * SECOND;
pub const HOUR: f64 = 60.0 * MINUTE;
pub const DAY: f64 = 24.0 * HOUR;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The finest calendar field a tick label shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Second,
    Minute,
    Day,
    Month,
    Year,
}

/// Year, month 1–12 and day 1–31 of the day `days` after 1970-01-01.
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// Days from 1970-01-01 to `year`-`month`-`day`.
pub fn days(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (i64::from(month) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, then optionally `T` or a space and
/// `HH:MM`, `:SS` and `.fff`, and a trailing `Z`. Every time is UTC; an offset
/// other than `Z` is `None`.
pub fn parse(text: &str) -> Option<f64> {
    let text = text.trim();
    let text = text.strip_suffix('Z').unwrap_or(text);
    let (date, clock) = match text.find(['T', ' ']) {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next().map_or(Some(1), |p| p.parse().ok())?;
    let day: u32 = parts.next().map_or(Some(1), |p| p.parse().ok())?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut ms = days(year, month, day) as f64 * DAY;
    if let Some(clock) = clock {
        let mut parts = clock.split(':');
        let hour: f64 = parts.next()?.parse().ok()?;
        let minute: f64 = parts.next()?.parse().ok()?;
        let second: f64 = parts.next().map_or(Some(0.0), |p| p.parse().ok())?;
        if parts.next().is_some() {
            return None;
        }
        ms += hour * HOUR + minute * MINUTE + second * SECOND;
    }
    Some(ms)
}

/// `ms` as a tick label showing fields down to `unit`.
pub fn label(ms: f64, unit: Unit) -> String {
    let (year, month, day, hour, minute, second) = fields(ms);
    let name = MONTHS[month as usize - 1];
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

fn fields(ms: f64) -> (i64, u32, u32, u32, u32, u32) {
    let day = (ms / DAY).floor();
    let (year, month, date) = civil(day as i64);
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

    let (year, month, _) = civil((lo / DAY).floor() as i64);
    if let Some(step) = [1, 3, 6]
        .into_iter()
        .find(|&k: &i64| k as f64 * MONTH >= target)
    {
        let mut index = year * 12 + i64::from(month) - 1;
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
    let mut year = if (days(year, 1, 1) as f64 * DAY) < lo {
        year + 1
    } else {
        year
    };
    year = (year + step - 1).div_euclid(step) * step;
    while (days(year, 1, 1) as f64 * DAY) <= hi {
        ticks.push(days(year, 1, 1) as f64 * DAY);
        year += step;
    }
    (ticks, Unit::Year)
}

fn month_start(index: i64) -> f64 {
    days(index.div_euclid(12), index.rem_euclid(12) as u32 + 1, 1) as f64 * DAY
}
