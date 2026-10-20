//! Tokens, and the cursor a parser reads them with.

use std::borrow::Cow;

use crate::{
    detect::Kind,
    error::Error,
    util::chars::{
        char_at, code_at, is_line_end, is_space, is_word, starts_with_caseless, width_at,
    },
};

/// A grammar's token kinds. Declare one with [`grammar!`].
pub trait Grammar: Copy + Eq + 'static {
    /// Past the last token.
    const END: Self;
    /// Text the lexer could not read. A parser that meets it reports a
    /// lexical error, or the token's text as the message when it has any.
    const ERROR: Self;

    /// The name Jison's "Expecting" messages give the kind.
    fn name(self) -> &'static str;

    /// What an error says it got, for a token of this kind with `text` after
    /// a token of kind `previous`.
    fn got(self, _text: &str, _previous: Option<Self>) -> &str {
        self.name()
    }
}

/// Declares a token enum and its [`Grammar`]: each variant with the name
/// Jison's messages give it, then optionally a body for [`Grammar::got`].
/// `End` and `Error` must be among the variants.
macro_rules! grammar {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$variant_meta:meta])* $variant:ident = $text:literal,)*
    }
    $(fn got($self:ident, $got_text:ident, $got_previous:ident) $got:block)?) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        $vis enum $name {
            $($(#[$variant_meta])* $variant,)*
        }

        impl $crate::token::Grammar for $name {
            const END: Self = Self::End;
            const ERROR: Self = Self::Error;

            fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)*
                }
            }

            $(fn got($self, $got_text: &str, $got_previous: Option<Self>) -> &str $got)?
        }
    };
}
pub(crate) use grammar;

#[derive(Clone, Debug)]
pub struct Token<'s, K> {
    pub kind: K,
    /// The source text, or what the lexer made of it.
    pub text: Cow<'s, str>,
    /// Byte offset into the source.
    pub start: usize,
}

impl<'s, K> Token<'s, K> {
    pub fn new(kind: K, text: impl Into<Cow<'s, str>>, start: usize) -> Self {
        Self {
            kind,
            text: text.into(),
            start,
        }
    }
}

impl<K: Grammar> Token<'_, K> {
    /// Jison's error in `diagram` at this token, after a token of kind
    /// `previous`.
    pub fn error(&self, diagram: Kind, src: &str, expected: &str, previous: Option<K>) -> Error {
        if self.kind == K::ERROR {
            if self.text.is_empty() {
                return Error::at(diagram, src, self.start, "", true);
            }
            return Error::syntax_at(diagram, src, self.start, self.text.clone());
        }
        let got = self.kind.got(&self.text, previous);
        Error::at(
            diagram,
            src,
            self.start,
            &format!("Expecting {expected}, got '{got}'"),
            false,
        )
    }
}

/// A hand-written lexer's place in `src`, the tokens it has made, and its
/// stack of Jison start conditions `S`.
pub struct Cursor<'s, K, S> {
    pub src: &'s str,
    /// Byte offset of the next character to read.
    pub p: usize,
    pub tokens: Vec<Token<'s, K>>,
    pub state: S,
    stack: Vec<S>,
}

impl<'s, K, S: Copy> Cursor<'s, K, S> {
    pub fn new(src: &'s str, state: S) -> Self {
        Self {
            src,
            p: 0,
            tokens: Vec::new(),
            state,
            stack: Vec::new(),
        }
    }

    /// The character at byte `at`, as `charCodeAt` reads it.
    pub fn code(&self, at: usize) -> u32 {
        code_at(self.src, at)
    }

    /// The character at byte `at`, or `None` past the end.
    pub fn char_at(&self, at: usize) -> Option<char> {
        char_at(self.src, at)
    }

    /// A token of `src[start..end]`; reading goes on from `end`.
    pub fn emit(&mut self, kind: K, start: usize, end: usize) {
        self.tokens
            .push(Token::new(kind, &self.src[start..end], start));
        self.p = end;
    }

