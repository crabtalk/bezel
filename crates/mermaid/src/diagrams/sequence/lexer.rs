//! Pele's `diagrams/sequence/lexer.ts`.

use super::tokens::T;
use crate::{
    js,
    token::{Cursor, Token},
    util::chars::{is_space, starts_with_caseless, width_at},
};

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Id,
    Alias,
    Line,
    Config,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

/// A character that may start an actor name.
fn is_actor_start(c: char) -> bool {
    !matches!(
        c,
        '/' | '\\' | '+' | '(' | ')' | '<' | '-' | '>' | ':' | '\n' | ',' | ';'
    )
}

/// A character of an actor name after its start.
fn is_actor_rest(c: char) -> bool {
    !matches!(c, '+' | '<' | '-' | '>' | ':' | '\n' | ',' | ';')
}

/// A character of a participant name that may hold whitespace.
fn is_id_wide(c: char) -> bool {
    !matches!(c, '<' | '>' | ':' | '\n' | ',' | ';' | '@')
}

/// A character of a participant name.
fn is_id(c: char) -> bool {
    is_id_wide(c) && !is_space(c)
}

/// A character of the rest of a line.
fn is_line(c: char) -> bool {
    !matches!(c, '#' | '\n' | ';')
}

/// A character that may start the name after an actor menu keyword.
fn is_menu(c: char) -> bool {
    !matches!(
        c,
        '/' | '\\' | '+' | '(' | ')' | '<' | '>' | ':' | '\n' | ',' | ';' | '-'
    ) && !is_space(c)
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    next_nl: Option<usize>,
}

impl<'s> Lexer<'s> {
    fn is(&self, q: usize, test: fn(char) -> bool) -> bool {
        self.cur.char_at(q).is_some_and(test)
    }

    /// The next line feed at or after `q`, or the end.
    fn eol(&mut self, q: usize) -> usize {
        if self.next_nl.is_none_or(|nl| nl < q) {
            self.next_nl = Some(self.cur.line_end(q));
        }
        self.next_nl.unwrap_or(self.cur.src.len())
    }

    fn skip_same_line(&mut self) -> bool {
        let p = self.cur.p;
        let q = self.cur.src[p..]
            .find(|c: char| c == '\n' || !is_space(c))
            .map_or(self.cur.src.len(), |at| p + at);
        if q == p {
            return false;
        }
        self.cur.p = q;
        true
    }

    fn line_run(&self, mut q: usize) -> usize {
        while self.is(q, is_line) {
            q = self.cur.after(q);
        }
        q
    }

    fn begin(&mut self, kind: T, len: usize, next: S) {
        self.cur.emit_to(kind, self.cur.p + len);
        self.cur.push(next);
    }

    fn keyword(&mut self, lit: &str, kind: T, next: Option<S>) -> bool {
        if !self.cur.keyword_caseless(lit, self.cur.p) {
            return false;
        }
        self.cur.emit_to(kind, self.cur.p + lit.len());
        if let Some(next) = next {
            self.cur.push(next);
        }
        true
    }

    /// The actor menu keywords count only when spaces and the start of a name
    /// follow.
    fn menu(&mut self, lit: &str, kind: T) -> bool {
        let p = self.cur.p;
        if !starts_with_caseless(self.cur.src, p, lit) {
            return false;
        }
        let mut q = p + lit.len();
        if !matches!(self.cur.char_at(q), Some(' ' | '\t')) {
            return false;
        }
        while matches!(self.cur.char_at(q), Some(' ' | '\t')) {
            q += 1;
        }
        if !self.is(q, is_menu) {
            return false;
        }
        self.cur.emit_to(kind, p + lit.len());
        true
    }

    fn arrow_ahead(&self, q: usize) -> bool {
        let at = |k| self.cur.char_at(q + k);
        match at(0) {
            Some('-') => {
                matches!(at(1), Some('x' | 'X' | ')' | '\\' | '/' | '-'))
                    || (at(1) == Some('|') && matches!(at(2), Some('\\' | '/')))
            }
            Some(c @ ('/' | '\\')) => {
                matches!(at(1), Some(d) if d == '|' || d == c) && at(2) == Some('-')
            }
            Some('(') => at(1) == Some(')'),
            _ => false,
        }
    }

    fn actor(&mut self) {
        let p = self.cur.p;
        let n = self.cur.src.len();
        let mut q = self.cur.after(p);
        while self.is(q, is_actor_start) {
            q = self.cur.after(q);
        }
        while q < n && !self.arrow_ahead(q) {
            let mut r = q;
            while self.cur.char_at(r) == Some('-') {
                r += 1;
            }
            if !self.is(r, is_actor_rest) {
                break;
            }
            r = self.cur.after(r);
            while self.is(r, is_actor_rest) {
                r = self.cur.after(r);
            }
            q = r;
        }
        let text = js::trim(&self.cur.src[p..q]);
        self.cur.emit_text(T::Actor, text, p, q);
    }

