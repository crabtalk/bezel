//! Every diagram type, and the one a source names.

use serde::Serialize;

use super::{Config, Error, Kind, Options, ParseLimits, Rendered};

pub mod common;
pub mod info;

/// What a diagram type provides. `source` has been through preprocessing and
/// entity encoding and ends with a newline, as Mermaid's parsers expect.
pub trait Diagram: Sync {
    /// The name of this type's section in Mermaid's config, when it is not
    /// the type's own name.
    fn section(&self) -> Option<&'static str> {
        None
    }

    /// A type that reports source positions is given the text with its `%%`
    /// comment lines kept. `line_offset` counts the front matter lines
    /// removed before it.
    fn keep_comments(&self) -> bool {
        false
    }

    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        line_offset: usize,
        limits: &ParseLimits,
    ) -> Result<Model, Error>;

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error>;
}

/// A parsed diagram, of whichever type, as Pele's `parse` answers it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Model {
    Info(info::InfoModel),
}

/// The type `kind`, when this build has it.
pub fn diagram(kind: Kind) -> Option<&'static dyn Diagram> {
    Some(match kind {
        Kind::Info => &info::Info,
        _ => return None,
    })
}
