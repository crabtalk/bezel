//! Pele's `diagrams/sankey/lexer.ts`.

use crate::{
    token::{Token, grammar},
    util::chars::{code_at, is_word},
};

grammar! {
    /// The sankey lexer's token kinds.
    pub enum T {
        End = "$end",
        /// Never lexed: every text matches some rule.
        Error = "INVALID",
        /// Not a Jison token: the reference lexer would return the same empty
        /// token forever from here.
        Stuck = "STUCK",
        Sankey = "SANKEY",
        Newline = "NEWLINE",
        Comma = "COMMA",
        Dquote = "DQUOTE",
        NonEscapedText = "NON_ESCAPED_TEXT",
        EscapedText = "ESCAPED_TEXT",
        Eof = "EOF",
    }

    fn got(self, _text, previous) {
        match self {
            Self::Stuck => previous.map_or("undefined", Self::name),
            _ => self.name(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Csv,
    Escaped,
}

/// RFC 4180 TEXTDATA: printable ASCII except the double quote and the comma.
fn is_text_data(c: u32) -> bool {
    (32..=126).contains(&c) && c != 34 && c != 44
}

/// The end of `word` at `p`, matched as `slice(p, p + word.length)
/// .toLowerCase() === word` with no word character after it.
fn keyword(src: &str, p: usize, word: &str) -> Option<usize> {
    let mut units = 0;
    let mut end = p;
    for c in src[p..].chars() {
        if units >= word.len() {
            break;
        }
        units += c.len_utf16();
        end += c.len_utf8();
    }
    let lower = src[p..end].to_lowercase();
    (lower == word && !is_word(code_at(src, end))).then_some(end)
}

pub fn tokenize<'s>(src: &'s str) -> Vec<Token<'s, T>> {
    let n = src.len();
    let mut tokens = Vec::new();
    let mut stack = Vec::new();
    let mut state = S::Initial;
    let mut p = 0;

    // Answers where the lexer goes on from.
    let emit = |tokens: &mut Vec<Token<'s, T>>, kind: T, s: usize, e: usize| -> usize {
        tokens.push(Token::new(kind, &src[s..e], s));
        e
    };

    while p < n {
        let c = code_at(src, p);
        let start = p;
        let mut q = p;
        if state == S::Escaped {
            if c == 34 && code_at(src, p + 1) != 34 {
                if let Some(previous) = stack.pop() {
                    state = previous;
                }
                p = emit(&mut tokens, T::Dquote, p, p + 1);
                continue;
            }
            while q < n {
                let d = code_at(src, q);
                if is_text_data(d) || d == 44 || d == 13 || d == 10 {
                    q += 1;
                } else if d == 34 && code_at(src, q + 1) == 34 {
                    q += 2;
                } else {
                    break;
                }
            }
            p = emit(&mut tokens, T::EscapedText, p, q);
        } else {
            if state == S::Initial && (c | 32) == 115 {
                let end = keyword(src, p, "sankey-beta").or_else(|| keyword(src, p, "sankey"));
                if let Some(end) = end {
                    stack.push(state);
                    state = S::Csv;
                    p = emit(&mut tokens, T::Sankey, p, end);
                    continue;
                }
            }
            if c == 10 || (c == 13 && code_at(src, p + 1) == 10) {
                let end = if c == 10 { p + 1 } else { p + 2 };
                p = emit(&mut tokens, T::Newline, p, end);
                continue;
            }
            if c == 44 {
                p = emit(&mut tokens, T::Comma, p, p + 1);
                continue;
            }
            if c == 34 {
                stack.push(state);
                state = S::Escaped;
                p = emit(&mut tokens, T::Dquote, p, p + 1);
                continue;
            }
            while q < n && is_text_data(code_at(src, q)) {
                q += 1;
            }
            p = emit(&mut tokens, T::NonEscapedText, p, q);
        }
        // A character outside the CSV alphabet matches as empty text and is
        // never consumed.
        if q == start {
            emit(&mut tokens, T::Stuck, p, p);
            return tokens;
        }
    }

    let last = if state == S::Escaped {
        T::EscapedText
    } else {
        T::Eof
    };
    emit(&mut tokens, last, n, n);
    emit(&mut tokens, T::End, n, n);
    tokens
}
