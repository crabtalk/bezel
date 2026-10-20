//! Pele's `diagrams/state/lexer.ts`: the tokens of Mermaid's state grammar,
//! lexed by hand.

use super::tokens::T;
use crate::{
    js::{self, Lazy},
    token::{Cursor, Token},
    util::chars::{digits_end, is_space, is_word, starts_with_caseless, width_at},
};

static RE_DIRECTION: Lazy = Lazy::new(r"direction", "gi");
/// The direction kinds, and the words naming them, in grammar order.
const DIRECTIONS: [(T, &str); 4] = [
    (T::DirectionTb, "tb"),
    (T::DirectionBt, "bt"),
    (T::DirectionRl, "rl"),
    (T::DirectionLr, "lr"),
];
static RE_STEREOTYPE: Lazy = Lazy::new(
    r"<<(?:fork|join|choice)>>|\[\[(?:fork|join|choice)\]\]",
    "gi",
);
/// The stereotype kinds in `<< >>` and then in `[[ ]]`, in grammar order.
const STEREOTYPES: [T; 6] = [T::Fork, T::Join, T::Choice, T::Fork, T::Join, T::Choice];

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Struct,
    State,
    StateString,
    StateId,
    Scale,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Classdef,
    ClassdefId,
    Class,
    ClassStyle,
    Style,
    StyledefStyles,
    Note,
    NoteId,
    NoteText,
    FloatingNote,
    FloatingNoteId,
}

impl S {
    /// The token Jison gives in this condition when it tries its rules once
    /// more on the empty rest of the input.
    fn end_token(self) -> Option<T> {
        match self {
            Self::Initial => Some(T::Nl),
            Self::AccTitle => Some(T::AccTitleValue),
            Self::AccDescr => Some(T::AccDescrValue),
            Self::AccDescrMultiline => Some(T::AccDescrMultilineValue),
            Self::ClassdefId => Some(T::ClassdefStyleopts),
            Self::ClassStyle => Some(T::Styleclass),
            Self::StyledefStyles => Some(T::StyledefStyleopts),
            Self::StateString => Some(T::StateDescr),
            Self::FloatingNote => Some(T::NoteText),
            Self::StateId | Self::FloatingNoteId => Some(T::Id),
            _ => None,
        }
    }
}

/// Finds the next match of a search at or after a position, remembering the
/// answer so that asking at each token costs one scan of the text in total.
struct Finder {
    search: fn(&str, usize) -> Option<usize>,
    /// The position last searched from, and what was found.
    last: Option<(usize, Option<usize>)>,
}

impl Finder {
    fn new(search: fn(&str, usize) -> Option<usize>) -> Self {
        Self { search, last: None }
    }

    fn at(&mut self, src: &str, q: usize) -> Option<usize> {
        match self.last {
            Some((from, next)) if q >= from && next.is_none_or(|next| q <= next) => next,
            _ => {
                let next = (self.search)(src, q);
                self.last = Some((q, next));
                next
            }
        }
    }
}

fn find_direction(src: &str, q: usize) -> Option<usize> {
    RE_DIRECTION.exec_at(src, q).map(|m| m.start())
}

fn find_stereotype(src: &str, q: usize) -> Option<usize> {
    RE_STEREOTYPE.exec_at(src, q).map(|m| m.start())
}

fn find_line_end(src: &str, q: usize) -> Option<usize> {
    src.get(q..)?
        .find(['\n', '\r', '\u{2028}', '\u{2029}'])
        .map(|at| q + at)
}

fn find_percent(src: &str, q: usize) -> Option<usize> {
    src.get(q..)?.find("%%").map(|at| q + at)
}

fn find_brace(src: &str, q: usize) -> Option<usize> {
    src.get(q..)?.find('{').map(|at| q + at)
}

/// What a rule found on the line being lexed, kept until a position past
/// that line is asked about.
struct LineFinds<const N: usize> {
    /// The positions the finds hold for.
    span: Option<(usize, usize)>,
    /// For each kind, where it was last found, and where it ends.
    at: [Option<(usize, usize)>; N],
}

