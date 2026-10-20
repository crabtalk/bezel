//! The parts of dayjs 1.11 that Mermaid's gantt code relies on, with the
//! plugins it loads (custom parse format, advanced format, ISO week) and the
//! English locale: strict parsing against a format, formatting, and
//! calendar arithmetic, all in local time. Quirks are kept, because a date
//! is accepted only when formatting the parsed result gives back the input.
//! Pele's `diagrams/gantt/dates.ts`.

use std::{cell::RefCell, rc::Rc};

use crate::js::{self, Lazy, date::Date};

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
pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// A day of the week, numbered from Sunday as [`Date::day`] numbers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl Weekday {
    pub const ALL: [Weekday; 7] = [
        Weekday::Sunday,
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
    ];

    /// The day's name in lower case.
    pub fn name(self) -> &'static str {
        match self {
            Weekday::Sunday => "sunday",
            Weekday::Monday => "monday",
            Weekday::Tuesday => "tuesday",
            Weekday::Wednesday => "wednesday",
            Weekday::Thursday => "thursday",
            Weekday::Friday => "friday",
            Weekday::Saturday => "saturday",
        }
    }

    /// The day `name` names, in lower case.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|day| day.name() == name)
    }
}

const DAY: f64 = 86_400_000.0;
const WEEK: f64 = 7.0 * DAY;
const DEFAULT_FORMAT: &str = "YYYY-MM-DDTHH:mm:ssZ";

static RE_ADVANCED: Lazy = Lazy::new(
    r"\[([^\]]+)]|Q|wo|ww|w|WW|W|zzz|z|gggg|GGGG|Do|X|x|k{1,2}|S",
    "g",
);
static RE_FORMAT: Lazy = Lazy::new(
    r"\[([^\]]+)]|YYYY|YY|M{1,4}|D{1,2}|d{1,4}|H{1,2}|h{1,2}|a|A|m{1,2}|s{1,2}|Z{1,2}|SSS",
    "g",
);
static RE_TOKENS: Lazy = Lazy::new(
    r"(\[[^[]*\])|([-_:/.,()\s]+)|(A|a|Q|YYYY|YY?|ww?|MM?M?M?|Do|DD?|hh?|HH?|mm?|ss?|S{1,3}|z|ZZ?)",
    "g",
);
static RE_LITERAL: Lazy = Lazy::new(r"\[[^\]]+]", "g");
static RE_LOCALIZED: Lazy = Lazy::new(r"[Ll]", "");
static RE_BRACKETS: Lazy = Lazy::new(r"^\[|\]$", "g");
static MATCH_1: Lazy = Lazy::new(r"\d", "");
static MATCH_2: Lazy = Lazy::new(r"\d\d", "");
static MATCH_3: Lazy = Lazy::new(r"\d{3}", "");
static MATCH_4: Lazy = Lazy::new(r"\d{4}", "");
static MATCH_1_TO_2: Lazy = Lazy::new(r"\d\d?", "");
static MATCH_SIGNED: Lazy = Lazy::new(r"[+-]?\d+", "");
static MATCH_OFFSET: Lazy = Lazy::new(r"[+-]\d\d:?(\d\d)?|Z", "");
static MATCH_WORD: Lazy = Lazy::new(r"\d*[^-_:/,()\s\d]+", "");
static RE_OFFSET_PARTS: Lazy = Lazy::new(r"([+-]|\d\d)", "g");
/// How many formats [`compiled`] keeps.
const COMPILED_KEPT: usize = 4;

thread_local! {
    /// The formats compiled last, newest at the end.
    static COMPILED: RefCell<Vec<(String, Compiled)>> =
        const { RefCell::new(Vec::new()) };
}
static RE_DIGITS: Lazy = Lazy::new(r"\d+", "");

/// A unit `add` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Ms,
    S,
    M,
    H,
    D,
    W,
    Month,
    Y,
}

impl Unit {
    pub fn parse(text: &str) -> Unit {
        match text {
            "s" => Unit::S,
            "m" => Unit::M,
            "h" => Unit::H,
            "d" => Unit::D,
            "w" => Unit::W,
            "M" => Unit::Month,
            "y" => Unit::Y,
            _ => Unit::Ms,
        }
    }
}

