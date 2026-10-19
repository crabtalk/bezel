//! JavaScript's `Date`: a time value in milliseconds since the epoch, `NaN`
//! for an invalid date, and its calendar fields in the host's time zone, as
//! `chrono::Local` reports it. `Date.parse` follows V8's parser.
//!
//! On `wasm32-unknown-unknown` the zone is the page's, through `Date`.
//!
//! A [`LocalZone`] caches the host zone's offsets as V8's `DateCache` caches
//! them: two instants at most [`SPAN`] apart with the same offset are taken
//! to have no transition between them.

use std::cell::Cell;

use chrono::{DateTime, Offset, TimeZone};

use crate::util::chars::is_space;

const MS_PER_DAY: f64 = 86_400_000.0;
/// The largest time value a `Date` holds, either side of the epoch.
const MAX_TIME: f64 = 8.64e15;
/// V8's `kNone` for a parsed field that is absent.
const NONE: i64 = i32::MAX as i64;
/// V8's `kDefaultDSTDeltaInSec`.
const SPAN: f64 = 19.0 * MS_PER_DAY;

/// The host's time zone, its offsets cached: instants `lo..=hi`, in
/// milliseconds, and the offset all of them have.
#[derive(Debug, Default)]
pub(crate) struct LocalZone(Cell<Option<(f64, f64, f64)>>);

/// A copy starts with an empty cache.
impl Clone for LocalZone {
    fn clone(&self) -> Self {
        Self::default()
    }
}

/// Every zone is the host's.
impl PartialEq for LocalZone {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// `ToIntegerOrInfinity`.
fn integer(x: f64) -> f64 {
    if x.is_nan() { 0.0 } else { x.trunc() + 0.0 }
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Year, month from 1, and day of the month of a day number.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
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
    integer(hour) * 3_600_000.0 + integer(min) * 60_000.0 + integer(sec) * 1000.0 + integer(ms)
}

/// `MakeDay`.
fn make_day(year: f64, month: f64, date: f64) -> f64 {
    if !(year.is_finite() && month.is_finite() && date.is_finite()) {
        return f64::NAN;
    }
    let (y, m, dt) = (integer(year), integer(month), integer(date));
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
    integer(time)
}

/// The year mapping of `new Date(y, …)` and `Date.UTC`: 0 to 99 are 1900s.
fn full_year(year: f64) -> f64 {
    if year.is_nan() {
        return year;
    }
    let y = integer(year);
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
    /// [`zone_offset`] through the cache: the `LocalTZA(t, true)` of
    /// `LocalTime(t)`.
    fn offset_at(&self, t: f64) -> f64 {
        let cached = self.0.get();
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
        self.0.set(Some(span));
        offset
    }

    /// `LocalTime(t)`.
    fn local_time(&self, t: f64) -> f64 {
        t + self.offset_at(t)
    }

    /// `UTC(t)`: the instant a local time value names. A wall time that occurs
    /// twice is the earlier instant, and one a transition skips takes the offset
    /// in force before the transition.
    fn utc(&self, t: f64) -> f64 {
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
    pub(crate) fn date(&self, time: f64) -> Date<'_> {
        Date(time_clip(time), self)
    }

    /// `new Date(year, month, day, hours, minutes, seconds, ms)`, in local
    /// time.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn local(
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
            time_clip(self.utc(make_date(day, make_time(h, m, s, ms)))),
            self,
        )
    }

    /// `new Date(text)`.
    pub(crate) fn parse(&self, text: &str) -> Date<'_> {
        Date(parse::parse(text, self), self)
    }
}

/// A JavaScript `Date`: a time value and the zone its local fields are read
/// in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Date<'z>(f64, &'z LocalZone);

impl PartialEq for Date<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'z> Date<'z> {
    /// `new Date(time)` in this date's zone.
    pub(crate) fn at(self, time: f64) -> Self {
        Self(time_clip(time), self.1)
    }

