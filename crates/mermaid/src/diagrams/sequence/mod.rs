//! `sequenceDiagram`: participants and the messages between them, top to
//! bottom. Pele's `diagrams/sequence`.

pub mod actors;
pub mod colors;
pub mod db;
pub mod lexer;
pub mod parser;
pub mod render;
pub mod tokens;
pub mod types;

pub use db::SequenceModel;

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::{Diagram, Model};
use crate::{Config, Error, ParseLimits};

pub struct Sequence;

impl Diagram for Sequence {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let wrap = config.wrap;
        let sequence = &config.sequence;
        let sequence_wrap = sequence.wrap == Some(true);
        let mut db = SequenceModel::new(wrap, sequence_wrap);
        if let Some(title) = title.filter(|title| !title.is_empty()) {
            db.titles.set_title(title);
        }
        parser::parse_sequence(source, &mut db)?;
        Ok(Model::Sequence(Box::new(db)))
    }
}

/// Mermaid's `sequence` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SequenceConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub wrap: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub mirror_actors: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub right_angles: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub show_sequence_numbers: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub hide_unused_participants: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub actor_margin: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub height: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub message_margin: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub note_margin: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub box_margin: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub message_align: Option<String>,
}