    /// A token from the cursor to `end`.
    pub fn emit_to(&mut self, kind: K, end: usize) {
        self.emit(kind, self.p, end);
    }

    /// A token at `start` whose text is `text`; reading goes on from `end`.
    pub fn emit_text(&mut self, kind: K, text: impl Into<Cow<'s, str>>, start: usize, end: usize) {
        self.tokens.push(Token::new(kind, text, start));
        self.p = end;
    }

    /// Enters start condition `state`, as Jison's `begin`.
    pub fn push(&mut self, state: S) {
        self.stack.push(self.state);
        self.state = state;
    }

    /// Leaves the current start condition, as Jison's `popState`. The first
    /// one is never left.
    pub fn pop(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    pub fn starts_with(&self, word: &str, at: usize) -> bool {
        self.src
            .as_bytes()
            .get(at..)
            .is_some_and(|rest| rest.starts_with(word.as_bytes()))
    }

    /// Whether `word`, written in lower case, is at `at`, ASCII letters
    /// matching in either case.
    pub fn starts_with_caseless(&self, word: &str, at: usize) -> bool {
        starts_with_caseless(self.src, at, word)
    }

    /// Whether `word`, written in lower case, is at `at` in any case, with no
    /// word character after it.
    pub fn keyword_caseless(&self, word: &str, at: usize) -> bool {
        self.starts_with_caseless(word, at) && !is_word(self.code(at + word.len()))
    }

    /// The end of `word`, in any case, at the cursor, then one whitespace
    /// character and a nonempty run of characters other than `stops`.
    pub fn keyword_line(&self, word: &str, stops: &[char]) -> Option<usize> {
        let after = self.p + word.len();
        if !self.starts_with_caseless(word, self.p) || !is_space(self.code(after)) {
            return None;
        }
        let from = after + width_at(self.src, after);
        let end = self.until(from, |c| stops.contains(&c));
        (end > from).then_some(end)
    }

    /// Whether `word` is at the cursor with no word character after it.
    pub fn keyword(&self, word: &str) -> bool {
        self.starts_with(word, self.p) && !is_word(self.code(self.p + word.len()))
    }

    /// The end of the run of whitespace at `from`.
    pub fn space_end(&self, mut from: usize) -> usize {
        while from < self.src.len() && is_space(self.code(from)) {
            from += width_at(self.src, from);
        }
        from
    }

    /// The end of the run of word characters at `from`.
    pub fn word_end(&self, from: usize) -> usize {
        from + self
            .src
            .get(from..)
            .map_or(0, |rest| rest.bytes().take_while(|&b| is_word(b)).count())
    }

    /// The end of the run of characters at `from` that `keep` accepts.
    pub fn run_end(&self, from: usize, keep: impl Fn(char) -> bool) -> usize {
        self.until(from, |c| !keep(c))
    }

    /// The first character at or after `from` that `stop` accepts, or the
    /// end of the source.
    pub fn until(&self, from: usize, stop: impl FnMut(char) -> bool) -> usize {
        self.src
            .get(from..)
            .and_then(|rest| rest.find(stop))
            .map_or(self.src.len(), |at| from + at)
    }

    /// The next line feed at or after `from`, or the end of the source.
    pub fn line_end(&self, from: usize) -> usize {
        self.until(from, |c| c == '\n')
    }

    /// The byte after the character at `at`.
    pub fn after(&self, at: usize) -> usize {
        at + width_at(self.src, at)
    }

    /// The next `ch` at or after `from`, or the end of the source.
    pub fn find(&self, ch: char, from: usize) -> usize {
        self.until(from, |c| c == ch)
    }

    /// The next line terminator, what `.` does not match, at or after `from`,
    /// or the end of the source.
    pub fn terminator_end(&self, from: usize) -> usize {
        self.until(from, is_line_end)
    }
}

/// `kinds` as Jison's "Expecting" messages list them.
pub fn expecting<K: Grammar>(kinds: impl IntoIterator<Item = K>) -> String {
    let names: Vec<String> = kinds
        .into_iter()
        .map(|kind| format!("'{}'", kind.name()))
        .collect();
    names.join(", ")
}

/// A position in a token list, and Jison's error messages for the token there.
pub struct Stream<'t, 's, K> {
    diagram: Kind,
    src: &'s str,
    tokens: &'t [Token<'s, K>],
    at: usize,
}

