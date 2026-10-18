//! Mermaid source drawn as SVG: every diagram type of Mermaid 12.1, read
//! with its syntax and drawn in bezel's style.
//!
//! A port of Pele 1.6.0 (github.com/obsidianmd/pele, MIT): its parsers,
//! layouts and SVG, so a source draws as Pele draws it, class names and CSS
//! variables named `bezel-*` for `pele-*`. Builds without gpui.
//!
//! ```
//! let drawing = markdown::mermaid::render("info", &Default::default()).unwrap();
//! assert!(drawing.svg.starts_with("<svg"));
//! ```

use std::rc::Rc;

use serde::Serialize;

pub mod config;
pub mod detect;
pub mod diagrams;
pub mod error;
pub(crate) mod js;
pub mod layout;
pub mod preprocess;
pub mod svg;
pub mod text;
pub mod util;
pub mod yaml;

#[cfg(feature = "gpui")]
pub(crate) mod fence;

pub use config::{Config, Value};
pub use detect::Kind;
pub use error::{Code, Error};
#[cfg(feature = "gpui")]
pub use fence::{DIAGRAM_SELECTOR, LANGUAGE, Zoom};
pub use text::{IconResolver, Measure, Metrics};

/// What class names, CSS variables and element ids start with.
pub const PREFIX: &str = "bezel";

/// Mermaid's default `maxTextSize`.
const DEFAULT_LIMIT: usize = 50_000;
/// Mermaid has no limit on its output. A source within the limit can still
/// describe a drawing of many megabytes, which stalls what it is put into.
const DEFAULT_OUTPUT_LIMIT: usize = 4_000_000;
/// Mermaid stops at 500. `A & B --> C & D` multiplies: without a limit, a
/// few kilobytes of source ask for millions of edges.
const DEFAULT_MAX_EDGES: usize = 5_000;

/// How a diagram is drawn. Every field is optional, as Pele's
/// `RenderOptions` are; `Default` draws as Pele does with no options.
#[derive(Clone, Default)]
pub struct Options {
    /// What labels are measured with. [`Metrics`] when unset.
    pub measure: Option<Rc<dyn Measure>>,
    /// The fonts labels are measured in, and drawn in where the page sets no
    /// `--bezel-font` or `--bezel-font-mono`.
    pub font_family: Option<String>,
    pub font_family_mono: Option<String>,
    pub font_size: Option<f64>,
    pub id_prefix: Option<String>,
    pub responsive: Option<bool>,
    /// The width the host has, in px. A type that can draw itself narrower
    /// does, so its text is not shrunk with it.
    pub max_width: Option<f64>,
    /// Whether a diagram that runs across may be drawn running down when it
    /// does not fit `max_width`, and the width under which that is done. On,
    /// under 640 px, by default.
    pub auto_direction: Option<bool>,
    pub direction_breakpoint: Option<f64>,
    pub padding: Option<f64>,
    /// The longest source, in UTF-16 units.
    pub limit: Option<usize>,
    /// The longest SVG, in UTF-16 units.
    pub output_limit: Option<usize>,
    /// The most edges a flowchart may have.
    pub max_edges: Option<usize>,
    /// Schemes a link or an image may use besides a relative address.
    pub link_schemes: Option<Vec<String>>,
    pub image_schemes: Option<Vec<String>>,
    pub links: Option<bool>,
    pub images: Option<bool>,
    /// A link's `rel`. `noopener` when unset; empty leaves it out.
    pub link_rel: Option<String>,
    pub icons: Option<IconResolver>,
    /// Configuration under the source's own, as Mermaid's site config.
    pub config: Option<Config>,
    /// The time gantt charts treat as now, in ms since the epoch.
    pub now: Option<f64>,
}

impl Options {
    /// What labels are measured with.
    pub fn measurer(&self) -> Rc<dyn Measure> {
        self.measure.clone().unwrap_or_else(|| Rc::new(Metrics))
    }
}

/// Limits set by the host. A diagram's own configuration cannot change them.
#[derive(Clone, Copy, Debug)]
pub struct ParseLimits {
    pub max_edges: usize,
}

/// A link the drawing holds: the element's id, and where it goes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Link {
    pub id: String,
    pub href: String,
    pub internal: bool,
}

/// A diagram type's drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rendered {
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub links: Vec<Link>,
}

