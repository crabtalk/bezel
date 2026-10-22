//! Vega-Lite's `timeUnit` transform: Vega's `timeunit`, which truncates
//! times to calendar parts as vega-time's `timeFloor` and `utcFloor` do.
//!
//! The local zone is UTC, as [`crate::time::parse`] reads it, so a local
//! unit and its `utc` twin agree but where Vega's own differ: a `utc` unit
//! with a year floors a time in the years 0 to 99 to `NaN`, and reads the
//! start of such a year in the 1900s for `week`, `day` and `dayofyear`.

use crate::data::Data;
use gpui::SharedString;
use js::{
    date::{Date, LocalZone},
    time::{Interval, Unit},
};

/// The year a time unit without one floors into.
const REFERENCE_YEAR: f64 = 2012.0;

/// A calendar part a time unit keeps, in Vega's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    Year,
    Quarter,
    Month,
    Week,
    Date,
    /// The day of the week.
    Day,
    DayOfYear,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
}

impl Part {
    /// Vega's names.
    pub const NAMES: [(Part, &'static str); 11] = [
        (Part::Year, "year"),
        (Part::Quarter, "quarter"),
        (Part::Month, "month"),
        (Part::Week, "week"),
        (Part::Date, "date"),
        (Part::Day, "day"),
        (Part::DayOfYear, "dayofyear"),
        (Part::Hours, "hours"),
        (Part::Minutes, "minutes"),
        (Part::Seconds, "seconds"),
        (Part::Milliseconds, "milliseconds"),
    ];

    pub fn name(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(part, _)| *part == self)
            .map(|(_, name)| *name)
            .expect("every part is named")
    }

    /// The d3-time interval a time floored to this part ends one step of.
    fn interval(self) -> Interval {
        let zone = LocalZone::utc();
        let unit = match self {
            Part::Year => Unit::Year,
            Part::Quarter => return Interval::every(Unit::Month, 3.0, 0.0, &zone),
            Part::Month => Unit::Month,
            Part::Week => Unit::Week,
            Part::Date | Part::Day | Part::DayOfYear => Unit::Day,
            Part::Hours => Unit::Hour,
            Part::Minutes => Unit::Minute,
            Part::Seconds => Unit::Second,
            Part::Milliseconds => Unit::Millisecond,
        };
        Interval::new(unit, 0.0)
    }
}

/// Each row's `field` truncated to `units`, and the end of that step, as
/// `output`.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeUnit {
    pub field: SharedString,
    /// Ascending, at least one.
    pub units: Vec<Part>,
    /// How many of the last unit a step takes.
    pub step: f64,
    /// Read on the UTC calendar rather than the local one.
    pub utc: bool,
    pub output: [SharedString; 2],
}

impl TimeUnit {
    /// Vega's defaults: a step of 1, the local calendar, output as `unit0`
    /// and `unit1`.
    pub fn new(field: impl Into<SharedString>, units: Vec<Part>) -> Self {
        Self {
            field: field.into(),
            units,
            step: 1.0,
            utc: false,
            output: ["unit0".into(), "unit1".into()],
        }
    }

    pub fn apply(&self, data: &Data) -> Data {
        let values = data.numbers(&self.field);
        let step = self.step;
        let zone = LocalZone::utc();
        let floor = Floor {
            units: &self.units,
            step,
            utc: self.utc,
            zone: &zone,
        };
        let interval = self
            .units
            .last()
            .map_or(Interval::new(Unit::Millisecond, 0.0), |u| u.interval());
        let starts: Vec<f64> = values.iter().map(|&v| floor.floor(v)).collect();
        let ends: Vec<f64> = starts
            .iter()
            .map(|&start| {
                let mut end = zone.date(start);
                interval.offset(&mut end, step.floor());
                end.time()
            })
            .collect();
        let [low, high] = self.output.clone();
        data.clone().times(low, starts).times(high, ends)
    }
}

/// vega-time's floor of `units` with the last one stepped by `step`.
struct Floor<'a> {
    units: &'a [Part],
    step: f64,
    utc: bool,
    zone: &'a LocalZone,
}

