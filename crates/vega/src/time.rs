//! vega-time: time units, the floor of a time to a set of them, and their
//! d3-time intervals, in the host's zone or in UTC.

use crate::Error;
use d3::time::{
    Interval, Unit as D3Unit,
    date::{Date, LocalZone},
};

/// The year a floor without a year unit puts a time in: 2012 is a leap year
/// that begins on a Sunday.
const REFERENCE_YEAR: f64 = 2012.0;
/// The year a floor with `isoweek` and no year puts a time in: 2015 has an
/// ISO week 53.
const ISOWEEK_REFERENCE_YEAR: f64 = 2015.0;

/// A Vega time unit, in Vega's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unit {
    Year,
    Quarter,
    Month,
    Week,
    Isoweek,
    Date,
    /// The day of the week.
    Day,
    Dayofyear,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
}

const NAMES: [(Unit, &str); 12] = [
    (Unit::Year, "year"),
    (Unit::Quarter, "quarter"),
    (Unit::Month, "month"),
    (Unit::Week, "week"),
    (Unit::Isoweek, "isoweek"),
    (Unit::Date, "date"),
    (Unit::Day, "day"),
    (Unit::Dayofyear, "dayofyear"),
    (Unit::Hours, "hours"),
    (Unit::Minutes, "minutes"),
    (Unit::Seconds, "seconds"),
    (Unit::Milliseconds, "milliseconds"),
];

impl Unit {
    pub fn parse(name: &str) -> Result<Self, Error> {
        NAMES
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(unit, _)| *unit)
            .ok_or_else(|| Error::Spec(format!("Invalid time unit: {name}.")))
    }

    pub fn name(self) -> &'static str {
        NAMES
            .iter()
            .find(|(unit, _)| *unit == self)
            .map(|(_, n)| *n)
            .expect("every unit is named")
    }

    /// `timeInterval(unit)` and `utcInterval(unit)`: the zone is the date's.
    pub fn interval(self, zone: &LocalZone) -> Interval {
        let (unit, step, week_start) = match self {
            Unit::Year => (D3Unit::Year, 1.0, 0.0),
            Unit::Quarter => (D3Unit::Month, 3.0, 0.0),
            Unit::Month => (D3Unit::Month, 1.0, 0.0),
            Unit::Week => (D3Unit::Week, 1.0, 0.0),
            Unit::Isoweek => (D3Unit::Week, 1.0, 1.0),
            Unit::Date | Unit::Day | Unit::Dayofyear => (D3Unit::Day, 1.0, 0.0),
            Unit::Hours => (D3Unit::Hour, 1.0, 0.0),
            Unit::Minutes => (D3Unit::Minute, 1.0, 0.0),
            Unit::Seconds => (D3Unit::Second, 1.0, 0.0),
            Unit::Milliseconds => (D3Unit::Millisecond, 1.0, 0.0),
        };
        Interval::every(unit, step, week_start, zone)
    }
}

/// `timeUnits`: the units checked and in Vega's order.
pub fn units(names: &[String]) -> Result<Vec<Unit>, Error> {
    if names.is_empty() {
        return Err(Error::Spec("Missing time unit.".into()));
    }
    let mut units = names
        .iter()
        .map(|n| Unit::parse(n))
        .collect::<Result<Vec<_>, _>>()?;
    let has = |u: Unit| units.contains(&u);
    let kinds = usize::from(has(Unit::Week) || has(Unit::Isoweek) || has(Unit::Day))
        + usize::from(has(Unit::Quarter) || has(Unit::Month) || has(Unit::Date))
        + usize::from(has(Unit::Dayofyear));
    if kinds > 1 || (has(Unit::Week) && has(Unit::Isoweek)) {
        return Err(Error::Spec(format!(
            "Incompatible time units: {}",
            names.join(",")
        )));
    }
    units.sort();
    Ok(units)
}

/// `timeFloor(units, step)` or `utcFloor(units, step)`: a time truncated to
/// `units`, the last of them stepped by `step`.
pub struct Floor {
    units: Vec<Unit>,
    step: f64,
    utc: bool,
    zone: LocalZone,
}

