//! JavaScript's `Date`: a time value in milliseconds since the epoch, `NaN`
//! for an invalid date, and its calendar fields in a [`LocalZone`]: the
//! host's, as `chrono::Local` reports it, or UTC. `Date.parse` follows V8's
//! parser.
//!
//! On `wasm32-unknown-unknown` the zone is the page's, through `Date`.
//!
//! A [`LocalZone`] caches the host zone's offsets as V8's `DateCache` caches
//! them: two instants at most 19 days apart with the same offset are taken
//! to have no transition between them.

use crate::format::number;
use chrono::{DateTime, Offset, TimeZone, Utc};
use std::cell::Cell;

mod parse;

const MS_PER_DAY: f64 = 86_400_000.0;
/// The largest time value a `Date` holds, either side of the epoch.
const MAX_TIME: f64 = 8.64e15;
/// V8's `kNone` for a parsed field that is absent.
const NONE: i64 = i32::MAX as i64;
/// V8's `kDefaultDSTDeltaInSec`.
const SPAN: f64 = 19.0 * MS_PER_DAY;

/// The months' names in English, January first, as [`Date::month`] numbers
/// them.
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The days' names in English, Sunday first, as [`Date::day`] numbers them.
pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// The zone a [`Date`]'s local fields are read in: the host's, its offsets
/// cached as instants `lo..=hi`, in milliseconds, and the offset all of them
/// have; or UTC, from [`LocalZone::utc`].
#[derive(Debug, Default)]
pub struct LocalZone {
    cache: Cell<Option<(f64, f64, f64)>>,
    utc: bool,
}

/// A copy starts with an empty cache.
impl Clone for LocalZone {
    fn clone(&self) -> Self {
        Self {
            cache: Cell::default(),
            utc: self.utc,
        }
    }
}

impl PartialEq for LocalZone {
    fn eq(&self, other: &Self) -> bool {
        self.utc == other.utc
    }
}

/// Days from 1970-01-01 to `year`-`month`-`day`, the month from 1.
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Year, month from 1, and day of the month of the day `days` after
/// 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `MakeTime`.
fn make_time(hour: f64, min: f64, sec: f64, ms: f64) -> f64 {
    if !(hour.is_finite() && min.is_finite() && sec.is_finite() && ms.is_finite()) {
        return f64::NAN;
    }
    number::to_integer(hour) * 3_600_000.0
        + number::to_integer(min) * 60_000.0
        + number::to_integer(sec) * 1000.0
        + number::to_integer(ms)
}

/// `MakeDay`.
fn make_day(year: f64, month: f64, date: f64) -> f64 {
    if !(year.is_finite() && month.is_finite() && date.is_finite()) {
        return f64::NAN;
    }
    let (y, m, dt) = (
        number::to_integer(year),
        number::to_integer(month),
        number::to_integer(date),
    );
    let ym = y + (m / 12.0).floor();
    // Past this no day of the year can be within a `Date`'s range.
    if ym.abs() > 400_000.0 {
        return f64::NAN;
    }
    let mn = m.rem_euclid(12.0);
    days_from_civil(ym as i64, mn as i64 + 1, 1) as f64 + dt - 1.0
}

/// `MakeDate`.
fn make_date(day: f64, time: f64) -> f64 {
    if !(day.is_finite() && time.is_finite()) {
        return f64::NAN;
    }
    let tv = day * MS_PER_DAY + time;
    if tv.is_finite() { tv } else { f64::NAN }
}

/// `TimeClip`.
fn time_clip(time: f64) -> f64 {
    if !time.is_finite() || time.abs() > MAX_TIME {
        return f64::NAN;
    }
    number::to_integer(time)
}

/// The year mapping of `new Date(y, …)` and `Date.UTC`: 0 to 99 are 1900s.
fn full_year(year: f64) -> f64 {
    if year.is_nan() {
        return year;
    }
    let y = number::to_integer(year);
    if (0.0..=99.0).contains(&y) {
        1900.0 + y
    } else {
        year
    }
}

/// The calendar date of a time value: year, month from 1, day of the month.
fn civil(tv: f64) -> (i64, i64, i64) {
    civil_from_days((tv / MS_PER_DAY).floor() as i64)
}

fn within_day(tv: f64) -> f64 {
    tv.rem_euclid(MS_PER_DAY)
}

