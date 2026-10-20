//! Why a diagram did not draw.

use std::fmt;

use serde::Serialize;

use super::{detect::Kind, js};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    /// No diagram type the source names, or one this build has not got.
    UnsupportedDiagram,
    Syntax,
    /// Source that parses but describes something that cannot be.
    Semantic,
    /// Past [`Options::limit`], [`Options::output_limit`] or
    /// [`Options::max_edges`], or past a limit a diagram type keeps on its
    /// own, such as nesting depth.
    ///
    /// [`Options::limit`]: crate::Options::limit
    /// [`Options::output_limit`]: crate::Options::output_limit
    /// [`Options::max_edges`]: crate::Options::max_edges
    Limit,
}

/// Pele's `PeleError`: the message is Pele's, word for word. `line` and
/// `column` count from 1, `column` in UTF-16 units; `0` where there is no
/// place in the source to point at.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Error {
    pub message: String,
    pub code: Code,
    #[serde(rename = "type")]
    pub kind: Option<Kind>,
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

impl Error {
    pub fn new(message: impl Into<String>, code: Code) -> Self {
        Self {
            message: message.into(),
            code,
            kind: None,
            line: 0,
            column: 0,
            snippet: String::new(),
        }
    }

    pub fn of(mut self, kind: Kind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn syntax(message: impl Into<String>) -> Self {
        Self::new(message, Code::Syntax)
    }

    pub fn semantic(message: impl Into<String>) -> Self {
        Self::new(message, Code::Semantic)
    }

    pub fn limit(message: impl Into<String>) -> Self {
        Self::new(message, Code::Limit)
    }

    /// A syntax error saying `message` at byte `offset` of `src`.
    pub fn syntax_at(kind: Kind, src: &str, offset: usize, message: impl Into<String>) -> Self {
        let place = Place::locate(src, offset);
        Self {
            message: message.into(),
            code: Code::Syntax,
            kind: Some(kind),
            line: place.line,
            column: place.column,
            snippet: place.snippet,
        }
    }

    /// A parse error at byte `offset` of `src`, in Jison's words: `lexical`
    /// for text no token matches.
    pub fn at(kind: Kind, src: &str, offset: usize, detail: &str, lexical: bool) -> Self {
        let place = Place::locate(src, offset);
        let message = if lexical {
            format!(
                "Lexical error on line {}. Unrecognized text.\n{}",
                place.line, place.snippet
            )
        } else {
            format!(
                "Parse error on line {}:\n{}\n{detail}",
                place.line, place.snippet
            )
        };
        Self {
            message,
            code: Code::Syntax,
            kind: Some(kind),
            line: place.line,
            column: place.column,
            snippet: place.snippet,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// A line and column in the source, and the line with a caret under the
/// column.
pub struct Place {
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

impl Place {
    pub fn locate(src: &str, offset: usize) -> Self {
        let offset = offset.min(src.len());
        let mut line = 1;
        let mut line_start = 0;
        for (i, b) in src.as_bytes()[..offset].iter().enumerate() {
            if *b == b'\n' {
                line += 1;
                line_start = i + 1;
            }
        }
        let line_end = src[offset..].find('\n').map_or(src.len(), |at| offset + at);
        let column = js::len(&src[line_start..offset]) + 1;
        let text = &src[line_start..line_end];
        Self {
            line,
            column,
            snippet: format!("{text}\n{}^", "-".repeat(column - 1)),
        }
    }
}
