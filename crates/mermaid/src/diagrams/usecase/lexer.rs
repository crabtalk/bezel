//! Pele's `diagrams/usecase/lexer.ts`. Offsets count UTF-16 units, as the
//! model reports them.

use super::{tokens::T, types::Span};
use crate::{
    detect::Kind,
    error::Error,
    js,
    util::chars::{is_blank, is_digit, is_hex, is_letter, is_word},
};

/// Each keyword, in the order of Mermaid's token list, and whether it
/// ignores case.
const KEYWORDS: [(&str, T, bool); 20] = [
    ("actor", T::Actor, false),
    ("systemBoundary", T::SystemBoundary, false),
    ("style", T::Style, false),
    ("end", T::End, false),
    ("extend", T::Extend, true),
    ("direction", T::Direction, false),
    ("TD", T::Td, false),
    ("TB", T::Tb, false),
    ("BT", T::Bt, false),
    ("LR", T::Lr, false),
    ("RL", T::Rl, false),
    ("note", T::Note, false),
    ("for", T::For, false),
    ("false", T::False, false),
    ("json", T::Json, false),
    ("classDef", T::ClassDef, false),
    ("class", T::Class, false),
    ("include", T::Include, true),
    ("true", T::True, false),
    ("usecase-beta", T::Usecase, false),
];

/// A token and the UTF-16 offsets it spans, end exclusive.
#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: T,
    pub start: usize,
    pub end: usize,
}

impl Token {
    fn new(kind: T, start: usize, end: usize) -> Self {
        Self { kind, start, end }
    }

    pub fn span(&self) -> Span {
        [self.start, self.end]
    }
}

/// A diagram's text as UTF-16 units, the way the lexer and the model count
/// it.
pub struct Source {
    pub text: String,
    pub units: Vec<u16>,
}

fn is_css_punctuation(c: char) -> bool {
    "!#$&*+/=?^|~".contains(c)
}

