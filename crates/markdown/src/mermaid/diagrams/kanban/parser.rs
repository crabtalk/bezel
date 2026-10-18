//! Pele's `diagrams/kanban/parser.ts`.

use crate::mermaid::{
    Error,
    diagrams::common::outline::{OutlineDb, Tokens, parse_outline, tokenize as tokenize_outline},
};

pub fn tokenize(src: &str) -> Tokens {
    tokenize_outline(src, true)
}

pub fn parse_kanban(src: &str, db: &mut dyn OutlineDb) -> Result<(), Error> {
    parse_outline(src, db, true)
}
