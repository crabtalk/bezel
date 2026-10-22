//! V8's `DateParser`: an ES5 ISO date string, or what its legacy parser
//! makes of anything else.

use crate::{
    string,
    time::date::{LocalZone, MAX_TIME, NONE, make_date, make_day, make_time, time_clip},
};

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
        !self.at_end() && string::is_space(self.ch())
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
            let found = KEYWORDS
                .iter()
                .find(|(word, kind, _)| *word == prefix && (length <= 3 || *kind == MONTH_NAME));
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
            time_clip(local.utc_of(date))
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
