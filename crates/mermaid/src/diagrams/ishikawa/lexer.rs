//! Pele's `diagrams/ishikawa/lexer.ts`.

use crate::{
    token::{Cursor, Token, grammar},
    util::chars::is_line_end,
};

grammar! {
    /// The Ishikawa lexer's token kinds.
    pub enum T {
        End = "$end",
        /// Never lexed: every text matches some rule.
        Error = "INVALID",
        Eof = "EOF",
        Spaceline = "SPACELINE",
        Nl = "NL",
        Ishikawa = "ISHIKAWA",
        Spacelist = "SPACELIST",
        Text = "TEXT",
    }
}

/// Produces the tokens of Mermaid's Jison lexer for Ishikawa diagrams, where
/// the first rule that matches wins: white space up to a comment or up to the
/// last line break in it is a blank line, a single line break before text is
/// a new line, other white space is indentation, and the rest of a line is
/// text.
pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let mut cur = Cursor::new(src, ());
    let n = src.len();
    while cur.p < n {
        let i = cur.p;
        let j = cur.space_end(i);
        let last_break = src[i..j].rfind('\n').map(|at| i + at);
        let (kind, end) = if cur.starts_with("%%", j) {
            (T::Spaceline, cur.run_end(j + 2, |c| !is_line_end(c)))
        } else if j == i {
            // `ishikawa-beta`, or failing that `ishikawa`, either one ending
            // at a word boundary.
            if cur.keyword_caseless("ishikawa-beta", i) {
                (T::Ishikawa, i + 13)
            } else if cur.keyword_caseless("ishikawa", i) {
                (T::Ishikawa, i + 8)
            } else {
                (T::Text, cur.line_end(i))
            }
        } else {
            match last_break {
                Some(at) if at > i => (T::Spaceline, at + 1),
                Some(_) => (T::Nl, i + 1),
                None => (T::Spacelist, j),
            }
        };
        cur.emit(kind, i, end);
    }
    cur.emit(T::Eof, n, n);
    cur.emit(T::End, n, n);
    cur.tokens
}
