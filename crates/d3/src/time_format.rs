//! d3-time-format's formatting and parsing with its en-US locale, in a
//! [`LocalZone`]. In [`LocalZone::utc`] they are d3's `utcFormat` and
//! `utcParse`.

use crate::{
    format::number::{self, Number},
    string,
    time::{
        Interval, Unit,
        date::{self, Date, LocalZone},
    },
};

const PERIODS: [&str; 2] = ["AM", "PM"];
/// en-US's `%c`.
const DATE_TIME: &str = "%x, %X";
/// en-US's `%x`.
const DATE: &str = "%-m/%-d/%Y";
/// en-US's `%X`.
const TIME: &str = "%-I:%M:%S %p";

impl LocalZone {
    /// d3's `timeParse(specifier)(text)`: `None` where d3 returns `null`.
    pub fn time_parse(&self, specifier: &str, text: &str) -> Option<Date<'_>> {
        Fields::parse(specifier, text)?.date(self)
    }
}

impl Date<'_> {
    /// d3's `timeFormat(specifier)(date)`, with a specifier such as
    /// `%Y-%m-%d`.
    pub fn format(self, specifier: &str) -> String {
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
            if let Some(c) = c {
                out += &self.directive(c, fill);
            }
        }
        out
    }

    /// vega-format's `timeFormat()` with no specifier: `%Y` on a year's
    /// start, `%B` on a month's, `%b %d` on a Sunday, `%a %d` on any other
    /// day's, then `%I %p`, `%I:%M`, `:%S` and `.%L` on an hour's, a
    /// minute's, a second's start and between seconds.
    pub fn multi_format(self) -> String {
        let time = self.time();
        let after = |unit: Unit| Interval::new(unit, 0.0).floored(self).time() < time;
        let specifier = if after(Unit::Second) {
            ".%L"
        } else if after(Unit::Minute) {
            ":%S"
        } else if after(Unit::Hour) {
            "%I:%M"
        } else if after(Unit::Day) {
            "%I %p"
        } else if after(Unit::Month) {
            match after(Unit::Week) {
                true => "%a %d",
                false => "%b %d",
            }
        } else if after(Unit::Year) {
            "%B"
        } else {
            "%Y"
        };
        self.format(specifier)
    }

    /// The start of the week holding this date, weeks starting on `start`,
    /// 0 for Sunday.
    fn week_floor(self, start: f64) -> Self {
        Interval::new(Unit::Week, start).floored(self)
    }

    /// The first start of a week at or after this date.
    fn week_ceil(self, start: f64) -> Self {
        let mut d = self;
        Interval::new(Unit::Week, start).ceil(&mut d);
        d
    }

    fn year_floor(self) -> Self {
        Interval::new(Unit::Year, 0.0).floored(self)
    }

    /// The Thursday that decides which ISO week and ISO year a date belongs
    /// to.
    fn iso_thursday(self) -> Self {
        let day = self.day();
        if day >= 4.0 || day == 0.0 {
            self.week_floor(4.0)
        } else {
            self.week_ceil(4.0)
        }
    }

    /// Weeks since the last `start` day before the year began, which is
    /// d3's `%U` and `%W`.
    fn week_of_year(self, start: f64) -> f64 {
        let year = self.year_floor();
        Interval::new(Unit::Week, start).count(year.at(year.time() - 1.0), self)
    }

    fn directive(self, c: char, p: &str) -> String {
        match c {
            'a' => named(&date::WEEKDAYS, self.day())
                .get(..3)
                .unwrap_or("")
                .into(),
            'A' => named(&date::WEEKDAYS, self.day()).into(),
            'b' => named(&date::MONTHS, self.month())
                .get(..3)
                .unwrap_or("")
                .into(),
            'B' => named(&date::MONTHS, self.month()).into(),
            'c' => self.format(DATE_TIME),
            'd' | 'e' => pad(self.date(), p, 2),
            'f' => pad(self.milliseconds(), p, 3) + "000",
            'g' => pad(self.iso_thursday().full_year() % 100.0, p, 2),
            'G' => pad(self.iso_thursday().full_year() % 10000.0, p, 4),
            'H' => pad(self.hours(), p, 2),
            'I' => pad(number::or(self.hours() % 12.0, 12.0), p, 2),
            'j' => pad(
                1.0 + Interval::new(Unit::Day, 0.0).count(self.year_floor(), self),
                p,
                3,
            ),
            'L' => pad(self.milliseconds(), p, 3),
            'm' => pad(self.month() + 1.0, p, 2),
            'M' => pad(self.minutes(), p, 2),
            'p' => PERIODS[usize::from(self.hours() >= 12.0)].into(),
            'q' => Number(1.0 + f64::from(number::to_int32(self.month() / 3.0))).to_string(),
            'Q' => Number(self.time()).to_string(),
            's' => Number((self.time() / 1000.0).floor()).to_string(),
            'S' => pad(self.seconds(), p, 2),
            'u' => {
                let day = self.day();
                Number(if day == 0.0 { 7.0 } else { day }).to_string()
            }
            'U' => pad(self.week_of_year(0.0), p, 2),
            'V' => {
                let thursday = self.iso_thursday();
                let year = thursday.year_floor();
                let first = f64::from(u8::from(year.day() == 4.0));
                pad(
                    Interval::new(Unit::Week, 4.0).count(year, thursday) + first,
                    p,
                    2,
                )
            }
            'w' => Number(self.day()).to_string(),
            'W' => pad(self.week_of_year(1.0), p, 2),
            'x' => self.format(DATE),
            'X' => self.format(TIME),
            'y' => pad(self.full_year() % 100.0, p, 2),
            'Y' => pad(self.full_year() % 10000.0, p, 4),
            // `utcFormat` writes `+0000` for an invalid date too.
            'Z' if *self.zone() == LocalZone::utc() => "+0000".into(),
            'Z' => {
                let z = self.timezone_offset();
                let abs = z.abs();
                format!(
                    "{}{}{}",
                    if z > 0.0 { "-" } else { "+" },
                    pad(f64::from(number::to_int32(abs / 60.0)), "0", 2),
                    pad(abs % 60.0, "0", 2)
                )
            }
            '%' => "%".into(),
            _ => c.to_string(),
        }
    }
}

