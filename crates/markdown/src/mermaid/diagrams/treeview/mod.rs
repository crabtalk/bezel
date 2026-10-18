//! `treeView`: a file tree. Pele's `diagrams/treeview`.

mod box_drawing;
mod icons;
mod model;
mod parser;
mod render;

pub use model::{NodeType, TreeNode, TreeViewModel};

use super::{Diagram, Model};
use crate::mermaid::{Config, Error, Options, ParseLimits, Rendered};

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
        Ok(Model::TreeView(model::build_tree_view(
            model::read_tree_view(source)?,
            title,
        )))
    }

    fn render(&self, model: &Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
        let Model::TreeView(model) = model else {
            unreachable!("treeView renders a treeView model")
        };
        Ok(render::render_tree_view(model, config, options))
    }
}
