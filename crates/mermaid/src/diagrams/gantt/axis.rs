//! Time axis support, following d3-time and d3-time-format (English locale,
//! local time), which Mermaid uses: the `axisFormat` specifiers, tick
//! intervals with d3's `every` rule, and its choice of automatic ticks.
//! Pele's `diagrams/gantt/axis.ts`.

use super::dates::{MONTHS, WEEKDAYS, Weekday};
use crate::js::{
    self, Lazy,
    date::{Date, LocalZone},
};
use std::fmt::Write as _;

const SECOND: f64 = 1000.0;
const MINUTE: f64 = 60.0 * SECOND;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;
const WEEK: f64 = 7.0 * DAY;
const MONTH: f64 = 30.0 * DAY;
const YEAR: f64 = 365.0 * DAY;
const AUTO: [(TickUnit, f64, f64); 18] = [
    (TickUnit::Second, 1.0, SECOND),
    (TickUnit::Second, 5.0, 5.0 * SECOND),
    (TickUnit::Second, 15.0, 15.0 * SECOND),
    (TickUnit::Second, 30.0, 30.0 * SECOND),
    (TickUnit::Minute, 1.0, MINUTE),
    (TickUnit::Minute, 5.0, 5.0 * MINUTE),
    (TickUnit::Minute, 15.0, 15.0 * MINUTE),
    (TickUnit::Minute, 30.0, 30.0 * MINUTE),
    (TickUnit::Hour, 1.0, HOUR),
    (TickUnit::Hour, 3.0, 3.0 * HOUR),
    (TickUnit::Hour, 6.0, 6.0 * HOUR),
    (TickUnit::Hour, 12.0, 12.0 * HOUR),
    (TickUnit::Day, 1.0, DAY),
    (TickUnit::Day, 2.0, 2.0 * DAY),
    (TickUnit::Week, 1.0, WEEK),
    (TickUnit::Month, 1.0, MONTH),
    (TickUnit::Month, 3.0, 3.0 * MONTH),
    (TickUnit::Year, 1.0, YEAR),
];