/// The host zone's offset from UTC at the instant `t`, in milliseconds, as
/// chrono reads it. `0` outside chrono's range.
fn zone_offset(t: f64) -> f64 {
    let Some(utc) = DateTime::from_timestamp_millis(t as i64) else {
        return 0.0;
    };
    let offset = chrono::Local.offset_from_utc_datetime(&utc.naive_utc());
    f64::from(offset.fix().local_minus_utc()) * 1000.0
}

impl LocalZone {
    /// The zone with offset 0: local fields, `new Date(y, …)` and
    /// `Date.parse` of a string without an offset read as UTC.
    pub fn utc() -> Self {
        Self {
            cache: Cell::default(),
            utc: true,
        }
    }

    /// [`zone_offset`] through the cache: the `LocalTZA(t, true)` of
    /// `LocalTime(t)`.
    fn offset_at(&self, t: f64) -> f64 {
        if self.utc {
            return 0.0;
        }
        let cached = self.cache.get();
        if let Some((lo, hi, offset)) = cached
            && (lo..=hi).contains(&t)
        {
            return offset;
        }
        let offset = zone_offset(t);
        let span = match cached {
            Some((lo, hi, known)) if known == offset && t > hi && t - hi <= SPAN => (lo, t, offset),
            Some((lo, hi, known)) if known == offset && t < lo && lo - t <= SPAN => (t, hi, offset),
            _ => (t, t, offset),
        };
        self.cache.set(Some(span));
        offset
    }

    /// `LocalTime(t)`.
    fn local_time(&self, t: f64) -> f64 {
        t + self.offset_at(t)
    }

    /// `UTC(t)`: the instant a local time value names. A wall time that occurs
    /// twice is the earlier instant, and one a transition skips takes the offset
    /// in force before the transition.
    fn utc_of(&self, t: f64) -> f64 {
        if !t.is_finite() {
            return f64::NAN;
        }
        if DateTime::from_timestamp_millis(t as i64).is_none() {
            return t;
        }
        // The offsets a day either side; a wall time is an instant under each
        // of them that holds at that instant.
        let before = self.offset_at(t - MS_PER_DAY);
        let after = self.offset_at(t + MS_PER_DAY);
        let holds = |offset: f64| self.offset_at(t - offset) == offset;
        let offset = match (holds(before), holds(after)) {
            (true, true) => before.max(after),
            (false, true) => after,
            _ => before,
        };
        t - offset
    }

    /// `new Date(time)`.
    pub fn date(&self, time: f64) -> Date<'_> {
        Date(time_clip(time), self)
    }

    /// `new Date(year, month, day, hours, minutes, seconds, ms)`, in local
    /// time.
    #[allow(clippy::too_many_arguments)]
    pub fn local(
        &self,
        year: f64,
        month: f64,
        day: f64,
        h: f64,
        m: f64,
        s: f64,
        ms: f64,
    ) -> Date<'_> {
        let day = make_day(full_year(year), month, day);
        Date(
            time_clip(self.utc_of(make_date(day, make_time(h, m, s, ms)))),
            self,
        )
    }

    /// `new Date(text)`.
    pub fn parse(&self, text: &str) -> Date<'_> {
        Date(parse::parse(text, self), self)
    }
}

/// A JavaScript `Date`: a time value and the zone its local fields are read
/// in.
#[derive(Clone, Copy, Debug)]
pub struct Date<'z>(f64, &'z LocalZone);

impl PartialEq for Date<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'z> Date<'z> {
    /// `new Date(time)` in this date's zone.
    pub fn at(self, time: f64) -> Self {
        Self(time_clip(time), self.1)
    }