    pub(crate) fn zone(self) -> &'z LocalZone {
        self.1
    }

    /// `Date.UTC(year, month, day, hours, minutes, seconds, ms)`.
    pub(crate) fn utc(year: f64, month: f64, day: f64, h: f64, m: f64, s: f64, ms: f64) -> f64 {
        let day = make_day(full_year(year), month, day);
        time_clip(make_date(day, make_time(h, m, s, ms)))
    }

    /// `getTime()`, `+date`.
    pub(crate) fn time(self) -> f64 {
        self.0
    }

    pub(crate) fn is_valid(self) -> bool {
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

    pub(crate) fn full_year(self) -> f64 {
        self.local_civil().map_or(f64::NAN, |(y, _, _)| y as f64)
    }

    /// `getMonth()`, from 0.
    pub(crate) fn month(self) -> f64 {
        self.local_civil()
            .map_or(f64::NAN, |(_, m, _)| (m - 1) as f64)
    }

    /// `getDate()`.
    pub(crate) fn date(self) -> f64 {
        self.local_civil().map_or(f64::NAN, |(_, _, d)| d as f64)
    }

    /// `getDay()`: 0 for Sunday.
    pub(crate) fn day(self) -> f64 {
        ((self.local_value() / MS_PER_DAY).floor() + 4.0).rem_euclid(7.0)
    }

    pub(crate) fn hours(self) -> f64 {
        (within_day(self.local_value()) / 3_600_000.0).floor()
    }

    pub(crate) fn minutes(self) -> f64 {
        (within_day(self.local_value()) / 60_000.0).floor() % 60.0
    }

    pub(crate) fn seconds(self) -> f64 {
        (within_day(self.local_value()) / 1000.0).floor() % 60.0
    }

    pub(crate) fn milliseconds(self) -> f64 {
        within_day(self.local_value()) % 1000.0
    }

    /// `getUTCSeconds()`.
    pub(crate) fn utc_seconds(self) -> f64 {
        (within_day(self.0) / 1000.0).floor() % 60.0
    }

    /// `getTimezoneOffset()`: minutes, positive west of UTC.
    pub(crate) fn timezone_offset(self) -> f64 {
        (self.0 - self.local_value()) / 60_000.0
    }

    /// `setTime(time)`.
    pub(crate) fn set_time(&mut self, time: f64) {
        self.0 = time_clip(time);
    }

    /// Sets the local date to `day` and the local time of day to `time`.
    fn set_local(&mut self, day: f64, time: f64) {
        self.0 = time_clip(self.1.utc(make_date(day, time)));
    }

    /// `setDate(date)`.
    pub(crate) fn set_date(&mut self, date: f64) {
        if !self.is_valid() {
            return;
        }
        let day = make_day(self.full_year(), self.month(), date);
        self.set_local(day, within_day(self.local_value()));
    }

    /// `setMonth(month)`.
    pub(crate) fn set_month(&mut self, month: f64) {
        if !self.is_valid() {
            return;
        }
        self.set_month_date(month, self.date());
    }

    /// `setMonth(month, date)`.
    pub(crate) fn set_month_date(&mut self, month: f64, date: f64) {
        if !self.is_valid() {
            return;
        }
        let day = make_day(self.full_year(), month, date);
        self.set_local(day, within_day(self.local_value()));
    }

    /// `setFullYear(year)`: an invalid date counts as `+0` local time.
    pub(crate) fn set_full_year(&mut self, year: f64) {
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
    pub(crate) fn set_hours(&mut self, h: f64, m: f64, s: f64, ms: f64) {
        if !self.is_valid() {
            return;
        }
        let day = (self.local_value() / MS_PER_DAY).floor();
        self.set_local(day, make_time(h, m, s, ms));
    }

    /// `toISOString()`, or `None` where it throws.
    pub(crate) fn iso_string(self) -> Option<String> {
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

/// A time value as `JSON.stringify` writes the `Date` it is, for
/// `serialize_with`: its ISO string, `null` when invalid.
pub(crate) fn json<S: serde::Serializer>(time: &f64, s: S) -> Result<S::Ok, S::Error> {
    match LocalZone::default().date(*time).iso_string() {
        Some(text) => s.serialize_str(&text),
        None => s.serialize_unit(),
    }
}

/// [`json`] for a date that may be unset.
pub(crate) fn json_opt<S: serde::Serializer>(time: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match time {
        Some(time) => json(time, s),
        None => s.serialize_unit(),
    }
}

/// V8's `DateParser`: an ES5 ISO date string, or what its legacy parser
/// makes of anything else.
mod parse {
    use super::{LocalZone, MAX_TIME, NONE, is_space, make_date, make_day, make_time, time_clip};

    const MAX_SIGNIFICANT_DIGITS: usize = 9;
    const MONTH_NAME: u8 = 1;
    const AM_PM: u8 = 2;
    const TIME_ZONE_NAME: u8 = 3;
    const TIME_SEPARATOR: u8 = 4;
    const INVALID: u8 = 0;
    /// Prefix, type and value of every word the parser knows.
    const KEYWORDS: [([u32; 3], u8, i64); 27] = [
        (word(b"jan"), MONTH_NAME, 1),
        (word(b"feb"), MONTH_NAME, 2),
        (word(b"mar"), MONTH_NAME, 3),
        (word(b"apr"), MONTH_NAME, 4),
        (word(b"may"), MONTH_NAME, 5),
        (word(b"jun"), MONTH_NAME, 6),
        (word(b"jul"), MONTH_NAME, 7),
        (word(b"aug"), MONTH_NAME, 8),
        (word(b"sep"), MONTH_NAME, 9),
        (word(b"oct"), MONTH_NAME, 10),
        (word(b"nov"), MONTH_NAME, 11),
        (word(b"dec"), MONTH_NAME, 12),
        (word(b"am\0"), AM_PM, 0),
        (word(b"pm\0"), AM_PM, 12),
        (word(b"ut\0"), TIME_ZONE_NAME, 0),
        (word(b"utc"), TIME_ZONE_NAME, 0),
        (word(b"z\0\0"), TIME_ZONE_NAME, 0),
        (word(b"gmt"), TIME_ZONE_NAME, 0),
        (word(b"cdt"), TIME_ZONE_NAME, -5),
        (word(b"cst"), TIME_ZONE_NAME, -6),
        (word(b"edt"), TIME_ZONE_NAME, -4),
        (word(b"est"), TIME_ZONE_NAME, -5),
        (word(b"mdt"), TIME_ZONE_NAME, -6),
        (word(b"mst"), TIME_ZONE_NAME, -7),
        (word(b"pdt"), TIME_ZONE_NAME, -7),
        (word(b"pst"), TIME_ZONE_NAME, -8),
        (word(b"t\0\0"), TIME_SEPARATOR, 0),
    ];

    const fn word(bytes: &[u8; 3]) -> [u32; 3] {
        [bytes[0] as u32, bytes[1] as u32, bytes[2] as u32]
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Token {
        Invalid,
        End,
        Number { value: i64, length: usize },
        Symbol(u8),
        WhiteSpace,
        Keyword { kind: u8, value: i64, length: usize },
        Unknown,
    }

    impl Token {
        fn is_number(self) -> bool {
            matches!(self, Token::Number { .. })
        }
        fn is_fixed(self, n: usize) -> bool {
            matches!(self, Token::Number { length, .. } if length == n)
        }
        fn number(self) -> i64 {
            match self {
                Token::Number { value, .. } => value,
                _ => 0,
            }
        }
        fn is_symbol(self, c: u8) -> bool {
            self == Token::Symbol(c)
        }
        fn is_sign(self) -> bool {
            self.is_symbol(b'+') || self.is_symbol(b'-')
        }
        fn sign(self) -> i64 {
            if self.is_symbol(b'-') { -1 } else { 1 }
        }
        fn is_keyword_z(self) -> bool {
            matches!(
                self,
                Token::Keyword {
                    kind: TIME_ZONE_NAME,
                    value: 0,
                    length: 1
                }
            )
        }
        fn is_keyword_kind(self, k: u8) -> bool {
            matches!(self, Token::Keyword { kind, .. } if kind == k)
        }
    }

    /// Reads tokens one ahead, over UTF-16 units as V8 does.
    struct Scanner {
        units: Vec<u16>,
        pos: usize,
        next: Token,
    }

    impl Scanner {
        fn new(text: &str) -> Self {
            let mut scanner = Self {
                units: text.encode_utf16().collect(),
                pos: 0,
                next: Token::End,
            };
            scanner.next = scanner.scan();
            scanner
        }

        fn ch(&self) -> u32 {
            self.units.get(self.pos).map_or(0, |&u| u32::from(u))
        }

        fn at_end(&self) -> bool {
            self.pos >= self.units.len()
        }

        fn is_digit(&self) -> bool {
            !self.at_end() && (48..=57).contains(&self.ch())
        }

        fn is_white(&self) -> bool {
            !self.at_end() && is_space(self.ch())
        }

        fn scan(&mut self) -> Token {
            let start = self.pos;
            if self.at_end() {
                return Token::End;
            }
            if self.is_digit() {
                while self.ch() == 48 && !self.at_end() {
                    self.pos += 1;
                }
                let mut n = 0i64;
                let mut i = 0;
                while self.is_digit() {
                    if i < MAX_SIGNIFICANT_DIGITS {
                        n = n * 10 + i64::from(self.ch() - 48);
                    }
                    i += 1;
                    self.pos += 1;
                }
                return Token::Number {
                    value: n,
                    length: self.pos - start,
                };
            }
            let c = self.ch();
            if matches!(c, 58 | 45 | 43 | 46 | 41) {
                self.pos += 1;
                return Token::Symbol(c as u8);
            }
            if c >= 65 && !self.is_white() {
                let mut prefix = [0u32; 3];
                let mut length = 0;
                while !self.at_end() && self.ch() >= 65 && !self.is_white() {
                    if length < 3 {
                        prefix[length] = self.ch() | 0x20;
                    }
                    length += 1;
                    self.pos += 1;
                }
                let found = KEYWORDS.iter().find(|(word, kind, _)| {
                    *word == prefix && (length <= 3 || *kind == MONTH_NAME)
                });
                let (kind, value) = found.map_or((INVALID, 0), |&(_, kind, value)| (kind, value));
                return Token::Keyword {
                    kind,
                    value,
                    length,
                };
            }
            if self.is_white() {
                while self.is_white() {
                    self.pos += 1;
                }
                return Token::WhiteSpace;
            }
            if c == 40 {
                let mut balance = 0;
                loop {
                    match self.ch() {
                        41 => balance -= 1,
                        40 => balance += 1,
                        _ => {}
                    }
                    self.pos += 1;
                    if balance == 0 || self.at_end() {
                        break;
                    }
                }
                return Token::Unknown;
            }
            self.pos += 1;
            Token::Unknown
        }

        fn next(&mut self) -> Token {
            let token = self.next;
            self.next = self.scan();
            token
        }

        fn peek(&self) -> Token {
            self.next
        }

        fn skip_symbol(&mut self, c: u8) -> bool {
            if self.next.is_symbol(c) {
                self.next();
                true
            } else {
                false
            }
        }
    }

    fn between(x: i64, low: i64, high: i64) -> bool {
        (low..=high).contains(&x)
    }

    #[derive(Default)]
    struct Day {
        comp: [i64; 3],
        index: usize,
        named_month: Option<i64>,
        iso: bool,
    }

    impl Day {
        fn add(&mut self, n: i64) -> bool {
            if self.index == 3 {
                return false;
            }
            self.comp[self.index] = n;
            self.index += 1;
            true
        }

        fn write(&mut self) -> Option<(i64, i64, i64)> {
            if self.index < 1 {
                return None;
            }
            while self.index < 3 {
                self.comp[self.index] = 1;
                self.index += 1;
            }
            let c = self.comp;
            let is_day = |x: i64| between(x, 1, 31);
            let (mut year, month, day);
            match self.named_month {
                None => {
                    if self.iso || !is_day(c[0]) {
                        (year, month, day) = (c[0], c[1], c[2]);
                    } else {
                        (year, month, day) = (c[2], c[0], c[1]);
                    }
                }
                Some(named) => {
                    month = named;
                    if !is_day(c[0]) {
                        (year, day) = (c[0], c[1]);
                    } else {
                        (year, day) = (c[1], c[0]);
                    }
                }
            }
            if !self.iso {
                if between(year, 0, 49) {
                    year += 2000;
                } else if between(year, 50, 99) {
                    year += 1900;
                }
            }
            if !between(year, -(1 << 30), (1 << 30) - 1) || !between(month, 1, 12) || !is_day(day) {
                return None;
            }
            Some((year, month - 1, day))
        }
    }

    #[derive(Default)]
    struct Time {
        comp: [i64; 4],
        index: usize,
        hour_offset: Option<i64>,
    }

    impl Time {
        fn is_empty(&self) -> bool {
            self.index == 0
        }

        fn is_expecting(&self, n: i64) -> bool {
            (self.index == 1 && between(n, 0, 59))
                || (self.index == 2 && between(n, 0, 59))
                || (self.index == 3 && between(n, 0, 999))
        }

        fn add(&mut self, n: i64) -> bool {
            if self.index < 4 {
                self.comp[self.index] = n;
                self.index += 1;
                true
            } else {
                false
            }
        }

        fn add_final(&mut self, n: i64) -> bool {
            if !self.add(n) {
                return false;
            }
            while self.index < 4 {
                self.comp[self.index] = 0;
                self.index += 1;
            }
            true
        }

        fn write(&mut self) -> Option<[i64; 4]> {
            while self.index < 4 {
                self.comp[self.index] = 0;
                self.index += 1;
            }
            let [mut hour, minute, second, ms] = self.comp;
            if let Some(offset) = self.hour_offset {
                if !between(hour, 0, 12) {
                    return None;
                }
                hour %= 12;
                hour += offset;
            }
            if (!between(hour, 0, 23)
                || !between(minute, 0, 59)
                || !between(second, 0, 59)
                || !between(ms, 0, 999))
                && (hour != 24 || minute != 0 || second != 0 || ms != 0)
            {
                return None;
            }
            Some([hour, minute, second, ms])
        }
    }

    struct Zone {
        sign: i64,
        hour: i64,
        minute: i64,
    }

    impl Zone {
        fn set(&mut self, hours: i64) {
            self.sign = if hours < 0 { -1 } else { 1 };
            self.hour = hours * self.sign;
            self.minute = 0;
        }

        fn is_expecting(&self, n: i64) -> bool {
            self.hour != NONE && self.minute == NONE && between(n, 0, 59)
        }

        fn is_utc(&self) -> bool {
            self.hour == 0 && self.minute == 0
        }

        fn is_empty(&self) -> bool {
            self.hour == NONE
        }

        /// The offset in seconds; `None` for local time; `Err` when invalid.
        fn write(&mut self) -> Result<Option<i64>, ()> {
            if self.sign == NONE {
                return Ok(None);
            }
            if self.hour == NONE {
                self.hour = 0;
            }
            if self.minute == NONE {
                self.minute = 0;
            }
            let total = self.hour * 3600 + self.minute * 60;
            if !(0..=i64::from(i32::MAX >> 1)).contains(&total) {
                return Err(());
            }
            Ok(Some(if self.sign < 0 { -total } else { total }))
        }
    }

    fn read_milliseconds(token: Token) -> i64 {
        let Token::Number { value, length } = token else {
            return 0;
        };
        let mut number = value;
        let mut length = length;
        if length == 1 {
            number *= 100;
        } else if length == 2 {
            number *= 10;
        } else if length > 3 {
            length = length.min(MAX_SIGNIFICANT_DIGITS);
            let mut factor = 1;
            loop {
                factor *= 10;
                length -= 1;
                if length <= 3 {
                    break;
                }
            }
            number /= factor;
        }
        number
    }

    fn es5(scanner: &mut Scanner, day: &mut Day, time: &mut Time, zone: &mut Zone) -> Token {
        if scanner.peek().is_sign() {
            let sign_token = scanner.next();
            if !scanner.peek().is_fixed(6) {
                return sign_token;
            }
            let sign = sign_token.sign();
            let year = scanner.next().number();
            if sign < 0 && year == 0 {
                return sign_token;
            }
            day.add(sign * year);
        } else if scanner.peek().is_fixed(4) {
            day.add(scanner.next().number());
        } else {
            return scanner.next();
        }
        if scanner.skip_symbol(b'-') {
            if !scanner.peek().is_fixed(2) || !between(scanner.peek().number(), 1, 12) {
                return scanner.next();
            }
            day.add(scanner.next().number());
            if scanner.skip_symbol(b'-') {
                if !scanner.peek().is_fixed(2) || !between(scanner.peek().number(), 1, 31) {
                    return scanner.next();
                }
                day.add(scanner.next().number());
            }
        }
        if !scanner.peek().is_keyword_kind(TIME_SEPARATOR) {
            if scanner.peek() != Token::End {
                return scanner.next();
            }
        } else {
            scanner.next();
            if !scanner.peek().is_fixed(2) || !between(scanner.peek().number(), 0, 24) {
                return Token::Invalid;
            }
            let hour_is_24 = scanner.peek().number() == 24;
            time.add(scanner.next().number());
            if !scanner.skip_symbol(b':') {
                return Token::Invalid;
            }
            if !scanner.peek().is_fixed(2)
                || !between(scanner.peek().number(), 0, 59)
                || (hour_is_24 && scanner.peek().number() > 0)
            {
                return Token::Invalid;
            }
            time.add(scanner.next().number());
            if scanner.skip_symbol(b':') {
                if !scanner.peek().is_fixed(2)
                    || !between(scanner.peek().number(), 0, 59)
                    || (hour_is_24 && scanner.peek().number() > 0)
                {
                    return Token::Invalid;
                }
                time.add(scanner.next().number());
                if scanner.skip_symbol(b'.') {
                    if !scanner.peek().is_number() || (hour_is_24 && scanner.peek().number() > 0) {
                        return Token::Invalid;
                    }
                    time.add(read_milliseconds(scanner.next()));
                }
            }
            if scanner.peek().is_keyword_z() {
                scanner.next();
                zone.set(0);
            } else if scanner.peek().is_sign() {
                zone.sign = scanner.next().sign();
                if scanner.peek().is_fixed(4) {
                    let hourmin = scanner.next().number();
                    let (hour, min) = (hourmin / 100, hourmin % 100);
                    if !between(hour, 0, 23) || !between(min, 0, 59) {
                        return Token::Invalid;
                    }
                    zone.hour = hour;
                    zone.minute = min;
                } else {
                    if !scanner.peek().is_fixed(2) || !between(scanner.peek().number(), 0, 23) {
                        return Token::Invalid;
                    }
                    zone.hour = scanner.next().number();
                    if !scanner.skip_symbol(b':') {
                        return Token::Invalid;
                    }
                    if !scanner.peek().is_fixed(2) || !between(scanner.peek().number(), 0, 59) {
                        return Token::Invalid;
                    }
                    zone.minute = scanner.next().number();
                }
            }
            if scanner.peek() != Token::End {
                return Token::Invalid;
            }
        }
        // Date-only forms without an offset are UTC, date-time forms local.
        if zone.is_empty() && time.is_empty() {
            zone.set(0);
        }
        day.iso = true;
        Token::End
    }

    fn fields(text: &str) -> Option<([i64; 3], [i64; 4], Option<i64>)> {
        let mut scanner = Scanner::new(text);
        let mut zone = Zone {
            sign: NONE,
            hour: NONE,
            minute: NONE,
        };
        let mut time = Time::default();
        let mut day = Day::default();

        let mut token = es5(&mut scanner, &mut day, &mut time, &mut zone);
        if token == Token::Invalid {
            return None;
        }
        let mut has_read_number = day.index > 0;
        while token != Token::End {
            match token {
                Token::Number { value: n, .. } => {
                    has_read_number = true;
                    if scanner.skip_symbol(b':') {
                        if scanner.skip_symbol(b':') {
                            if !time.is_empty() {
                                return None;
                            }
                            time.add(n);
                            time.add(0);
                        } else {
                            if !time.add(n) {
                                return None;
                            }
                            if scanner.peek().is_symbol(b'.') {
                                scanner.next();
                            }
                        }
                    } else if scanner.skip_symbol(b'.') && time.is_expecting(n) {
                        time.add(n);
                        if !scanner.peek().is_number() {
                            return None;
                        }
                        let ms = read_milliseconds(scanner.next());
                        if ms < 0 {
                            return None;
                        }
                        time.add_final(ms);
                    } else if zone.is_expecting(n) {
                        zone.minute = n;
                    } else if time.is_expecting(n) {
                        time.add_final(n);
                        let peek = scanner.peek();
                        if peek != Token::End
                            && peek != Token::WhiteSpace
                            && !peek.is_keyword_z()
                            && !peek.is_sign()
                        {
                            return None;
                        }
                    } else {
                        if !day.add(n) {
                            return None;
                        }
                        scanner.skip_symbol(b'-');
                    }
                }
                Token::Keyword { kind, value, .. } => {
                    if kind == AM_PM && !time.is_empty() {
                        time.hour_offset = Some(value);
                    } else if kind == MONTH_NAME {
                        day.named_month = Some(value);
                        scanner.skip_symbol(b'-');
                    } else if kind == TIME_ZONE_NAME && has_read_number {
                        zone.set(value);
                    } else {
                        if has_read_number || scanner.peek().is_number() {
                            return None;
                        }
                    }
                }
                _ if token.is_sign() && (zone.is_utc() || !time.is_empty()) => {
                    zone.sign = token.sign();
                    let mut n = 0;
                    let mut length = 0;
                    if let Token::Number { value, length: l } = scanner.peek() {
                        scanner.next();
                        n = value;
                        length = l;
                    }
                    has_read_number = true;
                    if scanner.peek().is_symbol(b':') {
                        zone.hour = n;
                        zone.minute = NONE;
                    } else if length == 1 || length == 2 {
                        zone.hour = n;
                        zone.minute = 0;
                    } else if length == 3 || length == 4 {
                        zone.hour = n / 100;
                        zone.minute = n % 100;
                    } else {
                        return None;
                    }
                }
                _ if (token.is_sign() || token.is_symbol(b')')) && has_read_number => {
                    return None;
                }
                _ => {}
            }
            token = scanner.next();
        }

        let day = day.write()?;
        let time = time.write()?;
        let offset = zone.write().ok()?;
        Some((day.into(), time, offset))
    }

    pub(super) fn parse(text: &str, local: &LocalZone) -> f64 {
        let Some(([year, month, day], [h, m, s, ms], offset)) = fields(text) else {
            return f64::NAN;
        };
        let date = make_date(
            make_day(year as f64, month as f64, day as f64),
            make_time(h as f64, m as f64, s as f64, ms as f64),
        );
        match offset {
            None => {
                if date.abs() > MAX_TIME + 864_000_000.0 {
                    return f64::NAN;
                }
                time_clip(local.utc(date))
            }
            Some(offset) => {
                let date = date - offset as f64 * 1000.0;
                if date.abs() > MAX_TIME {
                    return f64::NAN;
                }
                time_clip(date)
            }
        }
    }
}
