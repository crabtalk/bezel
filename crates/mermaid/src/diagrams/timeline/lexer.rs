//! Pele's `diagrams/timeline/lexer.ts`.

use crate::{
    diagrams::common::scan::{Lexer, T},
    token::Token,
    util::chars::is_space,
};

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let mut lx = Lexer::new(src);
    while lx.skip_trivia() {
        if lx.newline() {
            continue;
        }
        let cur = &mut lx.cur;
        let p = cur.p;
        if cur.starts_with_caseless("timeline", p) {
            let e = cur.run_end(p + 8, |c| matches!(c, ' ' | '\t'));
            let lr = cur.keyword_caseless("lr", e);
            if e > p + 8 && (lr || cur.keyword_caseless("td", e)) {
                cur.emit(if lr { T::TimelineLr } else { T::TimelineTd }, p, e + 2);
                continue;
            }
            if cur.keyword_caseless("timeline", p) {
                cur.emit(T::Timeline, p, p + 8);
                continue;
            }
        }
        // `\s` after a keyword also takes a line break, so `title` alone on
        // a line takes the next line as its text.
        if let Some(end) = cur.keyword_line("title", &['\n']) {
            cur.emit(T::Title, p, end);
            continue;
        }
        if lx.accessibility() {
            continue;
        }
        let cur = &mut lx.cur;
        if let Some(end) = cur.keyword_line("section", &[':', '\n']) {
            cur.emit(T::Section, p, end);
            continue;
        }
        let Some(c) = cur.char_at(p) else {
            break;
        };
        if c != ':' {
            let end = cur.until(p + c.len_utf8(), |c| matches!(c, '#' | ':' | '\n'));
            cur.emit(T::Period, p, end);
            continue;
        }
        // An event runs to the end of the line or to a colon that is followed
        // by whitespace.
        if let Some(space) = cur.char_at(p + 1).filter(|&c| is_space(c)) {
            let from = p + 1 + space.len_utf8();
            let end = src[from..]
                .char_indices()
                .find(|&(at, c)| {
                    c == '\n' || (c == ':' && cur.char_at(from + at + 1).is_some_and(is_space))
                })
                .map_or(src.len(), |(at, _)| from + at);
            if end > from {
                cur.emit(T::Event, p, end);
                continue;
            }
        }
        cur.emit(T::Invalid, p, p + 1);
    }
    lx.finish()
}
