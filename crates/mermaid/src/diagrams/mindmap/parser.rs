//! Pele's `diagrams/mindmap/parser.ts`.

use crate::{
    Error,
    diagrams::common::outline::{OutlineDb, Tokens, parse_outline, tokenize as tokenize_outline},
};

pub fn tokenize(src: &str) -> Tokens {
    tokenize_outline(src, false)
}

pub fn parse_mindmap(src: &str, db: &mut dyn OutlineDb) -> Result<(), Error> {
    parse_outline(src, db, false)
}