    pub fn zone(self) -> &'z LocalZone {
        self.1
    }

    /// `Date.UTC(year, month, day, hours, minutes, seconds, ms)`.
    pub fn utc(year: f64, month: f64, day: f64, h: f64, m: f64, s: f64, ms: f64) -> f64 {
        let day = make_day(full_year(year), month, day);
        time_clip(make_date(day, make_time(h, m, s, ms)))
    }

    /// `Date.now()`.
    pub fn now() -> f64 {
        Utc::now().timestamp_millis() as f64
    }

    /// `getTime()`, `+date`.
    pub fn time(self) -> f64 {
        self.0
    }

    pub fn is_valid(self) -> bool {
        !self.0.is_nan()
    }

    /// The local time value, `NaN` for an invalid date.
    fn local_value(self) -> f64 {
        if self.is_valid() {
            self.1.local_time(self.0)
        } else {
            f64::NAN
        }
    }

    fn local_civil(self) -> Option<(i64, i64, i64)> {
        self.is_valid().then(|| civil(self.local_value()))
    }

    pub fn full_year(self) -> f64 {
        self.local_civil().map_or(f64::NAN, |(y, _, _)| y as f64)
    }

    /// `getMonth()`, from 0.
    pub fn month(self) -> f64 {
        self.local_civil()
            .map_or(f64::NAN, |(_, m, _)| (m - 1) as f64)
    }

    /// `getDate()`.
    pub fn date(self) -> f64 {
        self.local_civil().map_or(f64::NAN, |(_, _, d)| d as f64)
    }

    /// `getDay()`: 0 for Sunday.
    pub fn day(self) -> f64 {
        ((self.local_value() / MS_PER_DAY).floor() + 4.0).rem_euclid(7.0)
    }

    pub fn hours(self) -> f64 {
        (within_day(self.local_value()) / 3_600_000.0).floor()
    }

    pub fn minutes(self) -> f64 {
        (within_day(self.local_value()) / 60_000.0).floor() % 60.0
    }

    pub fn seconds(self) -> f64 {
        (within_day(self.local_value()) / 1000.0).floor() % 60.0
    }

    pub fn milliseconds(self) -> f64 {
        within_day(self.local_value()) % 1000.0
    }

    /// `getTimezoneOffset()`: minutes, positive west of UTC, truncated to
    /// whole minutes as V8 does.
    pub fn timezone_offset(self) -> f64 {
        ((self.0 - self.local_value()) / 60_000.0).trunc()
    }

    /// `setTime(time)`.
    pub fn set_time(&mut self, time: f64) {
        self.0 = time_clip(time);
    }

    /// Sets the local date to `day` and the local time of day to `time`.
    fn set_local(&mut self, day: f64, time: f64) {
        self.0 = time_clip(self.1.utc_of(make_date(day, time)));
    }

    /// `setDate(date)`.
    pub fn set_date(&mut self, date: f64) {
        if !self.is_valid() {
            return;
        }
        let day = make_day(self.full_year(), self.month(), date);
        self.set_local(day, within_day(self.local_value()));
    }

    /// `setMonth(month)`.
    pub fn set_month(&mut self, month: f64) {
        if !self.is_valid() {
            return;
        }
        self.set_month_date(month, self.date());
    }

    /// `setMonth(month, date)`.
    pub fn set_month_date(&mut self, month: f64, date: f64) {
        if !self.is_valid() {
            return;
        }
        let day = make_day(self.full_year(), month, date);
        self.set_local(day, within_day(self.local_value()));
    }

    /// `setFullYear(year)`: an invalid date counts as `+0` local time.
    pub fn set_full_year(&mut self, year: f64) {
        let lt = if self.is_valid() {
            self.local_value()
        } else {
            0.0
        };
        let (_, m, d) = civil(lt);
        let day = make_day(year, (m - 1) as f64, d as f64);
        self.set_local(day, within_day(lt));
    }

    /// `setHours(h, m, s, ms)`.
    pub fn set_hours(&mut self, h: f64, m: f64, s: f64, ms: f64) {
        if !self.is_valid() {
            return;
        }
        let day = (self.local_value() / MS_PER_DAY).floor();
        self.set_local(day, make_time(h, m, s, ms));
    }

    /// `toISOString()`, or `None` where it throws.
    pub fn iso_string(self) -> Option<String> {
        if !self.is_valid() {
            return None;
        }
        let (y, m, d) = civil(self.0);
        let tod = within_day(self.0);
        let year = if (0..=9999).contains(&y) {
            format!("{y:04}")
        } else if y < 0 {
            format!("-{:06}", -y)
        } else {
            format!("+{y:06}")
        };
        Some(format!(
            "{year}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
            (tod / 3_600_000.0).floor(),
            (tod / 60_000.0).floor() % 60.0,
            (tod / 1000.0).floor() % 60.0,
            tod % 1000.0
        ))
    }
}
