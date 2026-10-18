//! Pele's `diagrams/quadrant/lexer.ts`.

use super::tokens::*;
use crate::mermaid::{
    diagrams::common::caseless::word_at,
    util::chars::{
        code_after, code_at, is_digit, is_letter, is_line_end, is_space, is_word, width_at,
    },
};

const S_INITIAL: usize = 0;
const S_TITLE: usize = 1;
const S_ACC_TITLE: usize = 2;
const S_ACC_DESCR: usize = 3;
const S_ACC_DESCR_MULTILINE: usize = 4;
const S_STRING: usize = 5;
const S_MD_STRING: usize = 6;
const S_CLASS_NAME: usize = 7;
const S_POINT_START: usize = 8;
const S_POINT_X: usize = 9;
const S_POINT_Y: usize = 10;

/// The token each state gives for the empty rest of the input, where it gives
/// one.
const AT_END: [u8; 6] = [
    EOF,
    TITLE_VALUE,
    ACC_TITLE_VALUE,
    ACC_DESCR_VALUE,
    ACC_DESCR_MULTILINE_VALUE,
    STR,
];

const SINGLE: [u8; 128] = {
    let mut table = [0u8; 128];
    let groups: [(&[u8], u8); 13] = [
        (b":", COLON),
        (b"+", PLUS),
        (b",", COMMA),
        (b"=", EQUALS),
        (b"*", MULT),
        (b"#", BRKT),
        (b"_", UNDERSCORE),
        (b".", DOT),
        (b"&", AMP),
        (b"-", MINUS),
        (b" \t\x0b\x0c", SPACE),
        (b";", SEMI),
        (b"!$%'`?\\/", PUNCTUATION),
    ];
    let mut g = 0;
    while g < groups.len() {
        let (chars, kind) = groups[g];
        let mut k = 0;
        while k < chars.len() {
            table[chars[k] as usize] = kind;
            k += 1;
        }
        g += 1;
    }
    table
};

pub struct Tokens<'s> {
    pub types: Vec<u8>,
    pub texts: Vec<&'s str>,
    pub starts: Vec<usize>,
}

/// Whether the byte at `at` is past ASCII, as a UTF-16 unit of 128 or more
/// is.
fn is_wide(src: &str, at: usize) -> bool {
    src.as_bytes().get(at).is_some_and(|&b| b >= 128)
}

