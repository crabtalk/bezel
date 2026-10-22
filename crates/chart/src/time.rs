//! Times as milliseconds since the Unix epoch, read on the UTC calendar.

use js::date::LocalZone;

pub const SECOND: f64 = 1_000.0;
pub const MINUTE: f64 = 60.0 * SECOND;
pub const HOUR: f64 = 60.0 * MINUTE;
pub const DAY: f64 = 24.0 * HOUR;
/// Vega-Lite's `timeFormat`, which text, tooltips and legends write a
/// temporal field's values in.
pub const FORMAT: &str = "%b %d, %Y";

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
