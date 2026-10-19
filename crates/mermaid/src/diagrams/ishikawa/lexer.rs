//! Pele's `diagrams/ishikawa/lexer.ts`.

use crate::util::chars::{code_at, is_line_end, is_space, is_word, width_at};

pub const END: i32 = 0;
pub const EOF: i32 = 1;
pub const SPACELINE: i32 = 2;
pub const NL: i32 = 3;
pub const ISHIKAWA: i32 = 4;
pub const SPACELIST: i32 = 5;
pub const TEXT: i32 = 6;

pub const TOKEN_NAMES: [&str; 7] = [
    "$end",
    "EOF",
    "SPACELINE",
    "NL",
    "ISHIKAWA",
    "SPACELIST",
    "TEXT",
];

const WORD: &[u8; 13] = b"ishikawa-beta";

pub struct Tokens {
    pub types: Vec<i32>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
}

fn keyword(src: &str, at: usize) -> usize {
    let mut k = 0;
    while k < 13 {
        let c = code_at(src, at + k);
        let w = u32::from(WORD[k]);
        if c != w && !((65..=90).contains(&c) && c + 32 == w) {
            break;
        }
        k += 1;
    }
    // `ishikawa-beta`, or failing that `ishikawa`, either one ending at a
    // word boundary.
    if k == 13 && !is_word(code_at(src, at + 13)) {
        return 13;
    }
    if k >= 8 && !is_word(code_at(src, at + 8)) {
        8
    } else {
        0
    }
}

/// Produces the tokens of Mermaid's Jison lexer for Ishikawa diagrams, where
/// the first rule that matches wins: white space up to a comment or up to the
/// last line break in it is a blank line, a single line break before text is
/// a new line, other white space is indentation, and the rest of a line is
/// text.
pub fn tokenize(src: &str) -> Tokens {
    let mut types = Vec::new();
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let n = src.len();
    let mut i = 0;
    while i < n {
        let mut j = i;
        let mut last_break = None;
        while j < n && is_space(code_at(src, j)) {
            if code_at(src, j) == 10 {
                last_break = Some(j);
            }
            j += width_at(src, j);
        }
        let kind;
        let end;
        if code_at(src, j) == 37 && code_at(src, j + 1) == 37 {
            kind = SPACELINE;
            let mut e = j + 2;
            while e < n && !is_line_end(code_at(src, e)) {
                e += width_at(src, e);
            }
            end = e;
        } else if j == i {
            let length = keyword(src, i);
            if length > 0 {
                kind = ISHIKAWA;
                end = i + length;
            } else {
                kind = TEXT;
                end = src[i..].find('\n').map_or(n, |at| i + at);
            }
        } else if last_break.is_some_and(|at| at > i) {
            kind = SPACELINE;
            end = last_break.unwrap_or(i) + 1;
        } else if last_break == Some(i) {
            kind = NL;
            end = i + 1;
        } else {
            kind = SPACELIST;
            end = j;
        }
        types.push(kind);
        starts.push(i);
        ends.push(end);
        i = end;
    }
    types.extend([EOF, END]);
    starts.extend([n, n]);
    ends.extend([n, n]);
    Tokens {
        types,
        starts,
        ends,
    }
}
