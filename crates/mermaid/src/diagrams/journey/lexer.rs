//! Pele's `diagrams/journey/lexer.ts`.

use crate::{
    diagrams::common::scan::{Lexer, T},
    token::Token,
};

/// `#`, a line break, and `;` end every kind of text; a colon also ends a
/// task name or a section.
const TEXT_STOPS: [char; 3] = ['#', '\n', ';'];
const NAME_STOPS: [char; 4] = ['#', '\n', ';', ':'];

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let mut lx = Lexer::new(src);
    while lx.skip_trivia() {
        if lx.newline() {
            continue;
        }
        let cur = &mut lx.cur;
        let p = cur.p;
        if cur.keyword_caseless("journey", p) {
            cur.emit(T::Journey, p, p + 7);
            continue;
        }
        // `\s` after a keyword also takes a line break, so `title` alone on
        // a line takes the next line as its text.
        if let Some(end) = cur.keyword_line("title", &TEXT_STOPS) {
            cur.emit(T::Title, p, end);
            continue;
        }
        if lx.accessibility() {
            continue;
        }
        let cur = &mut lx.cur;
        if let Some(end) = cur.keyword_line("section", &NAME_STOPS) {
            cur.emit(T::Section, p, end);
            continue;
        }
        match cur.char_at(p) {
            Some(':') => {
                let end = cur.until(p + 1, |c| TEXT_STOPS.contains(&c));
                let kind = if end > p + 1 { T::TaskData } else { T::Colon };
                cur.emit(kind, p, end);
            }
            Some(';') => cur.emit(T::Invalid, p, p + 1),
            Some(c) => {
                let end = cur.until(p + c.len_utf8(), |c| NAME_STOPS.contains(&c));
                cur.emit(T::TaskName, p, end);
            }
            None => {}
        }
    }
    lx.finish()
}