impl Floor {
    /// `units` as [`units`] orders them.
    pub fn new(units: Vec<Unit>, step: f64, utc: bool) -> Self {
        Self {
            units,
            step: if step == 0.0 || step.is_nan() {
                1.0
            } else {
                step
            },
            utc,
            zone: if utc {
                LocalZone::utc()
            } else {
                LocalZone::default()
            },
        }
    }

    /// The zone the floor reads dates in.
    pub fn zone(&self) -> &LocalZone {
        &self.zone
    }

    fn has(&self, unit: Unit) -> bool {
        self.units.contains(&unit)
    }

    pub fn floor(&self, time: f64) -> f64 {
        let d = self.zone.date(time);
        let year = if self.has(Unit::Year) {
            let year = match self.has(Unit::Isoweek) {
                true => self.iso_week_year(d),
                false => d.full_year(),
            };
            self.unit(Unit::Year, None, year)
        } else if self.has(Unit::Isoweek) {
            ISOWEEK_REFERENCE_YEAR
        } else {
            REFERENCE_YEAR
        };
        let month = if self.has(Unit::Month) {
            self.unit(Unit::Month, None, d.month())
        } else if self.has(Unit::Quarter) {
            3.0 * self.unit(Unit::Quarter, None, (d.month() / 3.0).floor())
        } else {
            0.0
        };
        let date = if self.has(Unit::Week) && self.has(Unit::Day) {
            let day = weekday(self.week(d), d.day(), self.first(year));
            self.unit(Unit::Day, Some(1.0), day)
        } else if self.has(Unit::Isoweek) && self.has(Unit::Day) {
            let day =
                self.iso_week_one_date(year) + (self.iso_week(d) - 1.0) * 7.0 + iso_day(d.day());
            self.unit(Unit::Day, Some(1.0), day)
        } else if self.has(Unit::Week) {
            let week = self.unit(Unit::Week, Some(1.0), self.week(d));
            weekday(week, 0.0, self.first(year))
        } else if self.has(Unit::Isoweek) {
            let week = self.unit(Unit::Isoweek, Some(1.0), self.iso_week(d));
            self.iso_week_one_date(year) + (week - 1.0) * 7.0
        } else if self.has(Unit::Day) {
            self.unit(
                Unit::Day,
                Some(1.0),
                weekday(1.0, d.day(), self.first(year)),
            )
        } else if self.has(Unit::Date) {
            self.unit(Unit::Date, Some(1.0), d.date())
        } else if self.has(Unit::Dayofyear) {
            self.unit(Unit::Dayofyear, Some(1.0), self.day_of_year(d))
        } else {
            1.0
        };
        let [hours, minutes, seconds, ms] = [
            (Unit::Hours, d.hours()),
            (Unit::Minutes, d.minutes()),
            (Unit::Seconds, d.seconds()),
            (Unit::Milliseconds, d.milliseconds()),
        ]
        .map(|(unit, value)| match self.has(unit) {
            true => self.unit(unit, None, value),
            false => 0.0,
        });
        // `utcDate` sets the year of a date in the years 0 to 99 from a
        // number's `y`, which is `NaN`.
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

    /// `value` of `unit` stepped, from `phase`, where `unit` is the last.
    fn unit(&self, unit: Unit, phase: Option<f64>, value: f64) -> f64 {
        let step = self.step;
        if self.units.last() != Some(&unit) || step <= 1.0 {
            return value;
        }
        match phase {
            Some(phase) => phase + step * ((value - phase) / step).floor(),
            None => step * (value / step).floor(),
        }
    }

    /// The start of `year`: `localYear(year)`, or `Date.UTC(year, 0, 1)`,
    /// which reads the years 0 to 99 in the 1900s.
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
        Interval::new(D3Unit::Week, 0.0).count(start.at(start.time() - 1.0), d)
    }

    fn day_of_year(&self, d: Date) -> f64 {
        let start = self.year_start(d.full_year());
        Interval::new(D3Unit::Day, 0.0).count(start.at(start.time() - 1.0), d)
    }

    /// The year of `d`'s ISO week: the year of the Thursday of its week.
    fn iso_week_year(&self, d: Date) -> f64 {
        let mut monday = d;
        Interval::new(D3Unit::Week, 1.0).floor(&mut monday);
        Interval::new(D3Unit::Day, 0.0).offset(&mut monday, 3.0);
        monday.full_year()
    }