/// `names[i]`, or `""` for an index of `NaN`, as JavaScript writes
/// `undefined` in a join.
fn named(names: &[&'static str], i: f64) -> &'static str {
    if i.is_nan() { "" } else { names[i as usize] }
}

fn pad(value: f64, fill: &str, width: usize) -> String {
    let negative = value < 0.0;
    let digits = Number(if negative { -value } else { value }).to_string();
    let sign = if negative { "-" } else { "" };
    let units = string::len(&digits);
    if units < width {
        format!("{sign}{}{digits}", fill.repeat(width - units))
    } else {
        format!("{sign}{digits}")
    }
}

/// The fields d3's parser reads, its `d`: year, month from 0, day of the
/// month, hours, minutes, seconds, milliseconds, and the directives' other
/// fields where the text gave them.
#[derive(Clone, Copy, Debug)]
struct Fields {
    y: f64,
    m: Option<f64>,
    d: f64,
    h: f64,
    min: f64,
    s: f64,
    ms: f64,
    /// `%Q`: milliseconds since the epoch.
    timestamp: Option<f64>,
    /// `%s`: seconds since the epoch.
    seconds: Option<f64>,
    /// `%Z`: the offset to add, as `±hhmm`.
    zone: Option<f64>,
    /// `%p`: 0 for AM, 1 for PM.
    period: Option<f64>,
    /// `%q`: the quarter's first month, from 0.
    quarter: Option<f64>,
    /// `%V`.
    iso_week: Option<f64>,
    /// `%a`, `%A`, `%w`: the day of the week, 0 for Sunday.
    weekday: Option<f64>,
    /// `%u`: the day of the week, 1 for Monday.
    monday_weekday: Option<f64>,
    /// `%W`.
    monday_week: Option<f64>,
    /// `%U`.
    sunday_week: Option<f64>,
}

/// A JavaScript string as UTF-16 units.
type Units = [u16];

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// `/^\s*\d+/` on the first `limit` units of `text` from `at`: the number,
/// and where the match ends.
fn number_at(text: &Units, at: usize, limit: Option<usize>) -> Option<(f64, usize)> {
    let end = limit.map_or(text.len(), |n| text.len().min(at + n));
    let window = text.get(at..end)?;
    let spaces = window.iter().take_while(|&&u| string::is_space(u)).count();
    let digits = window[spaces..]
        .iter()
        .take_while(|&&u| (u16::from(b'0')..=u16::from(b'9')).contains(&u))
        .count();
    if digits == 0 {
        return None;
    }
    let text = String::from_utf16_lossy(&window[spaces..spaces + digits]);
    Some((text.parse().ok()?, at + spaces + digits))
}

/// The first of `names` at `at` in `text`, ignoring ASCII case: its index,
/// and where it ends.
fn name_at(text: &Units, at: usize, names: &[&str]) -> Option<(f64, usize)> {
    let rest = text.get(at..)?;
    names.iter().enumerate().find_map(|(i, name)| {
        let name = units(name);
        let head = rest.get(..name.len())?;
        let same = head.iter().zip(&name).all(|(&a, &b)| {
            let lower = |u: u16| u8::try_from(u).map_or(u, |b| u16::from(b.to_ascii_lowercase()));
            lower(a) == lower(b)
        });
        same.then(|| (i as f64, at + name.len()))
    })
}

/// d3's zone pattern, `/^(Z)|([+-]\d\d)(?::?(\d\d))?/`, on the six units
/// from `at`: the offset to add, and `at` moved by the match's length
/// wherever in those units the match is.
fn zone_at(text: &Units, at: usize) -> Option<(f64, usize)> {
    let window = text.get(at..text.len().min(at + 6))?;
    if window.first() == Some(&u16::from(b'Z')) {
        return Some((0.0, at + 1));
    }
    let digit = |i: usize| {
        window
            .get(i)
            .is_some_and(|&u| (u16::from(b'0')..=u16::from(b'9')).contains(&u))
    };
    (0..window.len()).find_map(|p| {
        let sign = window[p];
        if !(sign == u16::from(b'+') || sign == u16::from(b'-')) || !digit(p + 1) || !digit(p + 2) {
            return None;
        }
        let (minutes, len) =
            if window.get(p + 3) == Some(&u16::from(b':')) && digit(p + 4) && digit(p + 5) {
                (p + 4..p + 6, 6)
            } else if digit(p + 3) && digit(p + 4) {
                (p + 3..p + 5, 5)
            } else {
                (0..0, 3)
            };
        let mut text = String::from_utf16_lossy(&window[p..p + 3]);
        if minutes.is_empty() {
            text += "00";
        } else {
            text += &String::from_utf16_lossy(&window[minutes]);
        }
        Some((-Number::from(text.as_str()).0, at + len))
    })
}

impl Fields {
    fn new() -> Self {
        Self {
            y: 1900.0,
            m: None,
            d: 1.0,
            h: 0.0,
            min: 0.0,
            s: 0.0,
            ms: 0.0,
            timestamp: None,
            seconds: None,
            zone: None,
            period: None,
            quarter: None,
            iso_week: None,
            weekday: None,
            monday_weekday: None,
            monday_week: None,
            sunday_week: None,
        }
    }

    /// The fields `text` gives, all of it read by `specifier`.
    fn parse(specifier: &str, text: &str) -> Option<Self> {
        let mut fields = Self::new();
        let text = units(text);
        let end = fields.read(&units(specifier), &text, 0)?;
        (end == text.len()).then_some(fields)
    }

    /// d3's `parseSpecifier`: reads `text` from `at` by `specifier`, and
    /// returns where it stopped.
    fn read(&mut self, specifier: &Units, text: &Units, mut at: usize) -> Option<usize> {
        let mut i = 0;
        while i < specifier.len() {
            if at >= text.len() {
                return None;
            }
            let c = specifier[i];
            i += 1;
            if c == u16::from(b'%') {
                let mut c = specifier.get(i).copied();
                i += 1;
                if c.is_some_and(|c| b"-_0".iter().any(|&p| c == u16::from(p))) {
                    c = specifier.get(i).copied();
                    i += 1;
                }
                let c = char::from_u32(u32::from(c?))?;
                at = self.directive(c, text, at)?;
            } else if text.get(at) != Some(&c) {
                return None;
            } else {
                at += 1;
            }
        }
        Some(at)
    }

    fn directive(&mut self, c: char, text: &Units, at: usize) -> Option<usize> {
        let short = |names: &[&'static str]| -> Vec<&'static str> {
            names.iter().map(|n| &n[..3]).collect()
        };
        let number = |limit| number_at(text, at, Some(limit));
        let (value, end) = match c {
            'a' => name_at(text, at, &short(&date::WEEKDAYS)),
            'A' => name_at(text, at, &date::WEEKDAYS),
            'b' => name_at(text, at, &short(&date::MONTHS)),
            'B' => name_at(text, at, &date::MONTHS),
            'c' => return self.read(&units(DATE_TIME), text, at),
            'x' => return self.read(&units(DATE), text, at),
            'X' => return self.read(&units(TIME), text, at),
            'p' => name_at(text, at, &PERIODS),
            'q' | 'u' | 'w' => number(1),
            'd' | 'e' | 'g' | 'y' | 'H' | 'I' | 'm' | 'M' | 'S' | 'U' | 'V' | 'W' => number(2),
            'j' | 'L' => number(3),
            'G' | 'Y' => number(4),
            'f' => number(6),
            'Q' | 's' => number_at(text, at, None),
            'Z' => zone_at(text, at),
            '%' => (text.get(at) == Some(&u16::from(b'%'))).then_some((0.0, at + 1)),
            _ => None,
        }?;
        match c {
            'a' | 'A' | 'w' => self.weekday = Some(value),
            'b' | 'B' => self.m = Some(value),
            'p' => self.period = Some(value),
            'q' => self.quarter = Some(value * 3.0 - 3.0),
            'u' => self.monday_weekday = Some(value),
            'd' | 'e' => self.d = value,
            'g' | 'y' => self.y = value + if value > 68.0 { 1900.0 } else { 2000.0 },
            'G' | 'Y' => self.y = value,
            'H' | 'I' => self.h = value,
            'm' => self.m = Some(value - 1.0),
            'M' => self.min = value,
            'S' => self.s = value,
            'U' => self.sunday_week = Some(value),
            'V' => self.iso_week = Some(value),
            'W' => self.monday_week = Some(value),
            'j' => {
                self.m = Some(0.0);
                self.d = value;
            }
            'L' => self.ms = value,
            'f' => self.ms = (value / 1000.0).floor(),
            'Q' => self.timestamp = Some(value),
            's' => self.seconds = Some(value),
            'Z' => self.zone = Some(value),
            _ => {}
        }
        Some(end)
    }

    /// d3's `localDate`: the fields as a date in `zone`, a year from 0 to 99
    /// kept as it is.
    fn calendar<'z>(&self, zone: &'z LocalZone) -> Date<'z> {
        let m = self.m.unwrap_or(f64::NAN);
        let early = (0.0..100.0).contains(&self.y);
        let year = if early { -1.0 } else { self.y };
        let mut date = zone.local(year, m, self.d, self.h, self.min, self.s, self.ms);
        if early {
            date.set_full_year(self.y);
        }
        date
    }

    /// d3's `newParse` after the text is read: the date the fields name.
    /// Fields read with a `%Z` are UTC.
    fn date(mut self, zone: &LocalZone) -> Option<Date<'_>> {
        if let Some(q) = self.timestamp {
            return Some(zone.date(q));
        }
        if let Some(s) = self.seconds {
            return Some(zone.date(s * 1000.0 + self.ms));
        }
        if let Some(p) = self.period {
            self.h = self.h % 12.0 + p * 12.0;
        }
        if self.m.is_none() {
            self.m = Some(self.quarter.unwrap_or(0.0));
        }
        let utc = LocalZone::utc();
        let fields_zone = if self.zone.is_some() { &utc } else { zone };
        let january = |y: f64| {
            Self {
                y,
                m: Some(0.0),
                d: 1.0,
                h: 0.0,
                min: 0.0,
                s: 0.0,
                ms: 0.0,
                ..Self::new()
            }
            .calendar(fields_zone)
        };
        if let Some(v) = self.iso_week {
            if !(1.0..=53.0).contains(&v) {
                return None;
            }
            let w = *self.weekday.get_or_insert(1.0);
            let first = january(self.y);
            let day = first.day();
            let mut week = if day > 4.0 || day == 0.0 {
                first.week_ceil(1.0)
            } else {
                first.week_floor(1.0)
            };
            Interval::new(Unit::Day, 0.0).offset(&mut week, ((v - 1.0) * 7.0).floor());
            self.y = week.full_year();
            self.m = Some(week.month());
            self.d = week.date() + (w + 6.0) % 7.0;
        } else if self.monday_week.is_some() || self.sunday_week.is_some() {
            let w = match (self.weekday, self.monday_weekday) {
                (Some(w), _) => w,
                (None, Some(u)) => u % 7.0,
                (None, None) => f64::from(u8::from(self.monday_week.is_some())),
            };
            let day = january(self.y).day();
            self.m = Some(0.0);
            self.d = match (self.monday_week, self.sunday_week) {
                (Some(week), _) => (w + 6.0) % 7.0 + week * 7.0 - (day + 5.0) % 7.0,
                (None, Some(week)) => w + week * 7.0 - (day + 6.0) % 7.0,
                (None, None) => self.d,
            };
        }
        if let Some(z) = self.zone {
            self.h += f64::from(number::to_int32(z / 100.0));
            self.min += z % 100.0;
        }
        Some(zone.date(self.calendar(fields_zone).time()))
    }
}