impl<const N: usize> LineFinds<N> {
    fn new() -> Self {
        Self {
            span: None,
            at: [None; N],
        }
    }

    fn holds(&self, q: usize) -> bool {
        self.span.is_some_and(|(from, to)| (from..=to).contains(&q))
    }

    /// The first kind found at or after `q`, where it is, and where it ends.
    fn first(&self, q: usize) -> Option<(usize, usize, usize)> {
        self.at.iter().enumerate().find_map(|(kind, found)| {
            found
                .filter(|&(at, _)| at >= q)
                .map(|(at, end)| (kind, at, end))
        })
    }
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    next_direction: Finder,
    next_stereotype: Finder,
    next_line_end: Finder,
    next_percent: Finder,
    next_brace: Finder,
    /// The last `direction <dir>` of each kind on the line being lexed, and
    /// where its direction ends.
    directions: LineFinds<4>,
    /// The same for `<<fork>>`, `<<join>>`, `<<choice>>` and their `[[ ]]`
    /// spellings, and where each ends.
    stereotypes: LineFinds<6>,
}

impl<'s> Lexer<'s> {
    fn at(&self, q: usize) -> Option<char> {
        self.cur.char_at(q)
    }

    fn emit(&mut self, kind: T, start: usize, end: usize) {
        self.cur.emit(kind, start, end);
    }

    fn emit_text(&mut self, kind: T, text: &str, start: usize, end: usize) {
        self.cur
            .emit_text(kind, js::trim(text).to_string(), start, end);
    }

    /// Whether `word`, in lower case, is at `at` in any case.
    fn word(&self, word: &str, at: usize) -> bool {
        starts_with_caseless(self.cur.src, at, word)
    }

    /// [`Lexer::word`] with no word character after it.
    fn keyword(&self, word: &str, at: usize) -> bool {
        self.word(word, at) && !self.at(at + word.len()).is_some_and(is_word)
    }

    fn ws_end(&self, q: usize) -> usize {
        self.cur.space_end(q)
    }

    /// The end of the run of whitespace at `q` that holds no line feed.
    fn blank_end(&self, q: usize) -> usize {
        self.cur.until(q, |c| c == '\n' || !is_space(c))
    }

    fn eol(&self, q: usize) -> usize {
        self.cur.line_end(q)
    }

    fn line_end(&mut self, q: usize) -> usize {
        self.next_line_end
            .at(self.cur.src, q)
            .unwrap_or(self.cur.src.len())
    }

    /// Rules `.*direction\s+TB[^\n]*` and its three siblings: the first kind,
    /// in grammar order, found between `q` and the end of its line, and
    /// where its direction ends.
    fn direction(&mut self, q: usize) -> Option<(T, usize)> {
        if !self.directions.holds(q) {
            let src = self.cur.src;
            let first = self.next_direction.at(src, q);
            let end = if first.is_none() {
                src.len()
            } else {
                self.line_end(q)
            };
            self.directions = LineFinds::new();
            self.directions.span = Some((q, end));
            let mut found = first;
            while let Some(at) = found.filter(|&at| at < end) {
                let dir = self.ws_end(at + 9);
                if dir != at + 9
                    && let Some(kind) = DIRECTIONS
                        .iter()
                        .position(|&(_, word)| self.word(word, dir))
                {
                    self.directions.at[kind] = Some((at, dir + 2));
                }
                found = self.next_direction.at(src, at + 1);
            }
        }
        self.directions
            .first(q)
            .map(|(kind, _, end)| (DIRECTIONS[kind].0, end))
    }

