//! Pele's `diagrams/gantt/lexer.ts`.

use super::tokens::T;
use crate::{
    token::{Cursor, Token},
    util::chars::{is_space, is_word},
};

/// What ends most statements' text.
const STATEMENT_END: &[char] = &['#', '\n', ';'];

/// Statements that start with a keyword, written in lower case, in the
/// grammar's rule order: the keyword alone, or the keyword, one whitespace
/// character and text up to the given characters.
const KEYWORDS: [(&str, Option<&[char]>, T); 12] = [
    ("axisformat", Some(STATEMENT_END), T::AxisFormat),
    ("accdescription", Some(STATEMENT_END), T::AccDescription),
    ("gantt", None, T::Gantt),
    ("dateformat", Some(STATEMENT_END), T::DateFormat),
    ("inclusiveenddates", None, T::InclusiveEndDates),
    ("includes", Some(STATEMENT_END), T::Includes),
    ("topaxis", None, T::TopAxis),
    ("tickinterval", Some(STATEMENT_END), T::TickInterval),
    ("todaymarker", Some(&['\n', ';']), T::TodayMarker),
    ("title", Some(&['\n']), T::Title),
    ("excludes", Some(STATEMENT_END), T::Excludes),
    ("section", Some(&['\n']), T::Section),
];

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Href,
    Callbackname,
    Callbackargs,
    Click,
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
}

