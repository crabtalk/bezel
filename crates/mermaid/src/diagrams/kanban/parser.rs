//! Pele's `diagrams/kanban/parser.ts`.

use crate::{
    Error,
    diagrams::common::outline::{Outline, OutlineDb},
};

pub fn parse_kanban(src: &str, db: &mut dyn OutlineDb) -> Result<(), Error> {
    Outline::Kanban.parse(src, db)
}