static RE_INTERVAL: Lazy = Lazy::new(
    r"^([1-9]\d*)(millisecond|second|minute|hour|day|week|month)$",
    "",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickUnit {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl TickUnit {
    fn parse(name: &str) -> TickUnit {
        match name {
            "second" => TickUnit::Second,
            "minute" => TickUnit::Minute,
            "hour" => TickUnit::Hour,
            "day" => TickUnit::Day,
            "week" => TickUnit::Week,
            "month" => TickUnit::Month,
            "year" => TickUnit::Year,
            _ => TickUnit::Millisecond,
        }
    }
}

fn start_of_day(d: &mut Date) {
    d.set_hours(0.0, 0.0, 0.0, 0.0);
}

/// A d3 interval: `unit`, and `step` of them when `every` gave more than one.
#[derive(Clone, Copy)]
struct Calendar {
    unit: TickUnit,
    /// The day weeks start on, 0 for Sunday.
    week_start: f64,
    step: f64,
    /// The start of the week the epoch is in, from which `every` counts
    /// weeks.
    epoch: f64,
}

impl Calendar {
    fn base(unit: TickUnit, week_start: f64) -> Self {
        Self {
            unit,
            week_start,
            step: 1.0,
            epoch: 0.0,
        }
    }

    /// How many values `field` cycles through, which bounds how far apart
    /// filtered ticks can be.
    fn cycle(&self) -> f64 {
        match self.unit {
            TickUnit::Second | TickUnit::Minute => 60.0,
            TickUnit::Hour => 24.0,
            TickUnit::Day => 31.0,
            TickUnit::Month => 12.0,
            _ => f64::INFINITY,
        }
    }

    fn ms(&self) -> f64 {
        match self.unit {
            TickUnit::Millisecond => 1.0,
            TickUnit::Second => SECOND,
            TickUnit::Minute => MINUTE,
            TickUnit::Hour => HOUR,
            TickUnit::Day => DAY,
            TickUnit::Week => WEEK,
            TickUnit::Month => MONTH,
            TickUnit::Year => YEAR,
        }
    }

    /// The value d3 tests against a step: a tick falls where it is a
    /// multiple of the step.
    fn field(&self, d: Date) -> f64 {
        match self.unit {
            TickUnit::Second => d.utc_seconds(),
            TickUnit::Minute => d.minutes(),
            TickUnit::Hour => d.hours(),
            TickUnit::Day => d.date() - 1.0,
            TickUnit::Month => d.month(),
            _ => f64::NAN,
        }
    }

    fn base_floor(&self, d: &mut Date) {
        match self.unit {
            TickUnit::Millisecond => {}
            TickUnit::Second => d.set_time(d.time() - d.milliseconds()),
            TickUnit::Minute => d.set_time(d.time() - d.milliseconds() - d.seconds() * SECOND),
            TickUnit::Hour => d.set_time(
                d.time() - d.milliseconds() - d.seconds() * SECOND - d.minutes() * MINUTE,
            ),
            TickUnit::Day => start_of_day(d),
            TickUnit::Week => {
                d.set_date(d.date() - (d.day() + 7.0 - self.week_start) % 7.0);
                start_of_day(d);
            }
            TickUnit::Month => {
                d.set_date(1.0);
                start_of_day(d);
            }
            TickUnit::Year => {
                d.set_month_date(0.0, 1.0);
                start_of_day(d);
            }
        }
    }

    fn base_offset(&self, d: &mut Date, step: f64) {
        match self.unit {
            TickUnit::Millisecond => d.set_time(d.time() + step),
            TickUnit::Second => d.set_time(d.time() + step * SECOND),
            TickUnit::Minute => d.set_time(d.time() + step * MINUTE),
            TickUnit::Hour => d.set_time(d.time() + step * HOUR),
            TickUnit::Day => d.set_date(d.date() + step),
            TickUnit::Week => d.set_date(d.date() + step * 7.0),
            TickUnit::Month => d.set_month(d.month() + step),
            TickUnit::Year => d.set_full_year(d.full_year() + step),
        }
    }

    fn test(&self, d: Date) -> bool {
        self.field(d) % self.step == 0.0
    }

    fn floor(&self, d: &mut Date) {
        if self.step <= 1.0 {
            return self.base_floor(d);
        }
        let step = self.step;
        match self.unit {
            TickUnit::Millisecond => d.set_time((d.time() / step).floor() * step),
            TickUnit::Year => {
                d.set_full_year((d.full_year() / step).floor() * step);
                self.base_floor(d);
            }
            TickUnit::Week => {
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

    fn offset(&self, d: &mut Date, n: f64) {
        if self.step <= 1.0 {
            return self.base_offset(d, n);
        }
        let step = self.step;
        match self.unit {
            TickUnit::Millisecond => d.set_time(d.time() + n * step),
            TickUnit::Year => d.set_full_year(d.full_year() + n * step),
            TickUnit::Week => self.base_offset(d, n * step),
            _ => {
                let mut i = 0.0;
                while i < n && d.time().is_finite() {
                    loop {
                        self.base_offset(d, 1.0);
                        if !(d.time().is_finite() && !self.test(*d)) {
                            break;
                        }
                    }
                    i += 1.0;
                }
            }
        }
    }
}

/// Whole units between two floored dates, discounting a change of UTC
/// offset between them.
fn span(start: Date, end: Date, unit: f64) -> f64 {
    ((end.time() - start.time() - (end.timezone_offset() - start.timezone_offset()) * MINUTE)
        / unit)
        .floor()
}

fn modulo(a: f64, b: f64) -> f64 {
    ((a % b) + b) % b
}

/// The interval d3's `unit.every(step)` gives.
fn every(unit: TickUnit, step: f64, week_start: f64, zone: &LocalZone) -> Calendar {
    let base = Calendar::base(unit, week_start);
    if step <= 1.0 {
        return base;
    }
    let mut calendar = Calendar { step, ..base };
    if unit == TickUnit::Week {
        let mut epoch = zone.date(0.0);
        base.base_floor(&mut epoch);
        calendar.epoch = epoch.time();
    }
    calendar
}

/// Ticks from start to stop inclusive, or `None` when there would be more
/// than `limit`.
fn range(
    calendar: &Calendar,
    start: f64,
    stop: f64,
    limit: f64,
    zone: &LocalZone,
) -> Option<Vec<f64>> {
    let mut d = zone.date(start - 1.0);
    calendar.floor(&mut d);
    calendar.offset(&mut d, 1.0);
    calendar.floor(&mut d);
    let mut out = Vec::new();
    let mut previous = f64::NEG_INFINITY;
    while d.time() <= stop && d.time() > previous {
        if out.len() as f64 >= limit {
            return None;
        }
        previous = d.time();
        out.push(previous);
        calendar.offset(&mut d, 1.0);
        calendar.floor(&mut d);
    }
    Some(out)
}

fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= std::f64::consts::SQRT_2 {
        2.0
    } else {
        1.0
    };
    10f64.powf(power) * factor
}

/// About `count` ticks at round times, chosen the way d3's time scale
/// chooses them.
pub(crate) fn auto_ticks(start: f64, stop: f64, count: f64, zone: &LocalZone) -> Vec<f64> {
    let target = (stop - start) / count;
    let mut i = 0;
    while i < AUTO.len() && AUTO[i].2 <= target {
        i += 1;
    }
    let calendar = if i == AUTO.len() {
        every(
            TickUnit::Year,
            1f64.max(tick_step(start / YEAR, stop / YEAR, count)),
            0.0,
            zone,
        )
    } else if i == 0 {
        every(
            TickUnit::Millisecond,
            tick_step(start, stop, count).max(1.0),
            0.0,
            zone,
        )
    } else {
        let (unit, step, _) = AUTO[if target / AUTO[i - 1].2 < AUTO[i].2 / target {
            i - 1
        } else {
            i
        }];
        every(unit, step, 0.0, zone)
    };
    range(&calendar, start, stop, 4.0 * count + 8.0, zone).unwrap_or_default()
}

/// Ticks for a `tickInterval` such as `1week`, or `None` when it is not one
/// or asks for more than `limit`.
pub(crate) fn interval_ticks(
    interval: &str,
    week_start: Weekday,
    start: f64,
    stop: f64,
    limit: f64,
    zone: &LocalZone,
) -> Option<Vec<f64>> {
    let m = RE_INTERVAL.exec(interval)?;
    let step = js::parse_int(m.str(1), 10);
    let unit = TickUnit::parse(m.str(2));
    let calendar = every(unit, step, week_start as usize as f64, zone);
    if (stop - start) / (step.min(calendar.cycle()) * calendar.ms()) > limit {
        return None;
    }
    range(&calendar, start, stop, limit, zone)
}

fn pad(value: f64, fill: &str, width: usize) -> String {
    let negative = value < 0.0;
    let digits = js::num_str(if negative { -value } else { value });
    let sign = if negative { "-" } else { "" };
    let units = js::len(&digits);
    if units < width {
        format!("{sign}{}{digits}", fill.repeat(width - units))
    } else {
        format!("{sign}{digits}")
    }
}

fn floored<'z>(date: Date<'z>, calendar: &Calendar) -> Date<'z> {
    let mut d = date.at(date.time());
    calendar.floor(&mut d);
    d
}

fn year_calendar() -> Calendar {
    Calendar::base(TickUnit::Year, 0.0)
}

fn week_calendar(week_start: f64) -> Calendar {
    Calendar::base(TickUnit::Week, week_start)
}

/// Weeks since the last `week_start` day before the year began, which is
/// d3's `%U` and `%W`.
fn week_of_year(date: Date, week_start: f64) -> f64 {
    let week = week_calendar(week_start);
    let year_start = floored(date, &year_calendar());
    span(
        floored(year_start.at(year_start.time() - 1.0), &week),
        floored(date, &week),
        WEEK,
    )
}

/// The Thursday that decides which ISO week and ISO year a date belongs to.
fn iso_thursday(date: Date) -> Date {
    let thursday = week_calendar(4.0);
    let day = date.day();
    let late = day >= 4.0 || day == 0.0;
    let mut d = floored(
        if late {
            date
        } else {
            date.at(date.time() - 1.0)
        },
        &thursday,
    );
    if !late {
        thursday.offset(&mut d, 1.0);
        thursday.floor(&mut d);
    }
    d
}

/// Formats a date with a d3 time format specifier such as `%Y-%m-%d`.
pub(crate) fn time_format(specifier: &str, date: Date) -> String {
    let mut out = String::new();
    let mut chars = specifier.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut c = chars.next();
        let fill = match c {
            Some('-') => Some(""),
            Some('_') => Some(" "),
            Some('0') => Some("0"),
            _ => None,
        };
        let fill = match fill {
            Some(fill) => {
                c = chars.next();
                fill
            }
            None if c == Some('e') => " ",
            None => "0",
        };
        let _ = write!(out, "{}", directive(c, date, fill));
    }
    out
}

fn twelve(hours: f64) -> f64 {
    let h = hours % 12.0;
    if js::truthy(h) { h } else { 12.0 }
}

fn directive(c: Option<char>, d: Date, p: &str) -> String {
    let Some(c) = c else {
        return String::new();
    };
    let day = || d.day() as usize;
    let month = || d.month() as usize;
    match c {
        'a' => WEEKDAYS[day()][..3].to_string(),
        'A' => WEEKDAYS[day()].to_string(),
        'b' => MONTHS[month()][..3].to_string(),
        'B' => MONTHS[month()].to_string(),
        'c' => time_format("%x, %X", d),
        'd' | 'e' => pad(d.date(), p, 2),
        'f' => pad(d.milliseconds(), p, 3) + "000",
        'g' => pad(iso_thursday(d).full_year() % 100.0, p, 2),
        'G' => pad(iso_thursday(d).full_year() % 10000.0, p, 4),
        'H' => pad(d.hours(), p, 2),
        'I' => pad(twelve(d.hours()), p, 2),
        'j' => pad(
            1.0 + span(
                floored(d, &year_calendar()),
                floored(d, &Calendar::base(TickUnit::Day, 0.0)),
                DAY,
            ),
            p,
            3,
        ),
        'L' => pad(d.milliseconds(), p, 3),
        'm' => pad(d.month() + 1.0, p, 2),
        'M' => pad(d.minutes(), p, 2),
        'p' => if d.hours() >= 12.0 { "PM" } else { "AM" }.into(),
        'q' => js::num_str(1.0 + (d.month() / 3.0).floor()),
        'Q' => js::num_str(d.time()),
        's' => js::num_str((d.time() / 1000.0).floor()),
        'S' => pad(d.seconds(), p, 2),
        'u' => js::num_str(if js::truthy(d.day()) { d.day() } else { 7.0 }),
        'U' => pad(week_of_year(d, 0.0), p, 2),
        'V' => {
            let thursday = iso_thursday(d);
            let year_start = floored(thursday, &year_calendar());
            let first = floored(year_start, &week_calendar(4.0));
            pad(
                span(first, thursday, WEEK) + if year_start.day() == 4.0 { 1.0 } else { 0.0 },
                p,
                2,
            )
        }
        'w' => js::num_str(d.day()),
        'W' => pad(week_of_year(d, 1.0), p, 2),
        'x' => time_format("%-m/%-d/%Y", d),
        'X' => time_format("%-I:%M:%S %p", d),
        'y' => pad(d.full_year() % 100.0, p, 2),
        'Y' => pad(d.full_year() % 10000.0, p, 4),
        'Z' => {
            let z = d.timezone_offset();
            let abs = z.abs();
            format!(
                "{}{}{}",
                if z > 0.0 { "-" } else { "+" },
                pad((abs / 60.0).floor(), "0", 2),
                pad(abs % 60.0, "0", 2)
            )
        }
        '%' => "%".into(),
        _ => c.to_string(),
    }
}