    fn number(&mut self) -> bool {
        let p = self.cur.p;
        let digit = |q| self.cur.char_at(q).is_some_and(|c| c.is_ascii_digit());
        let ends = |q| matches!(self.cur.char_at(q), Some(' ' | '\n'));
        let mut q = p;
        while digit(q) {
            q += 1;
        }
        if self.cur.char_at(q) == Some('.') && digit(q + 1) {
            let r = if digit(q + 2) { q + 3 } else { q + 2 };
            if ends(r) {
                self.cur.emit_to(T::Num, r);
                return true;
            }
        }
        if q > p && ends(q) {
            self.cur.emit_to(T::Num, q);
            return true;
        }
        false
    }

    fn dash(&mut self) {
        let p = self.cur.p;
        let at = |k| self.cur.char_at(p + k);
        let (kind, len) = match (at(1), at(2)) {
            (Some('>'), Some('>')) => (T::SolidArrow, 3),
            (Some('>'), _) => (T::SolidOpenArrow, 2),
            (Some('x' | 'X'), _) => (T::SolidCross, 2),
            (Some(')'), _) => (T::SolidPoint, 2),
            (Some('|'), Some('\\')) => (T::SolidArrowTop, 3),
            (Some('|'), Some('/')) => (T::SolidArrowBottom, 3),
            (Some('\\'), Some('\\')) => (T::StickArrowTop, 3),
            (Some('/'), Some('/')) => (T::StickArrowBottom, 3),
            (Some('-'), e) => match (e, at(3)) {
                (Some('>'), Some('>')) => (T::DottedArrow, 4),
                (Some('>'), _) => (T::DottedOpenArrow, 3),
                (Some('x' | 'X'), _) => (T::DottedCross, 3),
                (Some(')'), _) => (T::DottedPoint, 3),
                (Some('|'), Some('\\')) => (T::SolidArrowTopDotted, 4),
                (Some('|'), Some('/')) => (T::SolidArrowBottomDotted, 4),
                (Some('\\'), Some('\\')) => (T::StickArrowTopDotted, 4),
                (Some('/'), Some('/')) => (T::StickArrowBottomDotted, 4),
                _ => (T::Minus, 1),
            },
            _ => (T::Minus, 1),
        };
        self.cur.emit_to(kind, p + len);
    }

    fn reverse(&mut self, c: char) -> bool {
        let p = self.cur.p;
        let solid = match self.cur.char_at(p + 1) {
            Some('|') => true,
            Some(d) if d == c => false,
            _ => return false,
        };
        if self.cur.char_at(p + 2) != Some('-') {
            return false;
        }
        let dotted = self.cur.char_at(p + 3) == Some('-');
        let top = c == '/';
        let kind = match (solid, dotted, top) {
            (true, true, true) => T::SolidArrowTopReverseDotted,
            (true, true, false) => T::SolidArrowBottomReverseDotted,
            (true, false, true) => T::SolidArrowTopReverse,
            (true, false, false) => T::SolidArrowBottomReverse,
            (false, true, true) => T::StickArrowTopReverseDotted,
            (false, true, false) => T::StickArrowBottomReverseDotted,
            (false, false, true) => T::StickArrowTopReverse,
            (false, false, false) => T::StickArrowBottomReverse,
        };
        self.cur.emit_to(kind, p + if dotted { 4 } else { 3 });
        true
    }

    fn title(&mut self) -> bool {
        let p = self.cur.p;
        if !starts_with_caseless(self.cur.src, p, "title") {
            return false;
        }
        let mut q = p + 5;
        let legacy = self.cur.char_at(q) == Some(':');
        if legacy {
            q += 1;
        }
        if !self.is(q, is_space) {
            return false;
        }
        let next = self.cur.after(q);
        if !self.is(next, is_line) {
            return false;
        }
        let end = self.line_run(next);
        self.cur
            .emit_to(if legacy { T::LegacyTitle } else { T::Title }, end);
        true
    }

    fn acc(&mut self) -> bool {
        let p = self.cur.p;
        let descr = starts_with_caseless(self.cur.src, p, "accdescr");
        if !descr && !starts_with_caseless(self.cur.src, p, "acctitle") {
            return false;
        }
        let q = self.cur.space_end(p + 8);
        match self.cur.char_at(q) {
            Some(':') => {
                let end = self.cur.space_end(q + 1);
                self.cur
                    .emit_to(if descr { T::AccDescr } else { T::AccTitle }, end);
                self.cur.push(if descr { S::AccDescr } else { S::AccTitle });
                true
            }
            Some('{') if descr => {
                self.cur.p = self.cur.space_end(q + 1);
                self.cur.push(S::AccDescrMultiline);
                true
            }
            _ => false,
        }
    }

