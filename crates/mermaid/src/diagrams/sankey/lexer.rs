//! Pele's `diagrams/sankey/lexer.ts`.

use crate::util::chars::{code_at, is_word};

pub const END: u8 = 0;
/// Not a Jison token: the reference lexer would return the same empty token
/// forever from here.
pub const STUCK: u8 = 1;
pub const SANKEY: u8 = 2;
pub const NEWLINE: u8 = 3;
pub const COMMA: u8 = 4;
pub const DQUOTE: u8 = 5;
pub const NON_ESCAPED_TEXT: u8 = 6;
pub const ESCAPED_TEXT: u8 = 7;
pub const EOF: u8 = 8;

pub const TOKEN_NAMES: [&str; 9] = [
    "$end",
    "STUCK",
    "SANKEY",
    "NEWLINE",
    "COMMA",
    "DQUOTE",
    "NON_ESCAPED_TEXT",
    "ESCAPED_TEXT",
    "EOF",
];

const INITIAL: u8 = 0;
const CSV: u8 = 1;
const ESCAPED: u8 = 2;

pub struct Tokens<'s> {
    pub types: Vec<u8>,
    pub texts: Vec<&'s str>,
    pub starts: Vec<usize>,
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

pub fn tokenize<'s>(src: &'s str) -> Tokens<'s> {
    let n = src.len();
    let mut tokens = Tokens {
        types: Vec::new(),
        texts: Vec::new(),
        starts: Vec::new(),
    };
    let mut stack: Vec<u8> = Vec::new();
    let mut state = INITIAL;
    let mut p = 0;

    // Answers where the lexer goes on from.
    let emit = |tokens: &mut Tokens<'s>, kind: u8, s: usize, e: usize| -> usize {
        tokens.types.push(kind);
        tokens.texts.push(&src[s..e]);
        tokens.starts.push(s);
        e
    };

    while p < n {
        let c = code_at(src, p);
        let start = p;
        let mut q = p;
        if state == ESCAPED {
            if c == 34 && code_at(src, p + 1) != 34 {
                if let Some(previous) = stack.pop() {
                    state = previous;
                }
                p = emit(&mut tokens, DQUOTE, p, p + 1);
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
            p = emit(&mut tokens, ESCAPED_TEXT, p, q);
        } else {
            if state == INITIAL && (c | 32) == 115 {
                let end = keyword(src, p, "sankey-beta").or_else(|| keyword(src, p, "sankey"));
                if let Some(end) = end {
                    stack.push(state);
                    state = CSV;
                    p = emit(&mut tokens, SANKEY, p, end);
                    continue;
                }
            }
            if c == 10 || (c == 13 && code_at(src, p + 1) == 10) {
                let end = if c == 10 { p + 1 } else { p + 2 };
                p = emit(&mut tokens, NEWLINE, p, end);
                continue;
            }
            if c == 44 {
                p = emit(&mut tokens, COMMA, p, p + 1);
                continue;
            }
            if c == 34 {
                stack.push(state);
                state = ESCAPED;
                p = emit(&mut tokens, DQUOTE, p, p + 1);
                continue;
            }
            while q < n && is_text_data(code_at(src, q)) {
                q += 1;
            }
            p = emit(&mut tokens, NON_ESCAPED_TEXT, p, q);
        }
        // A character outside the CSV alphabet matches as empty text and is
        // never consumed.
        if q == start {
            emit(&mut tokens, STUCK, p, p);
            return tokens;
        }
    }

    let last = if state == ESCAPED { ESCAPED_TEXT } else { EOF };
    emit(&mut tokens, last, n, n);
    emit(&mut tokens, END, n, n);
    tokens
}
