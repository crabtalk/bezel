//! The common `accDescr` terminal for grammars where lexing goes on after an
//! `accDescr {` that never closes, because `accDescr` is also a name there.
//! Pele's `diagrams/common/accDescr.ts`.

use super::tokens::{Scanner, TokenType, acc_descr as base};
use crate::js::Lazy;

static RE_HEAD: Lazy = Lazy::new(
    r"[\t ]*accDescr(?:[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)|\s*(\{))",
    "y",
);

/// The braced form scans for its closing brace. Once none is found there is
/// none further on, so later braced descriptions fail without scanning the
/// rest of the text again.
fn scanner() -> Scanner {
    let mut unclosed = false;
    Box::new(move |src, at| {
        let m = RE_HEAD.match_here(src, at)?;
        let end = m.end();
        if m.get(1).is_none() {
            return Some(end);
        }
        if unclosed {
            return None;
        }
        match src[end..].find('}') {
            Some(close) => Some(end + close + 1),
            None => {
                unclosed = true;
                None
            }
        }
    })
}

pub fn acc_descr<K>(name: &'static str) -> TokenType<K> {
    base().named(name).scanner(scanner)
}
