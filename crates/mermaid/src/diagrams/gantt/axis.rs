//! Mermaid's `tickInterval`, such as `1week`, as d3-time intervals. Pele's
//! `diagrams/gantt/axis.ts`.

use crate::diagrams::gantt::dates::Weekday;
use js::{
    date::LocalZone,
    number,
    regex::Lazy,
    time::{Interval, Unit},
};

static RE_INTERVAL: Lazy = Lazy::new(
    r"^([1-9]\d*)(millisecond|second|minute|hour|day|week|month)$",
    "",
);

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
    let step = number::parse_int(m.str(1), 10);
    let unit = match m.str(2) {
        "second" => Unit::Second,
        "minute" => Unit::Minute,
        "hour" => Unit::Hour,
        "day" => Unit::Day,
        "week" => Unit::Week,
        "month" => Unit::Month,
        _ => Unit::Millisecond,
    };
    let interval = Interval::every(unit, step, week_start as usize as f64, zone);
    if (stop - start) / (step.min(unit.cycle()) * unit.duration()) > limit {
        return None;
    }
    interval.ticks(start, stop, limit, zone)
}
