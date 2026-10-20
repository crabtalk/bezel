//! Tokens, and the cursor a parser reads them with.

use std::borrow::Cow;

use crate::{detect::Kind, error::Error};

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
