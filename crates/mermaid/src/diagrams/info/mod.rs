//! `info`: the renderer's name and version. Pele's `diagrams/info`.

use crate::diagrams::common::db::Titles;
use crate::geom::Point;
use crate::text::Label;
mod parser;

use serde::Serialize;

use super::{Diagram, Model, Render};
use crate::{
    Config, Error, ParseLimits, Rendered,
    svg::{builder::TITLE, root::svg_document},
    text::Style,
};
use crate::{Ctx, Host};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
}

pub struct Info;

impl Diagram for Info {
    fn parse(
        &self,
        source: &str,
        _config: &Config,
        title: Option<&str>,
        _line_offset: usize,
        _limits: &ParseLimits,
    ) -> Result<Model, Error> {
        Ok(Model::Info(InfoModel {
            kind: "info",
            titles: parser::parse_info(source)?.titles(title),
        }))
    }
}

impl Render for InfoModel {
    fn render(&self, _config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let size = ctx.options.font_size;
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = title.spaced_height(8.0);
        let version = format!("Bezel v{}", env!("CARGO_PKG_VERSION"));
        let text = Label::layout(Some(&version), false, measure, size, 4000.0, 0);

        let width = (title.width.max(text.width) + 2.0 * pad).ceil();
        let height = (title_height + text.height + 2.0 * pad).ceil();
        let mut content = String::new();
        let _ = write!(
            content,
            "{}{}",
            title.svg(Point::new(width / 2.0, pad + title.height / 2.0), TITLE),
            text.svg(
                Point::new(width / 2.0, pad + title_height + text.height / 2.0),
                crate::class!("version", "fill=\"var(--_m)\"")
            )
        );
        Ok(Rendered {
            svg: svg_document(
                "info",
                width,
                height,
                size,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width,
            height,
            links: Vec::new(),
        })
    }
}
