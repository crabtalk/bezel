//! Vega-Lite's [DateTime](https://vega.github.io/vega-lite/docs/datetime.html)
//! object: a time from calendar fields.

use d3::time::date::Date;
use serde_json::{Map, Value};

use crate::time;

/// The fields a DateTime may hold.
const FIELDS: [&str; 10] = [
    "year",
    "quarter",
    "month",
    "date",
    "day",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
    "utc",
];
const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];
const DAYS: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];
/// The year a DateTime without one falls in.
const YEAR: f64 = 2012.0;

/// The time a DateTime names, in milliseconds since the epoch: on the UTC
/// calendar with `utc`, else as [`time::local`] reads it. A `day` beside any
/// other key, `utc` among them, is dropped. `None` for an object with a key a DateTime lacks,
/// with no calendar field, or with a field it cannot read.
pub(crate) fn time(def: &Map<String, Value>) -> Option<f64> {
    if def.keys().any(|key| !FIELDS.contains(&key.as_str())) || def.keys().all(|key| key == "utc") {
        return None;
    }
    let utc = match def.get("utc") {
        None => false,
        Some(utc) => utc.as_bool()?,
    };
    let day = match def.get("day") {
        Some(day) if def.len() == 1 => Some(day),
        _ => None,
    };
    let field = |key: &str, default: f64| match def.get(key) {
        None => Some(default),
        Some(value) => number(value),
    };
    let month = match (def.get("month"), def.get("quarter")) {
        (Some(month), _) => number(month)
            .map(|month| month - 1.0)
            .or_else(|| index(month, &MONTHS))?,
        (None, Some(quarter)) => (number(quarter)? - 1.0) * 3.0,
        (None, None) => 0.0,
    };
    let date = match (def.get("date"), day) {
        (Some(date), _) => number(date)?,
        // Weekday `n` of 2012, which begins on a Sunday.
        (None, Some(day)) => {
            number(day)
                .map(|day| day % 7.0)
                .or_else(|| index(day, &DAYS))?
                + 1.0
        }
        (None, None) => 1.0,
    };
    let fields = [
        field("year", YEAR)?,
        month,
        date,
        field("hours", 0.0)?,
        field("minutes", 0.0)?,
        field("seconds", 0.0)?,
        field("milliseconds", 0.0)?,
    ];
    let [year, month, date, h, m, s, ms] = fields;
    match utc {
        true => Some(Date::utc(year, month, date, h, m, s, ms)).filter(|t| !t.is_nan()),
        false => time::local(year, month, date, h, m, s, ms),
    }
}

/// A number, or a string of one.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The place in `names` of a name or its first three letters, any case.
fn index(value: &Value, names: &[&str]) -> Option<f64> {
    let name = value.as_str()?.to_lowercase();
    let short = name.get(..3)?;
    names
        .iter()
        .position(|full| *full == name)
        .or_else(|| names.iter().position(|full| full[..3] == *short))
        .map(|index| index as f64)
}