    /// The first fork, join or choice stereotype from `q` to the end of its
    /// line: its kind, where it is, and where it ends.
    fn stereotype(&mut self, q: usize) -> Option<(T, usize, usize)> {
        if !self.stereotypes.holds(q) {
            let src = self.cur.src;
            let first = self.next_stereotype.at(src, q);
            let end = if first.is_none() {
                src.len()
            } else {
                self.line_end(q)
            };
            self.stereotypes = LineFinds::new();
            self.stereotypes.span = Some((q, end));
            let mut found = first;
            while let Some(at) = found.filter(|&at| at < end) {
                let angled = self.at(at) == Some('<');
                let (kind, len) = match self.at(at + 2).map(|c| c.to_ascii_lowercase()) {
                    Some('f') => (0, 8),
                    Some('j') => (1, 8),
                    _ => (2, 10),
                };
                self.stereotypes.at[if angled { kind } else { 3 + kind }] = Some((at, at + len));
                found = self.next_stereotype.at(src, at + 1);
            }
        }
        self.stereotypes
            .first(q)
            .map(|(kind, at, end)| (STEREOTYPES[kind], at, end))
    }

    /// The grammar's processId(): an id ends at an inline `%%`, and a text
    /// that starts with one is dropped.
    fn emit_id(&mut self, start: usize, end: usize) -> bool {
        match self.next_percent.at(self.cur.src, start) {
            Some(at) if at + 2 <= end => {
                if at == start {
                    self.cur.p = end;
                    return false;
                }
                self.emit(T::Id, start, at);
            }
            _ => self.emit(T::Id, start, end),
        }
        true
    }

    fn is_comment(&self, c: char) -> bool {
        let p = self.cur.p;
        c == '#' || (c == '%' && self.at(p + 1) == Some('%') && self.at(p + 2) != Some('{'))
    }

