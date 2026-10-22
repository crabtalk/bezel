//! The common `accDescr` terminal for grammars where lexing goes on after an
//! `accDescr {` that never closes, because `accDescr` is also a name there.
//! Pele's `diagrams/common/accDescr.ts`.

use crate::diagrams::common::{
    tokens,
    tokens::{Heading, Scanner, TokenType},
};

/// The braced form scans for its closing brace. Once none is found there is
/// none further on, so later braced descriptions fail without scanning the
/// rest of the text again.
fn scanner() -> Scanner {
    let mut unclosed = false;
    Box::new(move |src, at| {
        let (end, braced) = tokens::acc_descr_head(src, at)?;
        if !braced {
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
    TokenType::scanner(name, scanner, Some("\t a")).heading(Heading::AccDescr)
}