fn pad(value: f64, length: usize) -> String {
    let s = js::num_str(value);
    if s.len() >= length {
        s
    } else {
        "0".repeat(length - s.len()) + &s
    }
}

fn ordinal(n: f64) -> String {
    let s = ["th", "st", "nd", "rd"];
    let v = n % 100.0;
    let pick = |i: f64| -> Option<&str> {
        ((0.0..4.0).contains(&i) && i.fract() == 0.0).then(|| s[i as usize])
    };
    let suffix = pick((v - 20.0) % 10.0).or_else(|| pick(v)).unwrap_or(s[0]);
    format!("{}{suffix}", js::num_str(n))
}

fn utc_offset(date: Date) -> f64 {
    let offset = -js::round(date.timezone_offset() / 15.0) * 15.0;
    if offset == 0.0 { 0.0 } else { offset }
}

fn zone(date: Date) -> String {
    let offset = utc_offset(date);
    let minutes = offset.abs();
    format!(
        "{}{}:{}",
        if offset >= 0.0 { "+" } else { "-" },
        pad((minutes / 60.0).floor(), 2),
        pad(minutes % 60.0, 2)
    )
}

pub(crate) fn iso_weekday(date: Date) -> f64 {
    let day = date.day();
    if js::truthy(day) { day } else { 7.0 }
}

pub(crate) fn add_days(date: Date, days: f64) -> Date {
    let mut next = date;
    next.set_date(next.date() + days);
    next
}

fn days_in_month(date: Date) -> f64 {
    let mut last = date.zone().local(
        date.full_year(),
        date.month() + 1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    );
    last.set_hours(23.0, 59.0, 59.0, 999.0);
    last.date()
}

/// Sets the month or the year, keeping the day of the month unless the new
/// month is shorter.
fn set_calendar(date: Date, apply: impl FnOnce(&mut Date)) -> Date {
    let mut next = date;
    next.set_date(1.0);
    apply(&mut next);
    next.set_date(date.date().min(days_in_month(next)));
    next
}

pub(crate) fn add(date: Date, amount: f64, unit: Unit) -> Date {
    match unit {
        Unit::Month => set_calendar(date, |d| d.set_month(date.month() + amount)),
        Unit::Y => set_calendar(date, |d| d.set_full_year(date.full_year() + amount)),
        Unit::D => add_days(date, js::round(amount)),
        Unit::W => add_days(date, js::round(7.0 * amount)),
        Unit::H => date.at(date.time() + amount * 3_600_000.0),
        Unit::M => date.at(date.time() + amount * 60_000.0),
        Unit::S => date.at(date.time() + amount * 1000.0),
        Unit::Ms => date.at(date.time() + amount),
    }
}

fn week_thursday(date: Date) -> Date {
    add_days(date, 4.0 - iso_weekday(date))
}

