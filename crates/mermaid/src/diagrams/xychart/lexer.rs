//! Pele's `diagrams/xychart/lexer.ts`.

use std::sync::LazyLock;

use super::tokens::T;
use crate::{
    diagrams::common::chart::{ChartRules, ChartState},
    token::{Cursor, Token},
    util::chars::{digits_end, is_space},
};

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    AxisData,
    AxisBandData,
    Data,
    DataInner,
    String,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

impl S {
    /// The token the state gives for the empty rest of the input.
    fn at_end(self) -> T {
        match self {
            Self::String => T::Str,
            Self::AccTitle => T::AccTitleValue,
            Self::AccDescr => T::AccDescrValue,
            Self::AccDescrMultiline => T::AccDescrMultilineValue,
            _ => T::Eof,
        }
    }
}

impl ChartState for S {
    const ACC_TITLE: Self = Self::AccTitle;
    const ACC_DESCR: Self = Self::AccDescr;
    const ACC_DESCR_MULTILINE: Self = Self::AccDescrMultiline;
    const STRING: Self = Self::String;
}

/// Mermaid's grammar means to open a markdown string at `"` followed by a
/// backtick, but its quoting turns the rule into one that matches this text
/// and leaves the current state. Markdown strings are never read.
static BROKEN_RULE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "`){}{{ this.pushstate(md_string); }}\n<md_string>(?:(?!`\").)+{}{{ return md_str; }}\n<md_string>(?:`",
        " ".repeat(36),
        " ".repeat(18)
    )
});

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
}

impl Lexer<'_> {
    fn digit_at(&self, at: usize) -> bool {
        self.cur.char_at(at).is_some_and(|c| c.is_ascii_digit())
    }

    /// The end of a number with an optional sign and decimals at `p`.
    fn number_end(&self, p: usize, c: char) -> Option<usize> {
        let src = self.cur.src;
        let q = if matches!(c, '+' | '-') { p + 1 } else { p };
        if self.digit_at(q) {
            let end = digits_end(src, q);
            if self.cur.char_at(end) == Some('.') && self.digit_at(end + 1) {
                return Some(digits_end(src, end + 1));
            }
            return Some(end);
        }
        (self.cur.char_at(q) == Some('.') && self.digit_at(q + 1)).then(|| digits_end(src, q + 1))
    }

    /// A `kind` token of `word` at the cursor, entering `state` if any.
    fn keyword(&mut self, word: &str, bounded: bool, kind: T, state: Option<S>) -> bool {
        let p = self.cur.p;
        let found = if bounded {
            self.cur.keyword_caseless(word, p)
        } else {
            self.cur.starts_with_caseless(word, p)
        };
        if found {
            self.cur.emit(kind, p, p + word.len());
            if let Some(state) = state {
                self.cur.push(state);
            }
        }
        found
    }

    /// Reads one token or skips one piece of text; false where no rule
    /// matches.
    fn step(&mut self, c: char) -> bool {
        if self.cur.value() || self.cur.comment() {
            return true;
        }
        let p = self.cur.p;
        let state = self.cur.state;
        if matches!(c, '\n' | '\r') {
            // Only a line break that directly follows an axis or a series ends
            // it; one after spaces is skipped with them, and the state carries
            // on to the next line.
            if matches!(state, S::AxisData | S::Data)
                && (c == '\n' || self.cur.char_at(p + 1) == Some('\n'))
            {
                let end = if c == '\n' { p + 1 } else { p + 2 };
                self.cur.emit(T::Newline, p, end);
                self.cur.pop();
                return true;
            }
            return self.cur.newlines();
        }

        let keyword = match c.to_ascii_lowercase() {
            't' => self.keyword("title", true, T::Title, None),
            'a' => self.cur.accessibility(),
            'x' => {
                self.keyword("xychart-beta", true, T::Xychart, None)
                    || self.keyword("xychart", true, T::Xychart, None)
                    || self.keyword("x-axis", true, T::XAxis, Some(S::AxisData))
            }
            'y' => self.keyword("y-axis", true, T::YAxis, Some(S::AxisData)),
            'v' => self.keyword("vertical", false, T::ChartOrientation, None),
            'h' => self.keyword("horizontal", false, T::ChartOrientation, None),
            'l' => self.keyword("line", true, T::Line, Some(S::Data)),
            'b' => self.keyword("bar", true, T::Bar, Some(S::Data)),
            _ => false,
        };
        if keyword {
            return true;
        }

        if c == '[' {
            match state {
                S::AxisData => self.cur.push(S::AxisBandData),
                S::Data => self.cur.push(S::DataInner),
                _ => {}
            }
            self.cur.emit(T::SquareBracesStart, p, p + 1);
            return true;
        }
        if c == ']' {
            if matches!(state, S::DataInner | S::AxisBandData) {
                self.cur.pop();
            }
            self.cur.emit(T::SquareBracesEnd, p, p + 1);
            return true;
        }
        if state == S::AxisData && self.cur.starts_with("-->", p) {
            self.cur.emit(T::ArrowDelimiter, p, p + 3);
            return true;
        }
        if matches!(state, S::AxisData | S::DataInner)
            && let Some(end) = self.number_end(p, c)
        {
            self.cur.emit(T::NumberWithDecimal, p, end);
            return true;
        }
        if c == '`' && self.cur.starts_with_caseless(&BROKEN_RULE, p) {
            self.cur.p += BROKEN_RULE.len();
            self.cur.pop();
            return true;
        }
        if c == '"' {
            self.cur.p += 1;
            self.cur.push(S::String);
            return true;
        }
        if is_space(c) {
            self.cur.p = self.cur.space_end(p);
            return true;
        }
        self.cur.word(c)
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let mut lexer = Lexer {
        cur: Cursor::new(src, S::Initial),
    };
    while let Some(c) = lexer.cur.char_at(lexer.cur.p) {
        if !lexer.step(c) {
            break;
        }
    }
    let mut cur = lexer.cur;
    let n = src.len();
    if cur.p < n {
        cur.emit(T::Error, cur.p, cur.p);
        return cur.tokens;
    }
    // Jison tries the rules of the current state once more against the empty
    // rest of the input.
    cur.emit(cur.state.at_end(), n, n);
    cur.emit(T::End, n, n);
    cur.tokens
}
