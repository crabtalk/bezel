//! Vega's date functions on `js::date`, and its time units as d3-time
//! intervals.

use crate::expr::Error;
use js::{
    date::{Date, LocalZone},
    time::{Interval, Unit},
    value::Value,
};
use std::str::FromStr;

/// A calendar field of a date.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Field {
    /// Day of the month, 1–31.
    Date,
    /// Day of the week, 0 for Sunday.
    Day,
    /// 1 for January 1st.
    DayOfYear,
    Year,
    /// 1–4.
    Quarter,
    /// 0 for January.
    Month,
    /// Sundays from January 1st up to the date.
    Week,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
    /// The time value itself.
    Time,
}

impl FromStr for Field {
    type Err = ();

    /// Vega's getter names, without the `utc` prefix.
    fn from_str(name: &str) -> Result<Self, ()> {
        Ok(match name {
            "date" => Self::Date,
            "day" => Self::Day,
            "dayofyear" => Self::DayOfYear,
            "year" => Self::Year,
            "quarter" => Self::Quarter,
            "month" => Self::Month,
            "week" => Self::Week,
            "hours" => Self::Hours,
            "minutes" => Self::Minutes,
            "seconds" => Self::Seconds,
            "milliseconds" => Self::Milliseconds,
            "time" => Self::Time,
            _ => return Err(()),
        })
    }
}

impl Field {
    /// The field of `date` in its zone.
    pub(crate) fn of(self, date: Date) -> f64 {
        if !date.is_valid() {
            return f64::NAN;
        }
        let mut year = date;
        Interval::new(Unit::Year, 0.0).floor(&mut year);
        match self {
            Self::Date => date.date(),
            Self::Day => date.day(),
            Self::DayOfYear => 1.0 + Interval::new(Unit::Day, 0.0).count(year, date),
            Self::Year => date.full_year(),
            Self::Quarter => (date.month() / 3.0).floor() + 1.0,
            Self::Month => date.month(),
            Self::Week => Interval::new(Unit::Week, 0.0).count(year.at(year.time() - 1.0), date),
            Self::Hours => date.hours(),
            Self::Minutes => date.minutes(),
            Self::Seconds => date.seconds(),
            Self::Milliseconds => date.milliseconds(),
            Self::Time => date.time(),
        }
    }
}

/// The d3-time interval of a Vega time unit: a quarter is three months, and
/// weeks start on Sunday.
pub(crate) fn interval(unit: &str, zone: &LocalZone) -> Result<Interval, Error> {
    let (unit, step) = match unit {
        "year" => (Unit::Year, 1.0),
        "quarter" => (Unit::Month, 3.0),
        "month" => (Unit::Month, 1.0),
        "week" => (Unit::Week, 1.0),
        "date" | "day" | "dayofyear" => (Unit::Day, 1.0),
        "hours" => (Unit::Hour, 1.0),
        "minutes" => (Unit::Minute, 1.0),
        "seconds" => (Unit::Second, 1.0),
        "milliseconds" => (Unit::Millisecond, 1.0),
        _ => return Err(Error::Runtime(format!("Invalid time unit: {unit}"))),
    };
    Ok(Interval::every(unit, step, 0.0, zone))
}

/// `new Date(value).getTime()`, a string read in the host's zone.
pub(crate) fn time(value: &Value) -> f64 {
    match value.to_primitive() {
        Value::String(s) => LocalZone::default().parse(&s).time(),
        other => LocalZone::default().date(other.to_number()).time(),
    }
}

/// The year, month from 0, day, hours, minutes, seconds and milliseconds of
/// `new Date(…)` and `Date.UTC(…)`: `NaN` for a missing year, the day 1 and
/// the rest 0 when missing.
pub(crate) fn parts(args: &[Value]) -> [f64; 7] {
    let mut parts = [f64::NAN, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    for (part, arg) in parts.iter_mut().zip(args) {
        *part = arg.to_number();
    }
    parts
}