impl<'t, 's, K: Grammar> Stream<'t, 's, K> {
    /// A stream whose errors are reported against `diagram`.
    pub fn new(diagram: Kind, src: &'s str, tokens: &'t [Token<'s, K>]) -> Self {
        Self {
            diagram,
            src,
            tokens,
            at: 0,
        }
    }

    /// The index of the current token.
    pub fn index(&self) -> usize {
        self.at
    }

    pub fn kind(&self) -> K {
        self.peek(0)
    }

    /// The kind of the token `ahead` past the current one.
    pub fn peek(&self, ahead: usize) -> K {
        self.tokens
            .get(self.at + ahead)
            .map_or(K::END, |token| token.kind)
    }

    pub fn is(&self, kind: K) -> bool {
        self.kind() == kind
    }

    pub fn is_any(&self, kinds: &[K]) -> bool {
        kinds.contains(&self.kind())
    }

    /// The current token's text.
    pub fn text(&self) -> &'t str {
        self.tokens.get(self.at).map_or("", |token| &token.text)
    }

    /// Steps past the current token.
    pub fn bump(&mut self) {
        self.at += 1;
    }

    /// Steps past the current token if it is `kind`.
    pub fn eat(&mut self, kind: K) -> bool {
        let found = self.is(kind);
        if found {
            self.at += 1;
        }
        found
    }

    /// Steps past every token of `kind` from here.
    pub fn eat_all(&mut self, kind: K) {
        while self.eat(kind) {}
    }

    /// The current token's text, stepping past it.
    pub fn take(&mut self) -> &'t str {
        let text = self.text();
        self.at += 1;
        text
    }

    /// The current token's text if it is `kind`; otherwise the error naming
    /// `kind`, then `also`.
    pub fn expect(&mut self, kind: K, also: &str) -> Result<&'t str, Error> {
        if !self.is(kind) {
            let name = kind.name();
            return Err(self.fail(&if also.is_empty() {
                format!("'{name}'")
            } else {
                format!("'{name}', {also}")
            }));
        }
        Ok(self.take())
    }

    /// The texts of the run of tokens in `kinds` from here, joined; an error
    /// naming `expected` when the run is empty.
    pub fn run_of(&mut self, kinds: &[K], expected: &str) -> Result<String, Error> {
        if !self.is_any(kinds) {
            return Err(self.fail(expected));
        }
        let mut run = String::new();
        while self.is_any(kinds) {
            run += self.take();
        }
        Ok(run)
    }

    /// Jison's error at the current token.
    pub fn fail(&self, expected: &str) -> Error {
        let previous = self.previous();
        match self.tokens.get(self.at) {
            Some(token) => token.error(self.diagram, self.src, expected, previous),
            None => Token::new(K::END, "", self.src.len()).error(
                self.diagram,
                self.src,
                expected,
                previous,
            ),
        }
    }

    /// [`Stream::fail`] expecting each of `kinds`.
    pub fn fail_expecting(&self, kinds: impl IntoIterator<Item = K>) -> Error {
        self.fail(&expecting(kinds))
    }

    /// The kind of the token before the current one.
    pub fn previous(&self) -> Option<K> {
        self.at
            .checked_sub(1)
            .and_then(|at| self.tokens.get(at))
            .map(|token| token.kind)
    }
}

/// A statement token's text past its keyword, `keyword` bytes long, and the
/// character after it, as `substring(keyword + 1)` reads it.
pub fn after_keyword(text: &str, keyword: usize) -> &str {
    text.get(keyword + width_at(text, keyword)..).unwrap_or("")
}