impl Source {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            units: text.encode_utf16().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// The unit at `at`, a surrogate as U+FFFD, or `None` past the end.
    pub fn char(&self, at: usize) -> Option<char> {
        let unit = *self.units.get(at)?;
        Some(char::from_u32(u32::from(unit)).unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    pub fn is(&self, at: usize, c: char) -> bool {
        self.char(at) == Some(c)
    }

    /// Whether the unit at `at` passes `test`; never past the end.
    pub fn matches(&self, at: usize, test: impl FnOnce(char) -> bool) -> bool {
        self.char(at).is_some_and(test)
    }

    pub fn slice(&self, from: usize, to: usize) -> String {
        let to = to.min(self.units.len());
        let from = from.min(to);
        String::from_utf16_lossy(&self.units[from..to])
    }

    pub fn starts_with(&self, at: usize, word: &str) -> bool {
        word.encode_utf16()
            .enumerate()
            .all(|(k, unit)| self.units.get(at + k) == Some(&unit))
    }

    /// `indexOf(needle, from)`.
    pub fn index_of(&self, needle: &str, from: usize) -> Option<usize> {
        let needle: Vec<u16> = needle.encode_utf16().collect();
        if needle.is_empty() {
            return Some(from.min(self.units.len()));
        }
        (from..self.units.len().saturating_sub(needle.len() - 1))
            .find(|&at| self.units[at..at + needle.len()] == needle[..])
    }

    /// Mermaid's INCLUDE and EXTEND are the only keywords matched without
    /// regard to case.
    fn starts_with_no_case(&self, at: usize, word: &str) -> bool {
        word.chars()
            .enumerate()
            .all(|(k, w)| self.matches(at + k, |c| c.eq_ignore_ascii_case(&w)))
    }

    /// The keyword that is a prefix of the text at `at`, and its length.
    fn keyword_at(&self, at: usize) -> Option<(T, usize)> {
        KEYWORDS
            .iter()
            .find(|&&(word, _, ignore_case)| {
                if ignore_case {
                    self.starts_with_no_case(at, word)
                } else {
                    self.starts_with(at, word)
                }
            })
            .map(|&(word, kind, _)| (kind, word.len()))
    }

    /// The line and column of `offset`, both from 1. `\r\n` is one line
    /// break.
    pub fn line_column(&self, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut line_start = 0;
        let mut i = 0;
        while i < offset {
            let c = self.char(i);
            if matches!(c, Some('\n' | '\r')) {
                if c == Some('\r') && i + 1 < offset && self.is(i + 1, '\n') {
                    i += 1;
                }
                line += 1;
                line_start = i + 1;
            }
            i += 1;
        }
        (line, offset - line_start + 1)
    }

    /// [`Self::line_column`] for text that starts at line and column
    /// `origin` of another; past the end counts as the end.
    pub fn line_column_from(&self, offset: usize, origin: (usize, usize)) -> (usize, usize) {
        match self.line_column(offset.min(self.len())) {
            (1, column) => (origin.0, origin.1 + column - 1),
            (line, column) => (origin.0 + line - 1, column),
        }
    }

    pub fn location_text(&self, start: usize, end: usize) -> String {
        let (line, column) = self.line_column(start);
        format!("line {line}, column {column} [{start},{end})")
    }

    /// `error` pointing at `offset`.
    pub fn place(&self, mut error: Error, offset: usize) -> Error {
        (error.line, error.column) = self.line_column(offset);
        error.of(Kind::Usecase)
    }

    fn line_end_at(&self, at: usize) -> bool {
        self.matches(at, |c| matches!(c, '\n' | '\r'))
    }

    /// The end of a quoted string that opens at `at`, or `None` when the line
    /// ends first.
    fn string_end(&self, at: usize, quote: char) -> Option<usize> {
        for k in at + 1..self.len() {
            if self.is(k, quote) {
                return Some(k + 1);
            }
            if self.line_end_at(k) {
                return None;
            }
        }
        None
    }

    fn starts_token(&self, at: usize) -> bool {
        match self.char(at) {
            Some('"') if self.is(at + 1, '`') => true,
            Some(quote @ ('"' | '\'')) => self.string_end(at, quote).is_some(),
            _ => true,
        }
    }

    /// Mermaid reports the first character no token starts with, and how
    /// many characters its lexer skipped before one did.
    fn lex_error(&self, offset: usize, length: usize) -> Error {
        let (line, column) = self.line_column(offset);
        self.place(
            Error::syntax(format!(
                "Error lexing usecase diagram: unexpected character: ->{}<- at offset: {offset}, skipped {length} characters. at line {line}, column {column} [{offset},{})",
                self.slice(offset, offset + 1),
                offset + length
            )),
            offset,
        )
    }

    /// Produces the tokens of Mermaid's Chevrotain lexer for use case
    /// diagrams: the first pattern in its list that matches wins, a keyword
    /// gives way to a longer identifier, and `json id@` and `<<` switch to
    /// modes that read a JSON object and a stereotype. Ends with three EOF
    /// tokens.
    pub fn tokenize(&self) -> Result<Vec<Token>, Error> {
        let mut tokens: Vec<Token> = Vec::new();
        let n = self.len();
        let mut i = 0;
        // Whether only blanks came before on this line, where comments and
        // accessibility lines may start.
        let mut line_start = true;
        let mut no_brace = false;

        'tokens: while let Some(c) = self.char(i) {
            if is_blank(c) {
                i += 1;
                continue;
            }
            let mut end = i + 1;
            let mut fresh = false;

            let kind = match c {
                c if is_word(c) => {
                    let mut line = None;
                    if c == 'a' && line_start && self.starts_with(i, "acc") {
                        let descr = self.starts_with(i + 3, "Descr");
                        if descr || self.starts_with(i + 3, "Title") {
                            let mut j = i + 8;
                            while self.matches(j, is_blank) {
                                j += 1;
                            }
                            if self.is(j, ':') {
                                line = Some(if descr {
                                    T::AccDescrLine
                                } else {
                                    T::AccTitleLine
                                });
                                end = j + 1;
                                while end < n && !self.line_end_at(end) {
                                    end += 1;
                                }
                            } else if self.is(j, '{') && descr && !no_brace {
                                match self.index_of("}", j + 1) {
                                    None => no_brace = true,
                                    Some(close) => {
                                        line = Some(T::AccDescrBlock);
                                        end = close + 1;
                                    }
                                }
                            }
                        }
                    } else if c == 'j'
                        && self.starts_with(i, "json")
                        && self.matches(i + 4, is_blank)
                    {
                        let mut j = i + 5;
                        while self.matches(j, is_blank) {
                            j += 1;
                        }
                        let id = j;
                        while self.matches(j, is_word) {
                            j += 1;
                        }
                        if j > id {
                            while self.matches(j, is_blank) {
                                j += 1;
                            }
                            if self.is(j, '@') {
                                j += 1;
                                while self.matches(j, is_blank) {
                                    j += 1;
                                }
                                if self.is(j, '{') {
                                    tokens.push(Token::new(T::JsonDeclarationStart, i, j));
                                    let mut depth = 0i64;
                                    let mut quoted = false;
                                    let mut escaped = false;
                                    let mut k = j;
                                    while let Some(ch) = self.char(k) {
                                        if quoted {
                                            if escaped {
                                                escaped = false;
                                            } else if ch == '\\' {
                                                escaped = true;
                                            } else if ch == '"' {
                                                quoted = false;
                                            }
                                        } else if ch == '"' {
                                            quoted = true;
                                        } else if ch == '{' {
                                            depth += 1;
                                        } else if ch == '}' {
                                            depth -= 1;
                                            if depth == 0 {
                                                break;
                                            }
                                        }
                                        k += 1;
                                    }
                                    let (kind, after) = if k < n {
                                        (T::JsonObjectLiteral, k + 1)
                                    } else {
                                        (T::UnclosedJsonObjectLiteral, n)
                                    };
                                    tokens.push(Token::new(kind, j, after));
                                    i = after;
                                    line_start = false;
                                    continue 'tokens;
                                }
                            }
                        }
                    }
                    if let Some(line) = line {
                        line
                    } else {
                        while self.matches(end, is_word) {
                            end += 1;
                        }
                        if let Some((keyword, length)) = self.keyword_at(i) {
                            // A keyword that is only the start of a longer
                            // word is an identifier.
                            if keyword == T::Usecase || end - i == length {
                                end = i + length;
                                keyword
                            } else {
                                T::Identifier
                            }
                        } else if matches!(c, 'o' | 'x')
                            && self.is(i + 1, '-')
                            && self.is(i + 2, '-')
                        {
                            end = i + 3;
                            if c == 'o' {
                                T::BackwardCircle
                            } else {
                                T::BackwardCross
                            }
                        } else if c.is_ascii_digit() {
                            // `1mg` is an identifier, and `1.5px` is a number
                            // because the identifier stops at the dot.
                            let mut digits = i + 1;
                            while self.matches(digits, is_digit) {
                                digits += 1;
                            }
                            if self.is(digits, '.') && self.matches(digits + 1, is_digit) {
                                end = digits + 2;
                                while self.matches(end, is_digit) {
                                    end += 1;
                                }
                                while self.matches(end, is_letter) {
                                    end += 1;
                                }
                                T::Number
                            } else {
                                T::Identifier
                            }
                        } else {
                            let mut kind = T::Identifier;
                            while self.is(end, '-') && self.matches(end + 1, is_word) {
                                kind = T::CssIdentifier;
                                end += 2;
                                while self.matches(end, is_word) {
                                    end += 1;
                                }
                            }
                            kind
                        }
                    }
                }
                '\n' => {
                    fresh = true;
                    T::Newline
                }
                '\r' => {
                    fresh = true;
                    if self.is(end, '\n') {
                        end += 1;
                    }
                    T::Newline
                }
                '"' if self.is(i + 1, '`') => {
                    let close = self.index_of("`\"", i + 2);
                    end = close.map_or(n, |close| close + 2);
                    if close.is_none() {
                        T::UnclosedMarkdownString
                    } else {
                        T::MarkdownString
                    }
                }
                '"' | '\'' => match self.string_end(i, c) {
                    Some(e) => {
                        end = e;
                        T::PlainString
                    }
                    None => {
                        let mut resume = i + 1;
                        while resume < n && !self.starts_token(resume) {
                            resume += 1;
                        }
                        return Err(self.lex_error(i, resume - i));
                    }
                },
                '%' => {
                    if line_start && self.is(end, '%') {
                        while end < n && !self.line_end_at(end) {
                            end += 1;
                        }
                        T::Comment
                    } else {
                        T::Percent
                    }
                }
                '-' if !self.is(end, '-') => T::Dash,
                '-' => {
                    while self.is(end, '-') {
                        end += 1;
                    }
                    let two = end - i == 2;
                    if two && self.is(end, '|') && self.is(end + 1, '>') {
                        end += 2;
                        T::Generalization
                    } else if self.is(end, '>') {
                        end += 1;
                        T::ForwardSolid
                    } else if two && self.is(end, 'o') {
                        end += 1;
                        T::ForwardCircle
                    } else if two && self.is(end, 'x') {
                        end += 1;
                        T::ForwardCross
                    } else {
                        T::MarkerlessSolid
                    }
                }
                '.' => {
                    if self.is(end, '.') && self.is(end + 1, '>') {
                        end += 2;
                        T::DependencyArrow
                    } else if self.matches(end, is_digit) {
                        while self.matches(end, is_digit) {
                            end += 1;
                        }
                        while self.matches(end, is_letter) {
                            end += 1;
                        }
                        T::Number
                    } else {
                        T::Dot
                    }
                }
                '<' if self.is(end, '<') => {
                    tokens.push(Token::new(T::StereotypeStart, i, i + 2));
                    i += 2;
                    if i >= n {
                        break 'tokens;
                    }
                    if self.is(i, '>') && self.is(i + 1, '>') {
                        end = i + 2;
                        T::StereotypeEnd
                    } else {
                        let mut close = i;
                        let mut found = false;
                        while let Some(ch) = self.char(close) {
                            if matches!(ch, '\n' | '\r') {
                                break;
                            }
                            if ch == '>' && self.is(close + 1, '>') {
                                found = true;
                                break;
                            }
                            close += 1;
                        }
                        if found && !js::trim(&self.slice(i, close)).is_empty() {
                            tokens.push(Token::new(T::StereotypeText, i, close));
                            i = close;
                            end = close + 2;
                            T::StereotypeEnd
                        } else {
                            while close < n && !self.line_end_at(close) {
                                close += 1;
                            }
                            if close == i {
                                while self.line_end_at(close) {
                                    close += 1;
                                }
                                return Err(self.lex_error(i, close - i));
                            }
                            end = close;
                            T::UnclosedStereotypeText
                        }
                    }
                }
                '<' if self.is(end, '-') && self.is(end + 1, '-') => {
                    end += 2;
                    while self.is(end, '-') {
                        end += 1;
                    }
                    T::BackwardSolid
                }
                ':' if self.is(end, ':') && self.is(end + 1, ':') => {
                    end += 2;
                    T::ClassSeparator
                }
                ':' => T::Colon,
                '@' if self.is(end, '{') => {
                    end += 1;
                    T::MetadataStart
                }
                '@' => T::At,
                '{' => T::Lbrace,
                '}' => T::Rbrace,
                '[' => T::Lbracket,
                ']' => T::Rbracket,
                '(' => T::Lparen,
                ')' => T::Rparen,
                ',' => T::Comma,
                '#' => {
                    while self.matches(end, is_hex) {
                        end += 1;
                    }
                    if end > i + 1 {
                        T::HashColor
                    } else {
                        T::CssPunctuation
                    }
                }
                '\\' if self.is(end, ',') => {
                    end += 1;
                    T::CssEscapedComma
                }
                '!'..='~' if is_css_punctuation(c) => T::CssPunctuation,
                '!'..='~' => T::LabelPunctuation,
                _ => {
                    while end < n
                        && !self.matches(end, |ch| matches!(ch, ' '..='~' | '\t' | '\n' | '\r'))
                    {
                        end += 1;
                    }
                    T::LabelSymbol
                }
            };

            tokens.push(Token::new(kind, i, end));
            i = end;
            line_start = fresh;
        }

        tokens.extend([Token::new(T::Eof, n, n); 3]);
        Ok(tokens)
    }
}
