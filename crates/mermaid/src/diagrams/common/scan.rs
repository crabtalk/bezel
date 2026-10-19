//! Lexer rules that Mermaid's journey and timeline grammars have in common,
//! ported from the compiled Jison rules: comments, whitespace, and the
//! `accTitle` and `accDescr` statements. Pele's `diagrams/common/scan.ts`.

use crate::util::chars::{code_at, is_space, width_at};

#[derive(Default)]
pub struct Tokens {
    pub types: Vec<i32>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
}

pub fn line_end(src: &str, from: usize) -> usize {
    src.get(from..)
        .and_then(|rest| rest.find('\n'))
        .map_or(src.len(), |at| from + at)
}

/// Whether `word`, lowercase ASCII letters, is at `p`; the grammars are
/// case-insensitive.
pub fn keyword_at(src: &str, p: usize, word: &str) -> bool {
    let bytes = src.as_bytes();
    word.bytes()
        .enumerate()
        .all(|(i, w)| bytes.get(p + i).is_some_and(|&c| (c | 32) == w))
}

/// Skips comments and whitespace. Stops at a line break that starts a token,
/// which is a NEWLINE. Jison compiles `\%%` to a single `%`, so one percent
/// sign starts a comment, and the second comment rule takes any character
/// before `%%` with it, a line break included.
pub fn skip_trivia(src: &str, from: usize) -> usize {
    let n = src.len();
    let mut p = from;
    while p < n {
        let c = code_at(src, p);
        if c == 37 && code_at(src, p + 1) != 123 {
            p = line_end(src, p);
        } else if c != 125 && {
            let w = width_at(src, p);
            code_at(src, p + w) == 37 && code_at(src, p + w + 1) == 37
        } {
            p = line_end(src, p + width_at(src, p) + 2);
        } else if c == 10 {
            return p;
        } else if is_space(c) {
            loop {
                p += width_at(src, p);
                if !(p < n && is_space(code_at(src, p))) {
                    break;
                }
            }
        } else if c == 35 {
            p = line_end(src, p);
        } else {
            return p;
        }
    }
    p
}

fn skip_space(src: &str, from: usize) -> usize {
    let mut p = from;
    while is_space(code_at(src, p)) {
        p += width_at(src, p);
    }
    p
}

/// Lexes an `accTitle` or `accDescr` statement at `p` and answers where it
/// ends, or `p` if there is none. The token types acc_title,
/// acc_title_value, acc_descr, acc_descr_value and acc_descr_multiline_value
/// are numbered consecutively from `first`.
pub fn accessibility(src: &str, p: usize, out: &mut Tokens, first: i32) -> usize {
    let descr = keyword_at(src, p, "accdescr");
    if !descr && !keyword_at(src, p, "acctitle") {
        return p;
    }
    let n = src.len();
    let mut q = skip_space(src, p + 8);
    let c = code_at(src, q);
    let mut emit = |kind: i32, s: usize, e: usize| {
        out.types.push(kind);
        out.texts.push(src[s..e].to_string());
        out.starts.push(s);
    };
    if c == 58 {
        q = skip_space(src, q + 1);
        let end = line_end(src, q);
        emit(if descr { first + 2 } else { first }, p, q);
        emit(if descr { first + 3 } else { first + 1 }, q, end);
        return end;
    }
    if !descr || c != 123 {
        return p;
    }
    q = skip_space(src, q + 1);
    while code_at(src, q) != 125 {
        let end = src[q..].find('}').map_or(n, |at| q + at);
        emit(first + 4, q, end);
        if q == n {
            return n;
        }
        q = end;
    }
    q + 1
}
