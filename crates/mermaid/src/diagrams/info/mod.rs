//! `info`: the renderer's name and version. Pele's `diagrams/info`.

mod parser;

use serde::Serialize;

use super::{Diagram, Model};
use crate::{
    Config, Error, Options, ParseLimits, Rendered,
    js::Lazy,
    svg::{
        builder::label_svg,
        root::{Accessible, svg_document},
    },
    text::{Style, layout_label},
};

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
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
        let ast = parser::parse_info(source)?;
        Ok(Model::Info(InfoModel {
            kind: "info",
            title: ast
                .title
                .filter(|title| !title.is_empty())
                .or_else(|| title.map(str::to_string)),
            acc_title: ast
                .acc_title
                .filter(|text| !text.is_empty())
                .map(|text| RE_LEAD.replace(&text, |_| String::new())),
            acc_descr: ast
                .acc_descr
                .filter(|text| !text.is_empty())
                .map(|text| RE_INDENT.replace(&text, |_| "\n".into())),
        }))
    }

    fn render(
        &self,
        model: &Model,
        _config: &Config,
        options: &Options,
    ) -> Result<Rendered, Error> {
        let model = match model {
            Model::Info(model) => model,
            #[allow(unreachable_patterns)]
            _ => unreachable!("info renders an info model"),
        };
        let size = options.font_size.unwrap_or(16.0);
        let measure = options.measurer();
        let pad = options.padding.unwrap_or(8.0);

        let title = layout_label(
            model.title.as_deref(),
            false,
            &*measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = if title.height > 0.0 {
            title.height + 8.0
        } else {
            0.0
        };
        let version = format!("Bezel v{}", env!("CARGO_PKG_VERSION"));
        let text = layout_label(Some(&version), false, &*measure, size, 4000.0, 0);

        let width = (title.width.max(text.width) + 2.0 * pad).ceil();
        let height = (title_height + text.height + 2.0 * pad).ceil();
        let p = crate::PREFIX;
        let content = label_svg(
            &title,
            width / 2.0,
            pad + title.height / 2.0,
            &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
        ) + &label_svg(
            &text,
            width / 2.0,
            pad + title_height + text.height / 2.0,
            &format!(" class=\"{p}-version\" fill=\"var(--_m)\""),
        );
        Ok(Rendered {
            svg: svg_document(
                "info",
                width,
                height,
                size,
                options,
                Accessible {
                    acc_title: model.acc_title.as_deref(),
                    acc_descr: model.acc_descr.as_deref(),
                },
                &content,
            ),
            width,
            height,
            links: Vec::new(),
        })
    }
}