    /// The Monday on which week 1 of the ISO week year `year` begins.
    fn iso_week_one(&self, year: f64) -> Date<'_> {
        let mut d = self.year_start(year);
        Interval::new(D3Unit::Day, 0.0).offset(&mut d, 3.0);
        Interval::new(D3Unit::Week, 1.0).floor(&mut d);
        d
    }

    /// `d`'s ISO week, from 1.
    fn iso_week(&self, d: Date) -> f64 {
        let one = self.iso_week_one(self.iso_week_year(d));
        1.0 + Interval::new(D3Unit::Week, 1.0).count(one, d)
    }

    /// The day of January on which week 1 of `year` begins; 0 or less
    /// reaches into the December before.
    fn iso_week_one_date(&self, year: f64) -> f64 {
        let d = self.iso_week_one(year);
        match d.month() != 0.0 {
            true => d.date() - 31.0,
            false => d.date(),
        }
    }
}

/// The day of the year of `day` of the week in `week`, the year starting on
/// day `first` of the week.
fn weekday(week: f64, day: f64, first: f64) -> f64 {
    day + week * 7.0 - (first + 6.0) % 7.0
}

/// The day within an ISO week, Monday 0 to Sunday 6.
fn iso_day(day: f64) -> f64 {
    (day + 6.0) % 7.0
}

const SECOND: f64 = 1000.0;
const MINUTE: f64 = 60.0 * SECOND;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;
const WEEK: f64 = 7.0 * DAY;
const MONTH: f64 = 30.0 * DAY;
const YEAR: f64 = 365.0 * DAY;

/// `timeBin`'s candidates: units, step and the step's duration.
const BINS: [(&[Unit], f64, f64); 17] = {
    use Unit::*;
    const SECONDS: &[Unit] = &[Year, Month, Date, Hours, Minutes, Seconds];
    const MINUTES: &[Unit] = &[Year, Month, Date, Hours, Minutes];
    const HOURS: &[Unit] = &[Year, Month, Date, Hours];
    [
        (SECONDS, 1.0, SECOND),
        (SECONDS, 5.0, 5.0 * SECOND),
        (SECONDS, 15.0, 15.0 * SECOND),
        (SECONDS, 30.0, 30.0 * SECOND),
        (MINUTES, 1.0, MINUTE),
        (MINUTES, 5.0, 5.0 * MINUTE),
        (MINUTES, 15.0, 15.0 * MINUTE),
        (MINUTES, 30.0, 30.0 * MINUTE),
        (HOURS, 1.0, HOUR),
        (HOURS, 3.0, 3.0 * HOUR),
        (HOURS, 6.0, 6.0 * HOUR),
        (HOURS, 12.0, 12.0 * HOUR),
        (&[Year, Month, Date], 1.0, DAY),
        (&[Year, Week], 1.0, WEEK),
        (&[Year, Month], 1.0, MONTH),
        (&[Year, Month], 3.0, 3.0 * MONTH),
        (&[Year], 1.0, YEAR),
    ]
};

/// `timeBin({extent, maxbins})`: the units and step that cut `extent` into
/// about `maxbins` bins.
pub fn bin(extent: [f64; 2], maxbins: f64) -> (Vec<Unit>, f64) {
    let max = if maxbins == 0.0 || maxbins.is_nan() {
        40.0
    } else {
        maxbins
    };
    let target = (extent[1] - extent[0]).abs() / max;
    // d3's `bisector(...).right`.
    let i = BINS.partition_point(|(_, _, duration)| *duration <= target);
    if i == BINS.len() {
        let step = d3::array::tick_step(extent[0] / YEAR, extent[1] / YEAR, max);
        (vec![Unit::Year], step)
    } else if i > 0 {
        let pick = match target / BINS[i - 1].2 < BINS[i].2 / target {
            true => BINS[i - 1],
            false => BINS[i],
        };
        (pick.0.to_vec(), pick.1)
    } else {
        use Unit::*;
        let step = d3::array::tick_step(extent[0], extent[1], max).max(1.0);
        (
            vec![Year, Month, Date, Hours, Minutes, Seconds, Milliseconds],
            step,
        )
    }
}

