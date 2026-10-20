//! Pele's `diagrams/mindmap/parser.ts`.

use crate::{
    Error,
    diagrams::common::outline::{Outline, OutlineDb},
};

pub fn parse_mindmap(src: &str, db: &mut dyn OutlineDb) -> Result<(), Error> {
    Outline::Mindmap.parse(src, db)
}
