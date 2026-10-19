//! Pele's `diagrams/xychart/lexer.ts`.

use std::sync::LazyLock;

use super::tokens::*;
use crate::{
    diagrams::common::caseless::word_at,
    util::chars::{code_after, code_at, is_digit, is_letter, is_space, is_word, width_at},
};

const S_INITIAL: usize = 0;
const S_AXIS_DATA: usize = 1;
const S_AXIS_BAND_DATA: usize = 2;
const S_DATA: usize = 3;
const S_DATA_INNER: usize = 4;
const S_STRING: usize = 5;
const S_ACC_TITLE: usize = 6;
const S_ACC_DESCR: usize = 7;
const S_ACC_DESCR_MULTILINE: usize = 8;

/// The token each state gives for the empty rest of the input.
const AT_END: [u8; 9] = [
    EOF,
    EOF,
    EOF,
    EOF,
    EOF,
    STR,
    ACC_TITLE_VALUE,
    ACC_DESCR_VALUE,
    ACC_DESCR_MULTILINE_VALUE,
];

const SINGLE: [u8; 128] = {
    let mut table = [0u8; 128];
    let pairs: [(u8, u8); 11] = [
        (b':', COLON),
        (b'+', PLUS),
        (b',', COMMA),
        (b'=', EQUALS),
        (b'*', MULT),
        (b'#', BRKT),
        (b'_', UNDERSCORE),
        (b'.', DOT),
        (b'&', AMP),
        (b'-', MINUS),
        (b';', SEMI),
    ];
    let mut k = 0;
    while k < pairs.len() {
        table[pairs[k].0 as usize] = pairs[k].1;
        k += 1;
    }
    table
};

/// Mermaid's grammar means to open a markdown string at `"` followed by a
/// backtick, but its quoting turns the rule into one that matches this text
/// and leaves the current state. Markdown strings are never read.
static BROKEN_RULE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "`){}{{ this.pushstate(md_string); }}\n<md_string>(?:(?!`\").)+{}{{ return md_str; }}\n<md_string>(?:`",
        " ".repeat(36),
        " ".repeat(18)
    )
});