impl Floor<'_> {
    fn has(&self, part: Part) -> bool {
        self.units.contains(&part)
    }

    fn floor(&self, time: f64) -> f64 {
        let d = self.zone.date(time);
        let year = match self.has(Part::Year) {
            true => self.unit(Part::Year, None, d.full_year()),
            false => REFERENCE_YEAR,
        };
        let month = if self.has(Part::Month) {
            self.unit(Part::Month, None, d.month())
        } else if self.has(Part::Quarter) {
            3.0 * self.unit(Part::Quarter, None, (d.month() / 3.0).floor())
        } else {
            0.0
        };
        let first = || self.first(year);
        let date = if self.has(Part::Week) && self.has(Part::Day) {
            self.unit(
                Part::Day,
                Some(1.0),
                weekday(self.week(d), d.day(), first()),
            )
        } else if self.has(Part::Week) {
            weekday(self.unit(Part::Week, Some(1.0), self.week(d)), 0.0, first())
        } else if self.has(Part::Day) {
            self.unit(Part::Day, Some(1.0), weekday(1.0, d.day(), first()))
        } else if self.has(Part::Date) {
            self.unit(Part::Date, Some(1.0), d.date())
        } else if self.has(Part::DayOfYear) {
            self.unit(Part::DayOfYear, Some(1.0), self.day_of_year(d))
        } else {
            1.0
        };
        let [hours, minutes, seconds, ms] = [
            (Part::Hours, d.hours()),
            (Part::Minutes, d.minutes()),
            (Part::Seconds, d.seconds()),
            (Part::Milliseconds, d.milliseconds()),
        ]
        .map(|(part, value)| match self.has(part) {
            true => self.unit(part, None, value),
            false => 0.0,
        });
        match self.utc {
            true if (0.0..100.0).contains(&year) => f64::NAN,
            true => Date::utc(year, month, date, hours, minutes, seconds, ms),
            false if (0.0..100.0).contains(&year) => {
                let mut date = self
                    .zone
                    .local(-1.0, month, date, hours, minutes, seconds, ms);
                date.set_full_year(year);
                date.time()
            }
            false => self
                .zone
                .local(year, month, date, hours, minutes, seconds, ms)
                .time(),
        }
    }

    /// `value` of `part` stepped, from `phase`, where `part` is the last unit.
    fn unit(&self, part: Part, phase: Option<f64>, value: f64) -> f64 {
        let step = self.step;
        if self.units.last() != Some(&part) || step <= 1.0 {
            return value;
        }
        match phase {
            Some(phase) => phase + step * ((value - phase) / step).floor(),
            None => step * (value / step).floor(),
        }
    }

    /// The start of `year`.
    fn year_start(&self, year: f64) -> Date<'_> {
        match self.utc {
            true => self
                .zone
                .date(Date::utc(year, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0)),
            false => {
                let mut d = self.zone.date(0.0);
                d.set_full_year(year);
                d.set_month_date(0.0, 1.0);
                d.set_hours(0.0, 0.0, 0.0, 0.0);
                d
            }
        }
    }

    /// The day of the week `year` starts on, 0 for Sunday.
    fn first(&self, year: f64) -> f64 {
        self.year_start(year).day()
    }

    /// Sunday-based weeks of `d`'s year before it.
    fn week(&self, d: Date) -> f64 {
        let start = self.year_start(d.full_year());
        Interval::new(Unit::Week, 0.0).count(start.at(start.time() - 1.0), d)
    }

    fn day_of_year(&self, d: Date) -> f64 {
        let start = self.year_start(d.full_year());
        Interval::new(Unit::Day, 0.0).count(start.at(start.time() - 1.0), d)
    }
}

/// The day of the year of `day` of the week in `week`, the year starting on
/// day `first` of the week.
fn weekday(week: f64, day: f64, first: f64) -> f64 {
    day + week * 7.0 - (first + 6.0) % 7.0
}