/// `timeUnitSpecifier(units, specifiers)`: a d3 time format for the units,
/// the longest run of them with a specifier taken at each step, `specifiers`
/// in place of the defaults.
pub fn specifier(units: &[String], specifiers: &[(String, String)]) -> Result<String, Error> {
    const DEFAULTS: [(&str, &str); 16] = [
        ("year", "%Y "),
        ("quarter", "Q%q "),
        ("month", "%b "),
        ("date", "%d "),
        ("week", "W%U "),
        ("isoweek", "W%V "),
        ("day", "%a "),
        ("dayofyear", "%j "),
        ("hours", "%H:00"),
        ("minutes", "00:%M"),
        ("seconds", ":%S"),
        ("milliseconds", ".%L"),
        ("year-isoweek", "%G W%V "),
        ("year-month", "%Y-%m "),
        ("year-month-date", "%Y-%m-%d "),
        ("hours-minutes", "%H:%M"),
    ];
    let units: Vec<&str> = self::units(units)?.into_iter().map(Unit::name).collect();
    let lookup = |key: &str| {
        specifiers
            .iter()
            .rfind(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .or_else(|| DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v))
    };
    let mut format = String::new();
    let mut start = 0;
    while start < units.len() {
        let run = (start + 1..=units.len())
            .rev()
            .find_map(|end| Some((end, lookup(&units[start..end].join("-"))?)));
        let Some((end, specifier)) = run else {
            return Err(Error::Spec(format!("No time format for {}", units[start])));
        };
        format += specifier;
        start = end;
    }
    Ok(format.trim().to_string())
}

/// `detectTimeUnits`: the finest of the candidate units and steps every one
/// of `times` is aligned to, a week only where a coarser unit is not.
pub fn detect(times: &[f64], utc: bool) -> Result<(Vec<Unit>, f64), Error> {
    use Unit::{Date as D, Hours, Milliseconds, Minutes, Month, Seconds, Week, Year};
    let zone = if utc {
        LocalZone::utc()
    } else {
        LocalZone::default()
    };
    if let Some(t) = times.iter().find(|t| t.is_nan()) {
        return Err(Error::Spec(format!("Invalid date: {t}")));
    }
    let dates: Vec<Date> = times.iter().map(|&t| zone.date(t)).collect();
    let all = |f: &dyn Fn(&Date) -> bool| dates.iter().all(f);
    let one_day = || {
        let mut days = dates.iter().map(|d| d.day());
        let first = days.next();
        days.all(|d| Some(d) == first)
    };
    // Each grain: its units and step, whether a coarser one may pass it by,
    // and whether every date is aligned to it.
    let grains: [(&[Unit], f64, bool, bool); 13] = [
        (
            &[Year, Month, D, Hours, Minutes, Seconds, Milliseconds],
            1.0,
            false,
            true,
        ),
        (
            &[Year, Month, D, Hours, Minutes, Seconds],
            1.0,
            false,
            all(&|d| d.milliseconds() == 0.0),
        ),
        (
            &[Year, Month, D, Hours, Minutes],
            1.0,
            false,
            all(&|d| d.seconds() == 0.0),
        ),
        (
            &[Year, Month, D, Hours, Minutes],
            5.0,
            false,
            all(&|d| d.minutes() % 5.0 == 0.0),
        ),
        (
            &[Year, Month, D, Hours, Minutes],
            10.0,
            false,
            all(&|d| d.minutes() % 10.0 == 0.0),
        ),
        (
            &[Year, Month, D, Hours],
            1.0,
            false,
            all(&|d| d.minutes() == 0.0),
        ),
        (&[Year, Month, D], 1.0, false, all(&|d| d.hours() == 0.0)),
        (&[Year, Week], 1.0, true, one_day()),
        (&[Year, Month], 1.0, false, all(&|d| d.date() == 1.0)),
        (&[Year, Month], 3.0, false, all(&|d| d.month() % 3.0 == 0.0)),
        (&[Year], 1.0, false, all(&|d| d.month() == 0.0)),
        (&[Year], 10.0, false, all(&|d| d.full_year() % 10.0 == 0.0)),
        (&[], 0.0, false, false),
    ];
    let mismatch = grains
        .iter()
        .position(|g| !g.3)
        .expect("the last never aligns");
    let required = grains
        .iter()
        .position(|g| !g.2 && !g.3)
        .expect("the last never aligns");
    let index = if required > mismatch + 1 {
        required
    } else {
        mismatch
    };
    let (units, step, _, _) = grains[index - 1];
    Ok((units.to_vec(), step))
}