    /// `INITIAL`, and the inside of a composite state.
    fn statement(&mut self, c: char) -> Result<(), String> {
        let src = self.cur.src;
        let p = self.cur.p;
        let root = self.cur.state == S::Initial;
        let lower = c.to_ascii_lowercase();
        if root {
            if lower == 'c' && self.keyword("click", p) {
                self.emit(T::Click, p, p + 5);
                return Ok(());
            }
            if lower == 'h' && self.keyword("href", p) {
                self.emit(T::Href, p, p + 4);
                return Ok(());
            }
            if c == '"'
                && let Some(close) = src[p + 1..].find('"')
            {
                self.emit(T::String, p, p + 1 + close + 1);
                return Ok(());
            }
            if lower == 'd' && self.keyword("default", p) {
                self.emit(T::Default, p, p + 7);
                return Ok(());
            }
            if let Some((kind, end)) = self.direction(p) {
                let end = self.eol(end);
                self.emit(kind, p, end);
                return Ok(());
            }
            if c == '\n' {
                let end = self.cur.until(p + 1, |c| c != '\n');
                self.emit(T::Nl, p, end);
                return Ok(());
            }
            if is_space(c) {
                self.cur.p = self.ws_end(p);
                return Ok(());
            }
        } else if c != '\n' && is_space(c) {
            self.cur.p = self.blank_end(p + c.len_utf8());
            return Ok(());
        }
        if self.is_comment(c) {
            self.cur.p = self.eol(p);
            return Ok(());
        }
        match lower {
            's' => {
                if root && self.word("scale", p) {
                    let end = self.ws_end(p + 5);
                    if end > p + 5 {
                        self.emit(T::Scale, p, end);
                        self.cur.push(S::Scale);
                        return Ok(());
                    }
                }
                if self.word("style", p) {
                    let end = self.ws_end(p + 5);
                    if end > p + 5 {
                        self.emit(T::Style, p, end);
                        self.cur.push(S::Style);
                        return Ok(());
                    }
                }
                if self.word("state", p) {
                    let end = self.ws_end(p + 5);
                    if end > p + 5 {
                        self.cur.p = end;
                        self.cur.push(S::State);
                        return Ok(());
                    }
                    if root && self.word("diagram", p + 5) {
                        let from = if self.word("-v2", p + 12) {
                            p + 15
                        } else {
                            p + 12
                        };
                        let end = self.ws_end(from);
                        if end > from {
                            self.emit(T::Sd, p, end);
                            return Ok(());
                        }
                    }
                }
            }
            'c' if self.word("class", p) => {
                let def = self.word("def", p + 5);
                let from = if def { p + 8 } else { p + 5 };
                let end = self.ws_end(from);
                if end > from {
                    self.emit(if def { T::Classdef } else { T::Class }, p, end);
                    self.cur.push(if def { S::Classdef } else { S::Class });
                    return Ok(());
                }
            }
            'a' if root && (self.word("acctitle", p) || self.word("accdescr", p)) => {
                let title = self.at(p + 3).is_some_and(|c| c.eq_ignore_ascii_case(&'t'));
                let q = self.ws_end(p + 8);
                match self.at(q) {
                    Some(':') => {
                        let end = self.ws_end(q + 1);
                        self.emit(if title { T::AccTitle } else { T::AccDescr }, p, end);
                        self.cur.push(if title { S::AccTitle } else { S::AccDescr });
                        return Ok(());
                    }
                    Some('{') if !title => {
                        self.cur.p = self.ws_end(q + 1);
                        self.cur.push(S::AccDescrMultiline);
                        return Ok(());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        if root {
            if c == '{' {
                self.emit(T::StructStart, p, p + 1);
                self.cur.push(S::Struct);
                return Ok(());
            }
        } else {
            if let Some((kind, end)) = self.direction(p) {
                let end = self.eol(end);
                self.emit(kind, p, end);
                return Ok(());
            }
            if c == '}' {
                self.emit(T::StructStop, p, p + 1);
                self.cur.pop();
                return Ok(());
            }
            if c == '\n' {
                self.cur.p = p + 1;
                return Ok(());
            }
        }
        if lower == 'n' && self.word("note", p) {
            let end = self.ws_end(p + 4);
            if end > p + 4 {
                self.emit(T::Note, p, end);
                self.cur.push(S::Note);
                return Ok(());
            }
        }
        if root && lower == 'h' && self.keyword("hide empty description", p) {
            self.emit(T::HideEmpty, p, p + 22);
            return Ok(());
        }
        let next = self.at(p + 1);
        if c == '[' && next == Some('*') && self.at(p + 2) == Some(']') {
            self.emit(T::EdgeState, p, p + 3);
            return Ok(());
        }
        let end = self
            .cur
            .until(p, |c| matches!(c, ':' | '-' | '{') || is_space(c));
        if end > p {
            self.emit_id(p, end);
            return Ok(());
        }
        if c == ':' {
            let mut end = p + 1;
            while let Some(d) = self.at(end) {
                if d == '\n' || d == ';' {
                    break;
                }
                if d == ':' {
                    if matches!(self.at(end + 1), None | Some(':' | '\n' | ';')) {
                        break;
                    }
                    end += 1;
                }
                end += width_at(src, end);
            }
            if end > p + 1 {
                self.emit_text(T::Descr, &src[p..end], p, end);
                return Ok(());
            }
            if next == Some(':') && self.at(p + 2) == Some(':') {
                self.emit(T::StyleSeparator, p, p + 3);
                return Ok(());
            }
        } else if c == '-' && next == Some('-') {
            if self.at(p + 2) == Some('>') {
                self.emit(T::Arrow, p, p + 3);
                return Ok(());
            }
            if !root {
                self.emit(T::Concurrent, p, p + 2);
                return Ok(());
            }
        }
        if !root {
            return Err(String::new());
        }
        self.emit(T::Invalid, p, p + c.len_utf8());
        Ok(())
    }

    /// After the `state` keyword.
    fn state(&mut self, c: char) -> Result<(), String> {
        let src = self.cur.src;
        let p = self.cur.p;
        if c != '\n' && is_space(c) {
            self.cur.p = self.blank_end(p + c.len_utf8());
            return Ok(());
        }
        if self.is_comment(c) {
            self.cur.p = self.eol(p);
            return Ok(());
        }
        if let Some((kind, at, end)) = self.stereotype(p) {
            self.emit_text(kind, &src[p..at], p, end);
            self.cur.pop();
            return Ok(());
        }
        if c == '"' {
            self.cur.p = p + 1;
            self.cur.push(S::StateString);
            return Ok(());
        }
        let q = if c == '\n' { self.ws_end(p) } else { p };
        if self.word("as", q) {
            let end = self.ws_end(q + 2);
            if end > q + 2 {
                self.emit(T::As, p, end);
                self.cur.push(S::StateId);
                return Ok(());
            }
        }
        if c == '\n' {
            self.cur.p = p + 1;
            self.cur.pop();
            return Ok(());
        }
        if c == '{' {
            self.emit(T::StructStart, p, p + 1);
            self.cur.state = S::Struct;
            return Ok(());
        }
        if is_word(c) {
            let first = self.cur.word_end(p);
            let second = self.ws_end(first);
            if second > first
                && self.at(second).is_some_and(is_word)
                && let Some(brace) = self.next_brace.at(src, second + 1)
                && brace < self.line_end(second)
            {
                return Err(format!(
                    "Error: State name must be a single word. Found: \"{}\"",
                    js::trim(&src[p..brace + 1])
                ));
            }
        }
        let end = self
            .cur
            .until(p + c.len_utf8(), |c| c == '{' || is_space(c));
        self.emit(T::CompositState, p, end);
        Ok(())
    }

    fn note_text(&mut self) -> Result<(), String> {
        let src = self.cur.src;
        let p = self.cur.p;
        let q = self.ws_end(p);
        if self.at(q) == Some(':') {
            let end = self.cur.until(q + 1, |c| matches!(c, ':' | '\n' | ';'));
            if end > q + 1 {
                // Mermaid drops two characters here, expecting " :"; with no
                // space before the colon the first character of the note goes
                // too.
                let from = p + js::units_end(&src[p..], 2).min(end);
                self.emit_text(T::NoteText, &src[from..end], p, end);
                self.cur.pop();
                return Ok(());
            }
        }
        let mut line = src[p..].find('\n').map(|at| p + at);
        let mut end = 0;
        while let Some(at) = line {
            end = self.ws_end(at);
            if self.keyword("end note", end) {
                break;
            }
            line = src[end..].find('\n').map(|at| end + at);
        }
        if line.is_none() {
            return Err(String::new());
        }
        self.emit_text(T::NoteText, &src[p..end], p, end + 8);
        self.cur.pop();
        Ok(())
    }

    /// Reads one token or skips text; the error token's text where no rule
    /// matches.
    fn step(&mut self, c: char) -> Result<(), String> {
        let src = self.cur.src;
        let p = self.cur.p;
        match self.cur.state {
            S::Initial | S::Struct => return self.statement(c),
            S::State => return self.state(c),
            S::StateString => {
                if c == '"' {
                    self.cur.p = p + 1;
                    self.cur.pop();
                } else {
                    let end = self.cur.until(p, |c| c == '"');
                    self.emit(T::StateDescr, p, end);
                }
            }
            S::StateId => {
                let end = self.cur.until(p, |c| c == '\n' || c == '{');
                if self.emit_id(p, end) {
                    self.cur.pop();
                }
            }
            S::Scale => {
                if c.is_ascii_digit() {
                    self.emit(T::Width, p, digits_end(src, p));
                    return Ok(());
                }
                let q = self.ws_end(p);
                if q > p && self.keyword("width", q) {
                    self.cur.p = q + 5;
                    self.cur.pop();
                    return Ok(());
                }
                return Err(String::new());
            }
            S::AccTitle | S::AccDescr | S::ClassdefId | S::ClassStyle | S::StyledefStyles => {
                let kind = match self.cur.state {
                    S::AccTitle => T::AccTitleValue,
                    S::AccDescr => T::AccDescrValue,
                    S::ClassdefId => T::ClassdefStyleopts,
                    S::ClassStyle => T::Styleclass,
                    _ => T::StyledefStyleopts,
                };
                let end = self.eol(p);
                self.emit(kind, p, end);
                self.cur.pop();
            }
            S::AccDescrMultiline => {
                if c == '}' {
                    self.cur.p = p + 1;
                    self.cur.pop();
                } else {
                    let end = self.cur.until(p, |c| c == '}');
                    self.emit(T::AccDescrMultilineValue, p, end);
                }
            }
            S::Classdef => {
                let default = self.word("default", p) && self.ws_end(p + 7) > p + 7;
                let id_end = if default { p + 7 } else { self.cur.word_end(p) };
                let end = self.ws_end(id_end);
                if id_end == p || end == id_end {
                    return Err(String::new());
                }
                let kind = if default {
                    T::DefaultClassdefId
                } else {
                    T::ClassdefId
                };
                self.emit(kind, p, end);
                self.cur.state = S::ClassdefId;
            }
            S::Class => {
                let mut end = self.cur.word_end(p);
                if end == p {
                    return Err(String::new());
                }
                while self.at(end) == Some(',') {
                    let from = self.ws_end(end + 1);
                    let to = self.cur.word_end(from);
                    if to == from {
                        break;
                    }
                    end = to;
                }
                self.emit(T::ClassentityIds, p, end);
                self.cur.state = S::ClassStyle;
            }
            S::Style => {
                let ids_end = self.cur.until(p, |c| !(is_word(c) || c == ','));
                let end = self.ws_end(ids_end);
                if ids_end == p || end == ids_end {
                    return Err(String::new());
                }
                self.emit(T::StyleIds, p, end);
                self.cur.state = S::StyledefStyles;
            }
            S::Note => {
                if self.keyword("left of", p) {
                    self.emit(T::LeftOf, p, p + 7);
                    self.cur.state = S::NoteId;
                } else if self.keyword("right of", p) {
                    self.emit(T::RightOf, p, p + 8);
                    self.cur.state = S::NoteId;
                } else if c == '"' {
                    self.cur.p = p + 1;
                    self.cur.state = S::FloatingNote;
                } else {
                    return Err(String::new());
                }
            }
            S::NoteId => {
                let q = self.ws_end(p);
                let end = self.cur.until(q, |c| c == ':' || c == '-' || is_space(c));
                if end == q {
                    return Err(String::new());
                }
                if self.emit_id(p, end) {
                    self.cur.state = S::NoteText;
                }
            }
            S::NoteText => return self.note_text(),
            S::FloatingNote => {
                let q = self.ws_end(p);
                if self.word("as", q) {
                    let end = self.ws_end(q + 2);
                    self.emit(T::As, p, end);
                    self.cur.state = S::FloatingNoteId;
                } else if c == '"' {
                    self.cur.p = p + 1;
                } else {
                    let end = self.cur.until(p, |c| c == '"');
                    self.emit(T::NoteText, p, end);
                }
            }
            S::FloatingNoteId => {
                let end = self.eol(p);
                if self.emit_id(p, end) {
                    self.cur.pop();
                }
            }
        }
        Ok(())
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let mut lexer = Lexer {
        cur: Cursor::new(src, S::Initial),
        next_direction: Finder::new(find_direction),
        next_stereotype: Finder::new(find_stereotype),
        next_line_end: Finder::new(find_line_end),
        next_percent: Finder::new(find_percent),
        next_brace: Finder::new(find_brace),
        directions: LineFinds::new(),
        stereotypes: LineFinds::new(),
    };

    while let Some(c) = lexer.at(lexer.cur.p) {
        if let Err(message) = lexer.step(c) {
            let p = lexer.cur.p;
            let mut tokens = lexer.cur.tokens;
            tokens.push(Token::new(T::Error, message, p));
            return tokens;
        }
    }
    let n = src.len();
    if let Some(kind) = lexer.cur.state.end_token() {
        lexer.emit(kind, n, n);
    }
    let mut tokens = lexer.cur.tokens;
    tokens.push(Token::new(T::End, "", n));
    tokens
}