impl Lexer<'_> {
    /// The end of `word`, in any case, at the cursor and the whitespace
    /// after it.
    fn word_spaces(&self, word: &str) -> Option<usize> {
        let p = self.cur.p;
        self.cur
            .starts_with_caseless(word, p)
            .then(|| self.cur.space_end(p + word.len()))
    }

    /// The end of `word` and one or more whitespace characters after it.
    fn word_space(&self, word: &str) -> Option<usize> {
        self.word_spaces(word)
            .filter(|&end| end > self.cur.p + word.len())
    }

    /// A token from the cursor to `end`.
    fn emit(&mut self, kind: T, end: usize) -> bool {
        self.cur.emit_to(kind, end);
        true
    }

    /// Enters `state` at `end`.
    fn begin(&mut self, state: S, end: usize) -> bool {
        self.cur.state = state;
        self.cur.p = end;
        true
    }

    /// `\d\d\d\d-\d\d-\d\d\b` at the cursor.
    fn date(&self) -> Option<usize> {
        let p = self.cur.p;
        let shape = self.cur.src.as_bytes().get(p..p + 10)?;
        let fits = shape.iter().enumerate().all(|(k, b)| match k {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        });
        (fits && !is_word(self.cur.code(p + 10))).then_some(p + 10)
    }

    /// A statement that starts with a keyword, at the cursor.
    fn keyword(&mut self) -> bool {
        let p = self.cur.p;
        if let Some(q) = self.word_spaces("acctitle")
            && self.cur.char_at(q) == Some(':')
        {
            self.cur.state = S::AccTitle;
            let end = self.cur.space_end(q + 1);
            return self.emit(T::AccTitle, end);
        }
        if let Some(q) = self.word_spaces("accdescr") {
            match self.cur.char_at(q) {
                Some(':') => {
                    self.cur.state = S::AccDescr;
                    let end = self.cur.space_end(q + 1);
                    return self.emit(T::AccDescr, end);
                }
                Some('{') => {
                    let end = self.cur.space_end(q + 1);
                    return self.begin(S::AccDescrMultiline, end);
                }
                _ => {}
            }
        }
        if let Some(q) = self.word_space("href")
            && self.cur.char_at(q) == Some('"')
        {
            return self.begin(S::Href, q + 1);
        }
        if let Some(q) = self.word_space("call") {
            return self.begin(S::Callbackname, q);
        }
        if let Some(q) = self.word_space("click") {
            return self.begin(S::Click, q);
        }
        if let Some(q) = self.word_space("weekday")
            && let Some(&(kind, day)) = T::WEEKDAYS
                .iter()
                .find(|(_, day)| self.cur.keyword_caseless(day.name(), q))
        {
            return self.emit(kind, q + day.name().len());
        }
        if let Some(q) = self.word_space("weekend") {
            for (day, kind) in [
                ("friday", T::WeekendFriday),
                ("saturday", T::WeekendSaturday),
            ] {
                if self.cur.keyword_caseless(day, q) {
                    return self.emit(kind, q + day.len());
                }
            }
        }
        for (word, stops, kind) in KEYWORDS {
            let end = match stops {
                Some(stops) => self.cur.keyword_line(word, stops),
                None => self.cur.keyword_caseless(word, p).then_some(p + word.len()),
            };
            if let Some(end) = end {
                return self.emit(kind, end);
            }
        }
        false
    }

    /// Reads at the cursor in the initial state; `false` ends the tokens.
    fn initial(&mut self, c: char) -> bool {
        let p = self.cur.p;
        let w = c.len_utf8();
        // The UTF-16 unit after an astral character is the second half of
        // its pair, which none of these rules match.
        let d = if c.len_utf16() == 2 {
            None
        } else {
            self.cur.char_at(p + w)
        };
        if c == '%' {
            if d != Some('%') {
                let end = self.cur.line_end(p + 1);
                self.cur.p = self.cur.run_end(end, |c| c == '\n');
            } else if self.cur.char_at(p + 2) == Some('{') {
                self.emit(T::OpenDirective, p + 3);
                self.cur.emit(T::Error, p, p);
                return false;
            } else {
                self.cur.p = self.cur.line_end(p + 2);
            }
        } else if c != '}' && d == Some('%') {
            // Any character but `}` followed by `%` starts a comment that
            // runs to the end of the line.
            self.cur.p = self.cur.line_end(p + w + 1);
        } else if c == '\n' {
            let end = self.cur.run_end(p, |c| c == '\n');
            self.emit(T::Nl, end);
        } else if is_space(c) {
            self.cur.p = self.cur.space_end(p + w);
        } else if !self.keyword() {
            self.task(c);
        }
        true
    }

    /// A date, a task's text, or its data after a colon.
    fn task(&mut self, c: char) {
        let p = self.cur.p;
        if let Some(end) = self.date() {
            self.emit(T::Date, end);
        } else if c != ':' {
            let from = p + c.len_utf8();
            let end = self.cur.until(from, |c| matches!(c, ':' | '\n'));
            self.emit(T::TaskTxt, end);
        } else {
            let end = self.cur.until(p + 1, |c| STATEMENT_END.contains(&c));
            self.emit(if end > p + 1 { T::TaskData } else { T::Colon }, end);
        }
    }

    /// Reads at the cursor; `false` ends the tokens.
    fn step(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match self.cur.state {
            S::Initial => return self.initial(c),
            S::AccTitle | S::AccDescr => {
                let kind = if self.cur.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                self.cur.state = S::Initial;
                let end = self.cur.line_end(p);
                self.emit(kind, end);
            }
            S::AccDescrMultiline => self.delimited(c, '}', T::AccDescrMultilineValue),
            S::Href => self.delimited(c, '"', T::Href),
            S::Callbackname => {
                if c == '(' {
                    let q = self.cur.space_end(p + 1);
                    if self.cur.char_at(q) == Some(')') {
                        self.begin(S::Initial, q + 1);
                    } else {
                        self.begin(S::Callbackargs, p + 1);
                    }
                } else {
                    let end = self.cur.until(p, |c| c == '(');
                    self.emit(T::Callbackname, end);
                }
            }
            S::Callbackargs => self.delimited(c, ')', T::Callbackargs),
            S::Click => {
                if is_space(c) {
                    self.begin(S::Initial, p + c.len_utf8());
                } else {
                    let end = self.cur.run_end(p + c.len_utf8(), |c| !is_space(c));
                    self.emit(T::Click, end);
                }
            }
        }
        true
    }

    /// Text up to `close`, which returns to the initial state.
    fn delimited(&mut self, c: char, close: char, kind: T) {
        let p = self.cur.p;
        if c == close {
            self.begin(S::Initial, p + 1);
        } else {
            let end = self.cur.until(p, |c| c == close);
            self.emit(kind, end);
        }
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lexer = Lexer {
        cur: Cursor::new(src, S::Initial),
    };
    while let Some(c) = lexer.cur.char_at(lexer.cur.p) {
        if !lexer.step(c) {
            return lexer.cur.tokens;
        }
    }
    let kind = match lexer.cur.state {
        S::Initial => T::Eof,
        S::AccTitle => T::AccTitleValue,
        S::AccDescr => T::AccDescrValue,
        S::AccDescrMultiline => T::AccDescrMultilineValue,
        S::Href => T::Href,
        S::Callbackname => T::Callbackname,
        S::Callbackargs => T::Callbackargs,
        S::Click => T::Click,
    };
    lexer.cur.emit(kind, n, n);
    lexer.cur.emit(T::End, n, n);
    lexer.cur.tokens
}