fn iso_week(date: Date) -> f64 {
    let thursday = week_thursday(date);
    let jan1 = thursday
        .zone()
        .local(thursday.full_year(), 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
    let weekday = iso_weekday(jan1);
    let first = add_days(jan1, 4.0 - weekday + if weekday > 4.0 { 7.0 } else { 0.0 });
    let zone_delta = (utc_offset(first) - utc_offset(thursday)) * 60_000.0;
    let weeks = (thursday.time() - first.time() - zone_delta) / WEEK;
    (if weeks < 0.0 {
        let up = weeks.ceil();
        if js::truthy(up) { up } else { 0.0 }
    } else {
        weeks.floor()
    }) + 1.0
}

/// `None` for the week-of-year and time zone name tokens, which need dayjs
/// plugins Mermaid does not load and so fail there.
pub(crate) fn format(date: Date, template: &str) -> Option<String> {
    if !date.is_valid() {
        return Some("Invalid Date".into());
    }
    let s = if template.is_empty() {
        DEFAULT_FORMAT
    } else {
        template
    };
    if s == "YYYY-MM-DD" {
        return Some(iso_date(date));
    }
    let mut unsupported = false;
    let advanced = RE_ADVANCED.replace(s, |caps| {
        let m = caps.str(0);
        match m {
            "Q" => js::num_str(((date.month() + 1.0) / 3.0).ceil()),
            "Do" => format!("[{}]", ordinal(date.date())),
            "GGGG" => js::num_str(week_thursday(date).full_year()),
            "W" => js::num_str(iso_week(date)),
            "WW" => pad(iso_week(date), 2),
            "k" => js::num_str(if js::truthy(date.hours()) {
                date.hours()
            } else {
                24.0
            }),
            "kk" => pad(
                if js::truthy(date.hours()) {
                    date.hours()
                } else {
                    24.0
                },
                2,
            ),
            "X" => js::num_str((date.time() / 1000.0).floor()),
            "x" => js::num_str(date.time()),
            "gggg" | "wo" | "w" | "ww" | "z" | "zzz" => {
                unsupported = true;
                m.to_string()
            }
            _ => m.to_string(),
        }
    });
    if unsupported {
        return None;
    }
    let hours = date.hours();
    Some(RE_FORMAT.replace(&advanced, |caps| {
        if let Some(literal) = caps.get(1).filter(|literal| !literal.is_empty()) {
            return literal.to_string();
        }
        let twelve = if js::truthy(hours % 12.0) {
            hours % 12.0
        } else {
            12.0
        };
        match caps.str(0) {
            "YY" => {
                let year = js::num_str(date.full_year());
                year[year.len().saturating_sub(2)..].to_string()
            }
            "YYYY" => pad(date.full_year(), 4),
            "M" => js::num_str(date.month() + 1.0),
            "MM" => pad(date.month() + 1.0, 2),
            "MMM" => MONTHS[date.month() as usize][..3].to_string(),
            "MMMM" => MONTHS[date.month() as usize].to_string(),
            "D" => js::num_str(date.date()),
            "DD" => pad(date.date(), 2),
            "d" => js::num_str(date.day()),
            "dd" => WEEKDAYS[date.day() as usize][..2].to_string(),
            "ddd" => WEEKDAYS[date.day() as usize][..3].to_string(),
            "dddd" => WEEKDAYS[date.day() as usize].to_string(),
            "H" => js::num_str(hours),
            "HH" => pad(hours, 2),
            "h" => js::num_str(twelve),
            "hh" => pad(twelve, 2),
            "a" => if hours < 12.0 { "am" } else { "pm" }.into(),
            "A" => if hours < 12.0 { "AM" } else { "PM" }.into(),
            "m" => js::num_str(date.minutes()),
            "mm" => pad(date.minutes(), 2),
            "s" => js::num_str(date.seconds()),
            "ss" => pad(date.seconds(), 2),
            "SSS" => pad(date.milliseconds(), 3),
            "Z" => zone(date),
            _ => zone(date).replacen(':', "", 1),
        }
    }))
}

pub(crate) fn iso_date(date: Date) -> String {
    let year = date.full_year();
    let month = date.month() + 1.0;
    let day = date.date();
    format!(
        "{}{}{}{}{}",
        if (1000.0..=9999.0).contains(&year) {
            js::num_str(year)
        } else {
            pad(year, 4)
        },
        if month < 10.0 { "-0" } else { "-" },
        js::num_str(month),
        if day < 10.0 { "-0" } else { "-" },
        js::num_str(day)
    )
}

#[derive(Default)]
struct Fields {
    year: Option<f64>,
    month: Option<f64>,
    day: Option<f64>,
    day_given: bool,
    hours: Option<f64>,
    minutes: Option<f64>,
    seconds: Option<f64>,
    milliseconds: Option<f64>,
    week: Option<f64>,
    offset: Option<f64>,
    afternoon: Option<bool>,
}

/// What a format token reads, and where it puts it.
#[derive(Clone, Copy)]
enum Read {
    Afternoon(&'static str),
    Quarter,
    Fraction(f64),
    Seconds,
    Minutes,
    Hours,
    Day,
    Ordinal,
    Week,
    Month,
    MonthName(bool),
    Year,
    ShortYear,
    Offset,
}

enum Step {
    Literal(String),
    Read(&'static Lazy, Read),
}

/// A format's steps, `None` for one dayjs fails on.
type Compiled = Rc<Option<Vec<Step>>>;

fn reader(token: &str) -> Option<(&'static Lazy, Read)> {
    Some(match token {
        "A" => (&MATCH_WORD, Read::Afternoon("PM")),
        "a" => (&MATCH_WORD, Read::Afternoon("pm")),
        "Q" => (&MATCH_1, Read::Quarter),
        "S" => (&MATCH_1, Read::Fraction(100.0)),
        "SS" => (&MATCH_2, Read::Fraction(10.0)),
        "SSS" => (&MATCH_3, Read::Fraction(1.0)),
        "s" | "ss" => (&MATCH_1_TO_2, Read::Seconds),
        "m" | "mm" => (&MATCH_1_TO_2, Read::Minutes),
        "H" | "h" | "HH" | "hh" => (&MATCH_1_TO_2, Read::Hours),
        "D" => (&MATCH_1_TO_2, Read::Day),
        "DD" => (&MATCH_2, Read::Day),
        "Do" => (&MATCH_WORD, Read::Ordinal),
        "w" => (&MATCH_1_TO_2, Read::Week),
        "ww" => (&MATCH_2, Read::Week),
        "M" => (&MATCH_1_TO_2, Read::Month),
        "MM" => (&MATCH_2, Read::Month),
        "MMM" => (&MATCH_WORD, Read::MonthName(true)),
        "MMMM" => (&MATCH_WORD, Read::MonthName(false)),
        "Y" => (&MATCH_SIGNED, Read::Year),
        "YY" => (&MATCH_2, Read::ShortYear),
        "YYYY" => (&MATCH_4, Read::Year),
        "Z" | "ZZ" => (&MATCH_OFFSET, Read::Offset),
        _ => return None,
    })
}

fn read(f: &mut Fields, how: Read, input: &str) -> bool {
    let value = js::number(input);
    match how {
        Read::Afternoon(pm) => f.afternoon = Some(input == pm),
        Read::Quarter => f.month = Some((value - 1.0) * 3.0 + 1.0),
        Read::Fraction(scale) => f.milliseconds = Some(value * scale),
        Read::Seconds => f.seconds = Some(value),
        Read::Minutes => f.minutes = Some(value),
        Read::Hours => f.hours = Some(value),
        Read::Day => f.day = Some(value),
        Read::Week => f.week = Some(value),
        Read::Month => f.month = Some(value),
        Read::Year => f.year = Some(value),
        Read::ShortYear => {
            f.year = Some(value + if value > 68.0 { 1900.0 } else { 2000.0 });
        }
        Read::Ordinal => {
            let Some(digits) = RE_DIGITS.exec(input) else {
                return false;
            };
            // dayjs keeps the digits as a string here, so even "0" counts as
            // a day having been given.
            f.day = Some(js::number(digits.str(0)));
            f.day_given = true;
            for i in 1..=31 {
                if ordinal(i as f64) == input {
                    f.day = Some(i as f64);
                }
            }
        }
        Read::MonthName(short) => {
            let index = MONTHS
                .iter()
                .position(|name| {
                    if short {
                        &name[..3] == input
                    } else {
                        *name == input
                    }
                })
                .map_or(0, |i| i + 1);
            if index < 1 {
                return false;
            }
            f.month = Some(if index % 12 != 0 { index % 12 } else { index } as f64);
        }
        Read::Offset => {
            if input == "Z" {
                f.offset = Some(0.0);
                return true;
            }
            let parts = RE_OFFSET_PARTS.find_all(input);
            let part = |i: usize| parts.get(i).map_or(f64::NAN, |p| js::number(p));
            let second = part(2);
            let minutes = part(1) * 60.0 + if js::truthy(second) { second } else { 0.0 };
            f.offset = Some(if minutes == 0.0 {
                0.0
            } else if parts[0] == "+" {
                -minutes
            } else {
                minutes
            });
        }
    }
    true
}

/// [`compile`], kept for the formats used last.
fn compiled(template: &str) -> Compiled {
    COMPILED.with(|kept| {
        let mut kept = kept.borrow_mut();
        if let Some((_, steps)) = kept.iter().find(|(text, _)| text == template) {
            return steps.clone();
        }
        let steps = Rc::new(compile(template));
        if kept.len() == COMPILED_KEPT {
            kept.remove(0);
        }
        kept.push((template.to_string(), steps.clone()));
        steps
    })
}

fn compile(template: &str) -> Option<Vec<Step>> {
    // dayjs expands localized tokens through a locale table the English
    // locale does not have, and fails.
    if RE_LOCALIZED.test(&RE_LITERAL.replace(template, |_| String::new())) {
        return None;
    }
    let tokens = RE_TOKENS.find_all(template);
    if tokens.is_empty() {
        return None;
    }
    Some(
        tokens
            .into_iter()
            .map(|token| match reader(token) {
                Some((re, how)) => Step::Read(re, how),
                None => Step::Literal(RE_BRACKETS.replace(token, |_| String::new())),
            })
            .collect(),
    )
}

fn loose<'z>(input: &str, template: &str, now: Date<'z>) -> Option<Date<'z>> {
    if template == "x" || template == "X" {
        let scale = if template == "X" { 1000.0 } else { 1.0 };
        return Some(now.at(scale * js::number(input)));
    }
    let steps = compiled(template);
    let steps = steps.as_ref().as_ref()?;
    let mut f = Fields::default();
    let mut rest = input.to_string();
    let mut start = 0;
    for step in steps {
        let (re, how) = match step {
            Step::Literal(text) => {
                start += js::len(text);
                continue;
            }
            Step::Read(re, how) => (re, *how),
        };
        let found = re
            .exec(&rest[js::units_end(&rest, start)..])?
            .str(0)
            .to_string();
        if !read(&mut f, how, &found) {
            return None;
        }
        // dayjs drops the first occurrence of the matched text, wherever it
        // is, and does not move `start`.
        rest = rest.replacen(&found, "", 1);
    }
    if let (Some(afternoon), Some(hours)) = (f.afternoon, f.hours) {
        if afternoon {
            if hours < 12.0 {
                f.hours = Some(hours + 12.0);
            }
        } else if hours == 12.0 {
            f.hours = Some(0.0);
        }
    }
    let truthy = |x: Option<f64>| x.is_some_and(js::truthy);
    let day = if f.day_given || truthy(f.day) {
        f.day.unwrap_or(f64::NAN)
    } else if !truthy(f.year) && !truthy(f.month) {
        now.date()
    } else {
        1.0
    };
    let year = if truthy(f.year) {
        f.year.unwrap_or(f64::NAN)
    } else {
        now.full_year()
    };
    let mut month = 0.0;
    if !(truthy(f.year) && !truthy(f.month)) {
        month = match f.month {
            Some(m) if m > 0.0 => m - 1.0,
            _ => now.month(),
        };
    }
    let or_zero = |x: Option<f64>| x.filter(|&x| js::truthy(x)).unwrap_or(0.0);
    let hours = or_zero(f.hours);
    let minutes = or_zero(f.minutes);
    let seconds = or_zero(f.seconds);
    let ms = or_zero(f.milliseconds);
    if let Some(offset) = f.offset {
        return Some(now.at(Date::utc(
            year,
            month,
            day,
            hours,
            minutes,
            seconds,
            ms + offset * 60_000.0,
        )));
    }
    // Applying a week number needs a dayjs plugin Mermaid does not load.
    if truthy(f.week) {
        return None;
    }
    Some(
        now.zone()
            .local(year, month, day, hours, minutes, seconds, ms),
    )
}

/// Parses the way `dayjs(input, format, true)` does. `now` supplies the
/// fields the format leaves out.
pub(crate) fn parse_strict<'z>(input: &str, template: &str, now: Date<'z>) -> Option<Date<'z>> {
    let date = loose(input, template, now)?;
    (date.is_valid() && format(date, template).as_deref() == Some(input)).then_some(date)
}