pub struct Tokens<'s> {
    pub types: Vec<u8>,
    pub texts: Vec<&'s str>,
    pub starts: Vec<usize>,
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

    // Answers where the lexer goes on from.
    let emit = |tokens: &mut Tokens<'s>, kind: u8, s: usize, e: usize| -> usize {
        tokens.types.push(kind);
        tokens.texts.push(&src[s..e]);
        tokens.starts.push(s);
        e
    };
    let line_end = |q: usize| -> usize { src[q.min(n)..].find('\n').map_or(n, |e| q + e) };
    let run = |mut q: usize| -> usize {
        while q < n && is_space(code_at(src, q)) {
            q += width_at(src, q);
        }
        q
    };
    let digits = |mut q: usize| -> usize {
        while is_digit(code_at(src, q)) {
            q += 1;
        }
        q
    };
    let keyword =
        |p: usize, word: &str| word_at(src, p, word) && !is_word(code_at(src, p + word.len()));

    while p < n {
        let c = code_at(src, p);

        if state == S_STRING {
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
        if state == S_ACC_TITLE || state == S_ACC_DESCR {
            p = emit(&mut tokens, AT_END[state], p, line_end(p));
            if let Some(previous) = stack.pop() {
                state = previous;
            }
            continue;
        }
        if state == S_ACC_DESCR_MULTILINE {
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

        let (next, next_at) = code_after(src, p);
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
            // Only a line break that directly follows an axis or a series ends
            // it; one after spaces is skipped with them, and the state carries
            // on to the next line.
            if (state == S_AXIS_DATA || state == S_DATA) && (c == 10 || next == 10) {
                let end = if c == 10 { p + 1 } else { p + 2 };
                p = emit(&mut tokens, NEWLINE, p, end);
                if let Some(previous) = stack.pop() {
                    state = previous;
                }
                continue;
            }
            let mut e = p + 1;
            while matches!(code_at(src, e), 10 | 13) {
                e += 1;
            }
            p = emit(&mut tokens, NEWLINE, p, e);
            continue;
        }

        match c | 32 {
            116 => {
                if keyword(p, "title") {
                    p = emit(&mut tokens, TITLE, p, p + 5);
                    continue;
                }
            }
            97 => {
                if word_at(src, p, "acc") {
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
            }
            120 => {
                if keyword(p, "xychart-beta") {
                    p = emit(&mut tokens, XYCHART, p, p + 12);
                    continue;
                }
                if keyword(p, "xychart") {
                    p = emit(&mut tokens, XYCHART, p, p + 7);
                    continue;
                }
                if keyword(p, "x-axis") {
                    p = emit(&mut tokens, X_AXIS, p, p + 6);
                    stack.push(state);
                    state = S_AXIS_DATA;
                    continue;
                }
            }
            121 => {
                if keyword(p, "y-axis") {
                    p = emit(&mut tokens, Y_AXIS, p, p + 6);
                    stack.push(state);
                    state = S_AXIS_DATA;
                    continue;
                }
            }
            118 => {
                if word_at(src, p, "vertical") {
                    p = emit(&mut tokens, CHART_ORIENTATION, p, p + 8);
                    continue;
                }
            }
            104 => {
                if word_at(src, p, "horizontal") {
                    p = emit(&mut tokens, CHART_ORIENTATION, p, p + 10);
                    continue;
                }
            }
            108 => {
                if keyword(p, "line") {
                    p = emit(&mut tokens, LINE, p, p + 4);
                    stack.push(state);
                    state = S_DATA;
                    continue;
                }
            }
            98 if keyword(p, "bar") => {
                p = emit(&mut tokens, BAR, p, p + 3);
                stack.push(state);
                state = S_DATA;
                continue;
            }
            _ => {}
        }

        if c == 91 {
            if state == S_AXIS_DATA {
                stack.push(state);
                state = S_AXIS_BAND_DATA;
            } else if state == S_DATA {
                stack.push(state);
                state = S_DATA_INNER;
            }
            p = emit(&mut tokens, SQUARE_BRACES_START, p, p + 1);
            continue;
        }
        if c == 93 {
            if (state == S_DATA_INNER || state == S_AXIS_BAND_DATA)
                && let Some(previous) = stack.pop()
            {
                state = previous;
            }
            p = emit(&mut tokens, SQUARE_BRACES_END, p, p + 1);
            continue;
        }
        if state == S_AXIS_DATA && c == 45 && next == 45 && code_at(src, p + 2) == 62 {
            p = emit(&mut tokens, ARROW_DELIMITER, p, p + 3);
            continue;
        }
        if state == S_AXIS_DATA || state == S_DATA_INNER {
            let q = if c == 43 || c == 45 { p + 1 } else { p };
            let d = code_at(src, q);
            let mut e = None;
            if is_digit(d) {
                let mut end = digits(q + 1);
                if code_at(src, end) == 46 && is_digit(code_at(src, end + 1)) {
                    end = digits(end + 2);
                }
                e = Some(end);
            } else if d == 46 && is_digit(code_at(src, q + 1)) {
                e = Some(digits(q + 2));
            }
            if let Some(e) = e {
                p = emit(&mut tokens, NUMBER_WITH_DECIMAL, p, e);
                continue;
            }
        }
        if c == 96 && word_at(src, p, &BROKEN_RULE) {
            p += BROKEN_RULE.len();
            if let Some(previous) = stack.pop() {
                state = previous;
            }
            continue;
        }
        if c == 34 {
            p += 1;
            stack.push(state);
            state = S_STRING;
            continue;
        }
        if is_letter(c) {
            let mut e = p + 1;
            while is_letter(code_at(src, e)) {
                e += 1;
            }
            p = emit(&mut tokens, ALPHA, p, e);
            continue;
        }
        if is_digit(c) {
            let end = digits(p + 1);
            p = emit(&mut tokens, NUM, p, end);
            continue;
        }
        if is_space(c) {
            p = run(p + width_at(src, p));
            continue;
        }
        if c < 128 && SINGLE[c as usize] != 0 {
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
    tokens.types.push(AT_END[state]);
    tokens.texts.push("");
    tokens.starts.push(n);
    tokens.types.push(END);
    tokens.texts.push("");
    tokens.starts.push(n);
    tokens
}
