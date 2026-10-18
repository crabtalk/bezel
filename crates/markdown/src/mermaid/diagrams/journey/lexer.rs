//! Pele's `diagrams/journey/lexer.ts`.

use crate::mermaid::{
    diagrams::common::scan::{Tokens, accessibility, keyword_at, skip_trivia},
    util::chars::{code_at, is_space, is_word, width_at},
};

pub const END: i32 = 0;
pub const EOF: i32 = 1;
pub const INVALID: i32 = 2;
pub const NEWLINE: i32 = 3;
pub const JOURNEY: i32 = 4;
pub const TITLE: i32 = 5;
pub const ACC_TITLE: i32 = 6;
pub const ACC_TITLE_VALUE: i32 = 7;
pub const ACC_DESCR: i32 = 8;
pub const ACC_DESCR_VALUE: i32 = 9;
pub const ACC_DESCR_MULTILINE_VALUE: i32 = 10;
pub const SECTION: i32 = 11;
pub const TASK_NAME: i32 = 12;
pub const TASK_DATA: i32 = 13;
pub const COLON: i32 = 14;

pub const TOKEN_NAMES: [&str; 15] = [
    "$end",
    "EOF",
    "INVALID",
    "NEWLINE",
    "journey",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "section",
    "taskName",
    "taskData",
    ":",
];

/// `#`, a line break, and `;` end every kind of text; a colon also ends a
/// task name or a section.
fn text_end(src: &str, from: usize, colon: bool) -> usize {
    let bytes = src.as_bytes();
    let mut e = from;
    while e < bytes.len() {
        let c = bytes[e];
        if c == b'#' || c == b'\n' || c == b';' || (colon && c == b':') {
            break;
        }
        e += 1;
    }
    e
}

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
        if keyword_at(src, p, "journey") && !is_word(code_at(src, p + 7)) {
            p = emit(src, &mut out, p, JOURNEY, p + 7);
            continue;
        }
        // `\s` after a keyword also takes a line break, so `title` alone on
        // a line takes the next line as its text.
        if keyword_at(src, p, "title") && is_space(code_at(src, p + 5)) {
            let from = p + 5 + width_at(src, p + 5);
            e = text_end(src, from, false);
            if e > from {
                p = emit(src, &mut out, p, TITLE, e);
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
            e = text_end(src, from, true);
            if e > from {
                p = emit(src, &mut out, p, SECTION, e);
                continue;
            }
        }
        if c == 58 {
            e = text_end(src, p + 1, false);
            if e > p + 1 {
                p = emit(src, &mut out, p, TASK_DATA, e);
            } else {
                p = emit(src, &mut out, p, COLON, p + 1);
            }
        } else if c == 59 {
            p = emit(src, &mut out, p, INVALID, p + 1);
        } else {
            e = text_end(src, p + width_at(src, p), true);
            p = emit(src, &mut out, p, TASK_NAME, e);
        }
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
