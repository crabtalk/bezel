//! `treeView`: a file tree. Pele's `diagrams/treeview`.

mod box_drawing;
mod icons;
mod model;
mod parser;
mod render;

pub use model::{NodeType, TreeNode, TreeViewModel};

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::{Diagram, Model};
use crate::config::{Map, Value};
use crate::{Config, Error, ParseLimits};

pub struct TreeView;

impl Diagram for TreeView {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::TreeView(TreeViewModel::new(
            model::read_tree_view(source)?,
            title,
        )))
    }
}

/// Mermaid's `treeView` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TreeViewConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub padding_x: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub padding_y: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub row_indent: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub line_thickness: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub show_icons: Option<Value>,
    #[serde_as(as = "DefaultOnError")]
    pub default_icon_pack: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    pub filename_icons: Option<Map>,
    #[serde_as(as = "DefaultOnError")]
    pub extension_icons: Option<Map>,
}