    fn keywords(&mut self, c: char) -> bool {
        match c.to_ascii_lowercase() {
            'a' => {
                self.keyword("actor", T::ParticipantActor, Some(S::Id))
                    || self.keyword("alt", T::Alt, Some(S::Line))
                    || self.keyword("and", T::And, Some(S::Line))
                    || self.keyword("activate", T::Activate, Some(S::Id))
                    || self.acc()
                    || self.keyword("autonumber", T::Autonumber, None)
            }
            'b' => {
                self.keyword("box", T::Box, Some(S::Line))
                    || self.keyword("break", T::Break, Some(S::Line))
            }
            'c' => {
                self.keyword("create", T::Create, None)
                    || self.keyword("critical", T::Critical, Some(S::Line))
            }
            'd' => {
                self.keyword("destroy", T::Destroy, Some(S::Id))
                    || self.menu("details", T::Details)
                    || self.keyword("deactivate", T::Deactivate, Some(S::Id))
            }
            'e' => {
                self.keyword("else", T::Else, Some(S::Line)) || self.keyword("end", T::EndKw, None)
            }
            'l' => {
                self.keyword("loop", T::Loop, Some(S::Line))
                    || self.keyword("left of", T::LeftOf, None)
                    || self.menu("links", T::Links)
                    || self.menu("link", T::Link)
            }
            'n' => self.keyword("note", T::Note, None),
            'o' => {
                self.keyword("opt", T::Opt, Some(S::Line))
                    || self.keyword("option", T::Option, Some(S::Line))
                    || self.keyword("over", T::Over, None)
                    || self.keyword("off", T::Off, None)
            }
            'p' => {
                self.keyword("participant", T::Participant, Some(S::Id))
                    || self.keyword("par", T::Par, Some(S::Line))
                    || self.keyword("par_over", T::ParOver, Some(S::Line))
                    || self.menu("properties", T::Properties)
            }
            'r' => {
                self.keyword("rect", T::Rect, Some(S::Line))
                    || self.keyword("right of", T::RightOf, None)
            }
            's' => self.keyword("sequencediagram", T::Sd, None),
            't' => self.title(),
            _ => false,
        }
    }

    fn initial(&mut self, c: char) {
        let p = self.cur.p;
        if c == '\n' {
            let mut q = p + 1;
            while self.cur.char_at(q) == Some('\n') {
                q += 1;
            }
            self.cur.emit_to(T::Newline, q);
            return;
        }
        if is_space(c) {
            self.cur.p = self.cur.space_end(p);
            return;
        }
        if c == '#' || (c == '%' && self.cur.char_at(p + 1) != Some('{')) {
            self.cur.p = self.eol(p);
            return;
        }
        // An astral character's second unit is no `%`.
        if c != '}'
            && width_at(self.cur.src, p) < 4
            && self.cur.starts_with("%%", self.cur.after(p))
        {
            self.cur.p = self.eol(p);
            return;
        }
        if (c.is_ascii_digit() || c == '.') && self.number() {
            return;
        }
        if c.is_ascii_alphabetic() && self.keywords(c) {
            return;
        }
        match c {
            ',' => self.cur.emit_to(T::Comma, p + 1),
            ';' => self.cur.emit_to(T::Newline, p + 1),
            _ if is_actor_start(c) => self.actor(),
            '-' => self.dash(),
            '<' if self.cur.starts_with("<<->>", p) => {
                self.cur.emit_to(T::BidirectionalSolidArrow, p + 5)
            }
            '<' if self.cur.starts_with("<<-->>", p) => {
                self.cur.emit_to(T::BidirectionalDottedArrow, p + 6)
            }
            '/' | '\\' if self.reverse(c) => {}
            ':' => {
                let end = self.line_run(p + 1);
                self.cur.emit_to(T::Txt, end);
            }
            '+' => self.cur.emit_to(T::Plus, p + 1),
            '(' if self.cur.char_at(p + 1) == Some(')') => self.cur.emit_to(T::Central, p + 2),
            _ => self.cur.emit_to(T::Invalid, p + 1),
        }
    }

    /// Whether ` as ` follows the name that ends at `a`.
    fn alias_after(&self, a: usize) -> bool {
        let w = self.cur.space_end(a);
        w > a && starts_with_caseless(self.cur.src, w, "as") && self.is(w + 2, is_space)
    }