/// A source's drawing, with the type it was.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Drawing {
    #[serde(rename = "type")]
    pub kind: Kind,
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub links: Vec<Link>,
}

/// A parsed source, ready to draw.
pub struct Parsed {
    pub kind: Kind,
    pub model: diagrams::Model,
    pub config: Config,
}

/// The type `source` names, if any.
pub fn detect_type(source: &str) -> Option<Kind> {
    let pre = preprocess::preprocess(source).ok()?;
    detect::detect(&pre.text)
}

/// Whether `source` names a type this build draws.
pub fn supports(source: &str) -> bool {
    detect_type(source).is_some_and(|kind| diagrams::diagram(kind).is_some())
}

/// Parses `source`, reading [`Options::limit`], [`Options::max_edges`] and
/// [`Options::config`].
pub fn parse(source: &str, options: &Options) -> Result<Parsed, Error> {
    let limit = options.limit.unwrap_or(DEFAULT_LIMIT);
    if js::len(source) > limit {
        return Err(Error::limit(format!(
            "Diagram source is longer than the limit of {limit} characters."
        )));
    }
    let pre = preprocess::preprocess(source)?;
    let Some(kind) = detect::detect(&pre.text) else {
        return Err(Error::new(
            "No diagram type detected for the given text.",
            Code::UnsupportedDiagram,
        ));
    };
    let Some(diagram) = diagrams::diagram(kind) else {
        return Err(Error::new(
            format!("Diagram type \"{}\" is not supported yet.", kind.name()),
            Code::UnsupportedDiagram,
        )
        .of(kind));
    };
    let config = match &options.config {
        Some(extra) => {
            let mut config = extra.clone();
            for (key, value) in pre.config.iter() {
                config.insert(key, value.clone());
            }
            config
        }
        None => pre.config.clone(),
    };
    let text = if diagram.keep_comments() {
        &pre.with_comments
    } else {
        &pre.text
    };
    let encoded = preprocess::encode_entities(text) + "\n";
    let limits = ParseLimits {
        max_edges: options.max_edges.unwrap_or(DEFAULT_MAX_EDGES),
    };
    let model = diagram.parse(
        &encoded,
        &config,
        pre.title.as_deref(),
        pre.line_offset,
        &limits,
    )?;
    Ok(Parsed {
        kind,
        model,
        config,
    })
}

/// `source` drawn as SVG.
pub fn render(source: &str, options: &Options) -> Result<Drawing, Error> {
    render_parsed(&parse(source, options)?, options)
}

/// A parsed source drawn as SVG.
pub fn render_parsed(parsed: &Parsed, options: &Options) -> Result<Drawing, Error> {
    let kind = parsed.kind;
    let Some(diagram) = diagrams::diagram(kind) else {
        return Err(Error::new(
            format!("Diagram type \"{}\" is not supported yet.", kind.name()),
            Code::UnsupportedDiagram,
        )
        .of(kind));
    };
    let section = parsed.config.get(diagram.section().unwrap_or(kind.name()));
    let fixed = section
        .and_then(Value::as_map)
        .and_then(|section| section.get("useMaxWidth"))
        == Some(&Value::Bool(false));
    let mut options = options.clone();
    options.responsive = Some(options.responsive.unwrap_or(!fixed));
    // The host's resolver and the ids in `links` see names as the author
    // meant them, entity codes turned into their characters.
    let icons = options.icons.clone().map(|resolve| -> IconResolver {
        Rc::new(move |name: &str| resolve(&text::decode_entities(name)))
    });
    options.icons = icons.clone();
    let rendered = text::with_icons(icons, || {
        diagram.render(&parsed.model, &parsed.config, &options)
    })?;
    let output_limit = options.output_limit.unwrap_or(DEFAULT_OUTPUT_LIMIT);
    if js::len(&rendered.svg) > output_limit {
        return Err(Error::limit(format!(
            "Diagram output is longer than the limit of {output_limit} characters."
        ))
        .of(kind));
    }
    Ok(Drawing {
        kind,
        svg: rendered.svg,
        width: rendered.width,
        height: rendered.height,
        links: rendered
            .links
            .into_iter()
            .map(|link| Link {
                id: text::decode_entities(&link.id),
                ..link
            })
            .collect(),
    })
}