pub fn tokenize<'s>(src: &'s str) -> Tokens<'s> {
    let n = src.len();
    let mut tokens = Tokens {
        types: Vec::new(),
        texts: Vec::new(),
        starts: Vec::new(),
    };
    let mut stack: Vec<usize> = Vec::new();
    let mut state = S_INITIAL;
    let mut p = 0;
    // The last run scanned of each kind, so that a long run is not scanned
    // again from each of its characters.
    let (mut space_from, mut space_to) = (0, 0);
    let (mut dash_from, mut dash_to) = (0, 0);
    let (mut ws_from, mut ws_to) = (0, 0);
    let (mut gap_from, mut gap_to) = (0, 0);

    // Answers where the lexer goes on from.
    let emit = |tokens: &mut Tokens<'s>, kind: u8, s: usize, e: usize| -> usize {
        tokens.types.push(kind);
        tokens.texts.push(&src[s..e]);
        tokens.starts.push(s);
        e
    };
    let line_end = |q: usize| -> usize { src[q.min(n)..].find('\n').map_or(n, |e| q + e) };
    let spaces = |mut q: usize| -> usize {
        while code_at(src, q) == 32 {
            q += 1;
        }
        q
    };
    let run = |mut q: usize| -> usize {
        while q < n && is_space(code_at(src, q)) {
            q += width_at(src, q);
        }
        q
    };
    // Mermaid's `(1)|(0(.\d+)?)`, where the dot is any character within a
    // line.
    let coordinate = |q: usize| -> Option<usize> {
        let c = code_at(src, q);
        if c == 49 {
            return Some(q + 1);
        }
        if c != 48 {
            return None;
        }
        let mut e = q + 1;
        if e < n && !is_line_end(code_at(src, e)) {
            let (after, at) = code_after(src, e);
            if is_digit(after) {
                e = at + 1;
                while is_digit(code_at(src, e)) {
                    e += 1;
                }
            }
        }
        Some(e)
    };

    'lexing: while p < n {
        let c = code_at(src, p);
        let (next, next_at) = code_after(src, p);

        match state {
            S_INITIAL => {}
            S_TITLE | S_ACC_TITLE | S_ACC_DESCR => {
                p = emit(&mut tokens, AT_END[state], p, line_end(p));
                if let Some(previous) = stack.pop() {
                    state = previous;
                }
                continue;
            }
            S_ACC_DESCR_MULTILINE => {
                if c == 125 {
                    p += 1;
                    if let Some(previous) = stack.pop() {
                        state = previous;
                    }
                } else {
                    let e = src[p..].find('}').map_or(n, |e| p + e);
                    p = emit(&mut tokens, ACC_DESCR_MULTILINE_VALUE, p, e);
                }
                continue;
            }
            S_STRING => {
                if c == 34 {
                    p += 1;
                    if let Some(previous) = stack.pop() {
                        state = previous;
                    }
                } else {
                    let e = src[p..].find('"').map_or(n, |e| p + e);
                    p = emit(&mut tokens, STR, p, e);
                }
                continue;
            }
            S_MD_STRING => {
                if c == 96 && next == 34 {
                    p += 2;
                    if let Some(previous) = stack.pop() {
                        state = previous;
                    }
                    continue;
                }
                if c != 96 && c != 34 {
                    let mut e = p + width_at(src, p);
                    while e < n && !matches!(src.as_bytes()[e], b'`' | b'"') {
                        e += 1;
                    }
                    p = emit(&mut tokens, MD_STR, p, e);
                    continue;
                }
                break 'lexing;
            }
            S_CLASS_NAME => {
                if is_word(c) {
                    let mut e = p + 1;
                    while is_word(code_at(src, e)) {
                        e += 1;
                    }
                    p = emit(&mut tokens, CLASS_NAME, p, e);
                    if let Some(previous) = stack.pop() {
                        state = previous;
                    }
                    continue;
                }
                break 'lexing;
            }
            S_POINT_START => {
                if let Some(e) = coordinate(p) {
                    p = emit(&mut tokens, POINT_X, p, e);
                    stack.push(state);
                    state = S_POINT_X;
                    continue;
                }
                let k = run(p);
                if code_at(src, k) == 93 {
                    p = spaces(k + 1);
                    if let Some(previous) = stack.pop() {
                        state = previous;
                    }
                    continue;
                }
                break 'lexing;
            }
            S_POINT_X => {
                let k = run(p);
                if code_at(src, k) == 44 {
                    p = run(k + 1);
                    state = S_POINT_Y;
                    continue;
                }
                break 'lexing;
            }
            S_POINT_Y => {
                if let Some(e) = coordinate(p) {
                    p = emit(&mut tokens, POINT_Y, p, e);
                    if let Some(previous) = stack.pop() {
                        state = previous;
                    }
                    continue;
                }
                break 'lexing;
            }
            _ => {}
        }

        if c == 37 && next == 37 {
            p = line_end(p + 2);
            continue;
        }
        // Mermaid's second comment rule takes any character before `%%` with
        // it, a line break included.
        if c != 125 && next == 37 && code_at(src, next_at + 1) == 37 {
            p = line_end(next_at + 2);
            continue;
        }
        if c == 10 || c == 13 {
            let mut e = p + 1;
            while matches!(code_at(src, e), 10 | 13) {
                e += 1;
            }
            p = emit(&mut tokens, NEWLINE, p, e);
            continue;
        }

        let lower = c | 32;
        if lower == 116 && word_at(src, p, "title") && !is_word(code_at(src, p + 5)) {
            p = emit(&mut tokens, TITLE, p, p + 5);
            stack.push(state);
            state = S_TITLE;
            continue;
        }
        if lower == 97 && word_at(src, p, "acc") {
            let descr = word_at(src, p + 3, "descr");
            if descr || word_at(src, p + 3, "title") {
                let k = run(p + 8);
                let d = code_at(src, k);
                if d == 58 {
                    let end = run(k + 1);
                    p = emit(
                        &mut tokens,
                        if descr { ACC_DESCR } else { ACC_TITLE },
                        p,
                        end,
                    );
                    stack.push(state);
                    state = if descr { S_ACC_DESCR } else { S_ACC_TITLE };
                    continue;
                }
                if d == 123 && descr {
                    p = run(k + 1);
                    stack.push(state);
                    state = S_ACC_DESCR_MULTILINE;
                    continue;
                }
            }
        }

        // Axis, quadrant and chart keywords and the axis arrow take the spaces
        // around them.
        let mut j = p;
        if c == 32 {
            if p < space_from || p >= space_to {
                space_from = p;
                space_to = spaces(p);
            }
            j = space_to;
        }
        let d = code_at(src, j);
        let dl = d | 32;
        let mut kind = 0;
        let mut end = 0;
        if dl == 120 && word_at(src, j, "x-axis") {
            kind = X_AXIS;
            end = j + 6;
        } else if dl == 121 && word_at(src, j, "y-axis") {
            kind = Y_AXIS;
            end = j + 6;
        } else if d == 45 {
            if j < dash_from || j >= dash_to {
                dash_from = j;
                dash_to = j;
                while code_at(src, dash_to) == 45 {
                    dash_to += 1;
                }
            }
            if dash_to - j >= 2 && code_at(src, dash_to) == 62 {
                kind = DELIMITER;
                end = dash_to + 1;
            }
        } else if dl == 113 && word_at(src, j, "quadrant") {
            let q = code_at(src, j + 9);
            if code_at(src, j + 8) == 45 && (49..=52).contains(&q) {
                kind = QUADRANT_1 + (q - 49) as u8;
                end = j + 10;
            } else if word_at(src, j + 8, "chart") {
                kind = QUADRANT;
                end = j + 13;
            }
        }
        if kind != 0 {
            let to = spaces(end);
            p = emit(&mut tokens, kind, p, to);
            continue;
        }

        if lower == 99 && word_at(src, p, "classdef") && !is_word(code_at(src, p + 8)) {
            p = emit(&mut tokens, CLASSDEF, p, p + 8);
            continue;
        }
        if c == 34 {
            stack.push(state);
            if next == 96 {
                p += 2;
                state = S_MD_STRING;
            } else {
                p += 1;
                state = S_STRING;
            }
            continue;
        }
        if c == 58 && next == 58 && code_at(src, p + 2) == 58 {
            p += 3;
            stack.push(state);
            state = S_CLASS_NAME;
            continue;
        }
        // A point's `: [`, with any whitespace before, between and after, line
        // breaks included.
        if c == 58 || is_space(c) {
            let mut k = p;
            if c != 58 {
                if p < ws_from || p >= ws_to {
                    ws_from = p;
                    ws_to = run(p);
                }
                k = ws_to;
            }
            if code_at(src, k) == 58 {
                if k + 1 < gap_from || k + 1 >= gap_to {
                    gap_from = k + 1;
                    gap_to = run(k + 1);
                }
                if code_at(src, gap_to) == 91 {
                    let to = run(gap_to + 1);
                    p = emit(&mut tokens, POINT_START, p, to);
                    stack.push(state);
                    state = S_POINT_START;
                    continue;
                }
            }
        }

        if is_letter(c) {
            let mut e = p + 1;
            while is_letter(code_at(src, e)) {
                e += 1;
            }
            p = emit(&mut tokens, ALPHA, p, e);
            continue;
        }
        if c >= 128 {
            let mut e = p + 1;
            while is_wide(src, e) {
                e += 1;
            }
            p = emit(&mut tokens, UNICODE_TEXT, p, e);
            continue;
        }
        if is_digit(c) {
            let mut e = p + 1;
            while is_digit(code_at(src, e)) {
                e += 1;
            }
            p = emit(&mut tokens, NUM, p, e);
            continue;
        }
        if SINGLE[c as usize] != 0 {
            p = emit(&mut tokens, SINGLE[c as usize], p, p + 1);
            continue;
        }
        break;
    }

    if p < n {
        tokens.types.push(ERROR);
        tokens.texts.push("");
        tokens.starts.push(p);
        return tokens;
    }
    // Jison tries the rules of the current state once more against the empty
    // rest of the input.
    if state < AT_END.len() {
        tokens.types.push(AT_END[state]);
        tokens.texts.push("");
        tokens.starts.push(n);
    }
    tokens.types.push(END);
    tokens.texts.push("");
    tokens.starts.push(n);
    tokens
}