    /// A participant name, up to an alias, a config object, or the end of the
    /// statement.
    fn id(&mut self, c: char) -> bool {
        if self.skip_same_line() {
            return true;
        }
        let p = self.cur.p;
        if c == '#' {
            self.cur.p = self.eol(p);
            return true;
        }
        if self.cur.starts_with("@{", p) {
            self.begin(T::ConfigStart, 2, S::Config);
            return true;
        }
        let mut a = p;
        while self.is(a, is_id) {
            a = self.cur.after(a);
        }
        if a > p {
            if self.cur.starts_with("@{", self.cur.space_end(a)) {
                self.cur.emit_to(T::Actor, a);
                return true;
            }
            if self.alias_after(a) {
                self.cur.emit_to(T::Actor, a);
                self.cur.push(S::Alias);
                return true;
            }
        }
        let mut b = a;
        while self.is(b, is_id_wide) {
            b = self.cur.after(b);
        }
        if b > p {
            // Before any other delimiter the name must be followed by a
            // comment, so it ends at the last one.
            let mut cut = b;
            if self
                .cur
                .char_at(b)
                .is_some_and(|end| end != '\n' && end != ';')
            {
                cut -= 1;
                while cut > p && self.cur.src.as_bytes()[cut] != b'#' {
                    cut -= 1;
                }
            }
            if cut > p {
                let text = js::trim(&self.cur.src[p..cut]);
                self.cur.emit_text(T::Actor, text, p, cut);
                self.cur.pop();
                return true;
            }
        }
        if self.cur.char_at(b) == Some('<') {
            let e = self.eol(p);
            self.cur.emit_to(T::Invalid, e);
            self.cur.pop();
            return true;
        }
        if c != '\n' {
            let e = self.eol(p);
            let text = js::trim(&self.cur.src[p..e]);
            self.cur.emit_text(T::Invalid, text, p, e);
            self.cur.pop();
            return true;
        }
        false
    }

    fn step(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match self.cur.state {
            S::Initial => self.initial(c),
            S::Id => return self.id(c),
            S::Alias => {
                if self.skip_same_line() {
                    return true;
                }
                if c == '#' {
                    self.cur.p = self.eol(p);
                    return true;
                }
                self.cur.pop();
                self.cur.pop();
                if self.cur.keyword_caseless("as", self.cur.p) {
                    self.begin(T::As, 2, S::Line);
                } else {
                    self.cur.emit_to(T::Newline, p);
                }
            }
            S::Line => {
                if self.skip_same_line() {
                    return true;
                }
                if c == '#' {
                    self.cur.p = self.eol(p);
                    return true;
                }
                let end = self.line_run(p);
                self.cur.emit_to(T::RestOfLine, end);
                self.cur.pop();
            }
            S::Config => {
                if c != '}' {
                    let q = self.cur.find('}', p);
                    self.cur.emit_to(T::ConfigContent, q);
                } else {
                    let alias = self.alias_after(p + 1);
                    self.cur.emit_to(T::ConfigEnd, p + 1);
                    self.cur.pop();
                    if alias {
                        self.cur.push(S::Alias);
                    } else {
                        self.cur.pop();
                    }
                }
            }
            S::AccTitle | S::AccDescr => {
                let kind = if self.cur.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                let e = self.eol(p);
                self.cur.emit_to(kind, e);
                self.cur.pop();
            }
            S::AccDescrMultiline => {
                if c == '}' {
                    self.cur.p += 1;
                    self.cur.pop();
                } else {
                    let q = self.cur.find('}', p);
                    self.cur.emit_to(T::AccDescrMultilineValue, q);
                }
            }
        }
        true
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lx = Lexer {
        cur: Cursor::new(src, S::Initial),
        next_nl: None,
    };
    while let Some(c) = lx.cur.char_at(lx.cur.p) {
        if !lx.step(c) {
            let p = lx.cur.p;
            lx.cur.tokens.push(Token::new(T::Error, "", p));
            return lx.cur.tokens;
        }
    }

    // Mermaid's lexer tries its rules once more on the empty remainder before
    // reporting the end.
    let at_end = match lx.cur.state {
        S::Initial | S::Alias => Some(T::Newline),
        S::Line => Some(T::RestOfLine),
        S::AccTitle => Some(T::AccTitleValue),
        S::AccDescr => Some(T::AccDescrValue),
        S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
        S::Id | S::Config => None,
    };
    if let Some(kind) = at_end {
        lx.cur.emit(kind, n, n);
    }
    lx.cur.tokens.push(Token::new(T::End, "", n));
    lx.cur.tokens
}
