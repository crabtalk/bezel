//! Pele's `diagrams/timeline/lexer.ts`.

use crate::mermaid::{
    diagrams::common::scan::{Tokens, accessibility, keyword_at, line_end, skip_trivia},
    util::chars::{code_at, is_space, is_word, width_at},
};

pub const END: i32 = 0;
pub const EOF: i32 = 1;
pub const INVALID: i32 = 2;
pub const NEWLINE: i32 = 3;
pub const TIMELINE: i32 = 4;
pub const TIMELINE_LR: i32 = 5;
pub const TIMELINE_TD: i32 = 6;
pub const TITLE: i32 = 7;
pub const ACC_TITLE: i32 = 8;
pub const ACC_TITLE_VALUE: i32 = 9;
pub const ACC_DESCR: i32 = 10;
pub const ACC_DESCR_VALUE: i32 = 11;
pub const ACC_DESCR_MULTILINE_VALUE: i32 = 12;
pub const SECTION: i32 = 13;
pub const PERIOD: i32 = 14;
pub const EVENT: i32 = 15;

pub const TOKEN_NAMES: [&str; 16] = [
    "$end",
    "EOF",
    "INVALID",
    "NEWLINE",
    "timeline",
    "timeline_lr",
    "timeline_td",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "section",
    "period",
    "event",
];

/// Pushes the token from `p` to `e`, and answers `e`, where the next one
/// starts.
fn emit(src: &str, out: &mut Tokens, p: usize, kind: i32, e: usize) -> usize {
    out.types.push(kind);
    out.texts
        .push(src.get(p..e.min(src.len())).unwrap_or("").to_string());
    out.starts.push(p);
    e
}

pub fn tokenize(src: &str) -> Tokens {
    let n = src.len();
    let bytes = src.as_bytes();
    let mut out = Tokens::default();
    let mut p = 0;

    loop {
        p = skip_trivia(src, p);
        if p >= n {
            break;
        }
        let c = code_at(src, p);
        let mut e = p + 1;
        if c == 10 {
            while code_at(src, e) == 10 {
                e += 1;
            }
            p = emit(src, &mut out, p, NEWLINE, e);
            continue;
        }
        if keyword_at(src, p, "timeline") {
            e = p + 8;
            while matches!(code_at(src, e), 32 | 9) {
                e += 1;
            }
            let lr = keyword_at(src, e, "lr");
            if e > p + 8 && (lr || keyword_at(src, e, "td")) && !is_word(code_at(src, e + 2)) {
                p = emit(
                    src,
                    &mut out,
                    p,
                    if lr { TIMELINE_LR } else { TIMELINE_TD },
                    e + 2,
                );
                continue;
            }
            if !is_word(code_at(src, p + 8)) {
                p = emit(src, &mut out, p, TIMELINE, p + 8);
                continue;
            }
        }
        // `\s` after a keyword also takes a line break, so `title` alone on
        // a line takes the next line as its text.
        if keyword_at(src, p, "title") && is_space(code_at(src, p + 5)) {
            let from = p + 5 + width_at(src, p + 5);
            if from < n && code_at(src, from) != 10 {
                p = emit(src, &mut out, p, TITLE, line_end(src, from));
                continue;
            }
        }
        e = accessibility(src, p, &mut out, ACC_TITLE);
        if e != p {
            p = e;
            continue;
        }
        if keyword_at(src, p, "section") && is_space(code_at(src, p + 7)) {
            let from = p + 7 + width_at(src, p + 7);
            e = from;
            while e < n && bytes[e] != b':' && bytes[e] != b'\n' {
                e += 1;
            }
            if e > from {
                p = emit(src, &mut out, p, SECTION, e);
                continue;
            }
        }
        if c != 58 {
            e = p + width_at(src, p);
            while e < n && !matches!(bytes[e], b'#' | b':' | b'\n') {
                e += 1;
            }
            p = emit(src, &mut out, p, PERIOD, e);
            continue;
        }
        // An event runs to the end of the line or to a colon that is followed
        // by whitespace.
        if is_space(code_at(src, p + 1)) {
            let from = p + 1 + width_at(src, p + 1);
            e = from;
            while e < n {
                let k = code_at(src, e);
                if k == 10 || (k == 58 && is_space(code_at(src, e + 1))) {
                    break;
                }
                e += width_at(src, e);
            }
            if e > from {
                p = emit(src, &mut out, p, EVENT, e);
                continue;
            }
        }
        p = emit(src, &mut out, p, INVALID, p + 1);
    }

    // Jison marks its lexer done when a rule runs on empty input, so an
    // accessibility value that matches nothing at the very end of the text is
    // followed by no EOF token.
    if out.starts.last().is_none_or(|&last| last < n) {
        p = emit(src, &mut out, p, EOF, n);
    }
    emit(src, &mut out, p, END, n);
    out
}
