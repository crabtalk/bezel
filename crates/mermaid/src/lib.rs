//! Mermaid source drawn as SVG: every diagram type of Mermaid 12.1, read
//! with its syntax and drawn in bezel's style.
//!
//! A port of Pele 1.6.0 (github.com/obsidianmd/pele, MIT): its parsers,
//! layouts and SVG, so a source draws as Pele draws it, class names, CSS
//! variables and element ids named `bezel-*` for `pele-*`. The SVG does not
//! match mermaid.js's.
//!
//! Builds without gpui, for `wasm32-unknown-unknown` too.
//!
//! ```
//! let drawing = mermaid::render("info", &Default::default(), &mermaid::Metrics).unwrap();
//! assert!(drawing.svg.starts_with("<svg"));
//! ```

use serde::Serialize;

pub mod config;
pub mod detect;
pub mod diagrams;
pub mod error;
pub mod geom;
pub mod js;
pub mod layout;
pub mod preprocess;
pub mod svg;
pub mod text;
mod token;
pub mod util;
pub mod yaml;

pub use config::{Config, Map, Value};
pub use detect::Kind;
pub use error::{Code, Error};
pub use svg::theme::{Colors, Rgba};
pub use text::{Host, Metrics};

/// [`PREFIX`] as a literal, for `concat!`.
macro_rules! prefix {
    () => {
        "bezel"
    };
}
pub(crate) use prefix;

/// ` class="bezel-<class>"` as a literal, and any attributes after it.
macro_rules! class {
    ($class:literal $(, $rest:literal)?) => {
        concat!(" class=\"", $crate::prefix!(), "-", $class, "\"" $(, " ", $rest)?)
    };
}
pub(crate) use class;

/// What class names, CSS variables and element ids start with.
pub const PREFIX: &str = prefix!();

/// How a diagram is drawn: Pele's `RenderOptions`. `Default` draws as Pele
/// does with no options.
#[derive(Clone)]
pub struct Options {
    /// The fonts labels are measured in, and drawn in where the page sets no
    /// `--bezel-font` or `--bezel-font-mono`.
    pub font_family: Option<String>,
    pub font_family_mono: Option<String>,
    pub font_size: f64,
    pub id_prefix: String,
    /// Whether the drawing scales down to its container. Unset, the
    /// diagram's `useMaxWidth` decides.
    pub responsive: Option<bool>,
    /// The width the host has, in px. A type that can draw itself narrower
    /// does, so its text is not shrunk with it.
    pub max_width: Option<f64>,
    /// Whether a diagram that runs across may be drawn running down when it
    /// does not fit `max_width`, and the width under which that is done.
    pub auto_direction: bool,
    pub direction_breakpoint: f64,
    /// Space around the drawing. Unset, each type's own.
    pub padding: Option<f64>,
    /// The longest source, in UTF-16 units. Mermaid's `maxTextSize` by
    /// default.
    pub limit: usize,
    /// The longest SVG, in UTF-16 units. Mermaid has no limit on its output.
    /// A source within the limit can still describe a drawing of many
    /// megabytes, which stalls what it is put into.
    pub output_limit: usize,
    /// The most edges a flowchart may have. Mermaid stops at 500.
    /// `A & B --> C & D` multiplies: without a limit, a few kilobytes of
    /// source ask for millions of edges.
    pub max_edges: usize,
    /// Schemes a link or an image may use besides a relative address.
    pub link_schemes: Vec<String>,
    pub image_schemes: Vec<String>,
    pub links: bool,
    pub images: bool,
    /// A link's `rel`. Empty leaves it out.
    pub link_rel: String,
    /// Configuration under the source's own, as Mermaid's site config.
    pub config: Map,
    /// The time gantt charts treat as now, in ms since the epoch. The
    /// clock's when unset.
    pub now: Option<f64>,
    /// Colours for a renderer without CSS custom properties: the SVG names
    /// them, and every other variable's value, in place of each `var(…)`,
    /// and keeps rectangles' radii in `rx`.
    pub colors: Option<Colors>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            font_family: None,
            font_family_mono: None,
            font_size: 16.0,
            id_prefix: PREFIX.to_string(),
            responsive: None,
            max_width: None,
            auto_direction: true,
            direction_breakpoint: 640.0,
            padding: None,
            limit: 50_000,
            output_limit: 4_000_000,
            max_edges: 5_000,
            link_schemes: util::url::LINK_SCHEMES.map(String::from).to_vec(),
            image_schemes: util::url::IMAGE_SCHEMES.map(String::from).to_vec(),
            links: true,
            images: true,
            link_rel: "noopener".to_string(),
            config: Map::new(),
            now: None,
            colors: None,
        }
    }
}

/// One render: the options it is drawn with and the host it is drawn for.
pub struct Ctx<'a, H> {
    pub options: &'a Options,
    pub host: &'a H,
    /// [`Options::responsive`], or the diagram's `useMaxWidth` where unset.
    pub responsive: bool,
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
    let limit = options.limit;
    if js::len_over(source, limit) {
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
    let mut config = options.config.clone();
    for (key, value) in pre.config {
        config.insert(key, value);
    }
    let config = Config::new(config);
    let text = if diagram.keep_comments() {
        &pre.with_comments
    } else {
        &pre.text
    };
    let encoded = preprocess::encode_entities(text) + "\n";
    let limits = ParseLimits {
        max_edges: options.max_edges,
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

/// `source` drawn as SVG for `host`.
pub fn render(source: &str, options: &Options, host: &impl Host) -> Result<Drawing, Error> {
    render_parsed(&parse(source, options)?, options, host)
}

/// A parsed source drawn as SVG for `host`.
pub fn render_parsed(
    parsed: &Parsed,
    options: &Options,
    host: &impl Host,
) -> Result<Drawing, Error> {
    let kind = parsed.kind;
    if diagrams::diagram(kind).is_none() {
        return Err(Error::new(
            format!("Diagram type \"{}\" is not supported yet.", kind.name()),
            Code::UnsupportedDiagram,
        )
        .of(kind));
    }
    let fixed = parsed.config.base(kind).and_then(|base| base.use_max_width) == Some(false);
    let ctx = Ctx {
        options,
        host,
        responsive: options.responsive.unwrap_or(!fixed),
    };
    let rendered = parsed.model.render(&parsed.config, &ctx)?;
    let output_limit = options.output_limit;
    if js::len_over(&rendered.svg, output_limit) {
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
