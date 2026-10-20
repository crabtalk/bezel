//! The colours, fonts and user styles of a drawing, as CSS. Pele's
//! `svg/theme.ts`, with `--bezel-*` for `--pele-*`.

use crate::svg::builder::Esc;
use std::{borrow::Cow, collections::HashSet, fmt::Write as _};

use super::builder::{esc, push_esc};
use crate::js::{self, Lazy};

/// Private aliases set once on the root element, so each use is short and
/// the public `--bezel-*` tokens keep their fallbacks. The text colour is set
/// too: an icon is drawn in `currentColor`.
pub const ROOT_STYLE: &str = "--_bg:var(--bezel-bg,#fff);--_fg:var(--bezel-fg,#1f1f1f);--_m:var(--bezel-muted,#6e6e6e);--_l:var(--bezel-line,#1f1f1f);--_s:var(--bezel-surface,#f3f3f3);--_a:var(--bezel-surface-alt,#e6e6e6);--_b:var(--bezel-border,#8a8a8a);--_c:var(--bezel-accent,#5b7bd5);color:var(--_fg)";

pub const FONT_MONO: &str = "var(--_fm)";
const COLOR_MIX: &str = "color-mix(in srgb,";
/// How deep a variable may be declared through others.
const NESTED: usize = 8;
pub const RADIUS: &str = "var(--bezel-radius,4px)";
/// What a diagram marks as critical. Not a series colour, as a theme may put
/// those in any order.
pub const CRITICAL: &str = "var(--bezel-critical,#e45756)";

const SERIES: [&str; 8] = [
    "#4c78a8", "#f58518", "#54a24b", "#e45756", "#72b7b2", "#eeca3b", "#b279a2", "#9d755d",
];

const SHAPE_PROPS: [&str; 12] = [
    "fill",
    "fill-opacity",
    "stroke",
    "stroke-width",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-opacity",
    "opacity",
    "rx",
    "ry",
];

const TEXT_PROPS: [&str; 9] = [
    "color",
    "font-size",
    "font-family",
    "font-weight",
    "font-style",
    "text-decoration",
    "letter-spacing",
    "word-spacing",
    "opacity",
];

/// A list of font names, quoted or not, and nothing that could end the
/// declaration it is put in.
static RE_FAMILIES: Lazy = Lazy::new(
    r#"^\s*(?:"[^"'\\\n<>{}();:@]*"|'[^"'\\\n<>{}();:@]*'|[\p{L}\p{N}_-][\p{L}\p{N}_ .-]*)(?:\s*,\s*(?:"[^"'\\\n<>{}();:@]*"|'[^"'\\\n<>{}();:@]*'|[\p{L}\p{N}_-][\p{L}\p{N}_ .-]*))*\s*$"#,
    "u",
);
static RE_UNSAFE: Lazy = Lazy::new(
    r"url\s*\(|expression\s*\(|javascript:|[<>{};\\]|@import|image-set|\/\*",
    "i",
);
static RE_IMPORTANT: Lazy = Lazy::new(r"!important", "gi");
static RE_PX: Lazy = Lazy::new(r"^[\d.]+(px)?$", "");
static RE_CLASS: Lazy = Lazy::new(r"[^\w-]+", "g");

fn families<'a>(list: Option<&'a str>, generic: &'a str) -> &'a str {
    match list {
        Some(list) if !list.is_empty() && RE_FAMILIES.test(list) => list,
        _ => generic,
    }
}

/// The fonts the labels were measured in are the ones they are drawn in,
/// unless the page names others. The rest of what text inherits is pinned,
/// as none of it was measured.
pub fn push_font_style(out: &mut String, size: f64, family: Option<&str>, mono: Option<&str>) {
    out.push_str(";--_fm:var(--bezel-font-mono,");
    push_esc(out, families(mono, "monospace"));
    out.push_str(");font:");
    let _ = write!(out, "{}", js::Number(size));
    out.push_str("px sans-serif;letter-spacing:normal;word-spacing:normal;text-transform:none;direction:ltr;font-family:var(--bezel-font,");
    push_esc(out, families(family, "sans-serif"));
    out.push(')');
}

/// Mermaid style declarations, split into what applies to a shape, to a
/// line, and to text, each an ` style="…"` attribute or `""`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedStyle {
    pub shape: String,
    pub line: String,
    pub text: String,
    pub stroke: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub font_size: Option<f64>,
}

impl ResolvedStyle {
    /// The colour of a line in this style and of its arrowheads.
    pub fn line_color(&self) -> Cow<'static, str> {
        match self.stroke.as_deref().filter(|stroke| !stroke.is_empty()) {
            Some(stroke) => Cow::Owned(esc(stroke)),
            None => Cow::Borrowed("var(--_l)"),
        }
    }

    /// The [`Style`](crate::text::Style) bits of its weight and slant.
    pub fn label_style(&self) -> u8 {
        let bold = if self.bold {
            crate::text::Style::BOLD
        } else {
            0
        };
        let italic = if self.italic {
            crate::text::Style::ITALIC
        } else {
            0
        };
        bold | italic
    }
}

pub fn resolve_style<S: AsRef<str>>(declarations: &[S]) -> ResolvedStyle {
    if declarations.is_empty() {
        return ResolvedStyle::default();
    }
    let shape_props: HashSet<&str> = SHAPE_PROPS.into_iter().collect();
    let text_props: HashSet<&str> = TEXT_PROPS.into_iter().collect();
    let mut shape: Vec<(String, String)> = Vec::new();
    let mut text: Vec<(String, String)> = Vec::new();
    let set = |list: &mut Vec<(String, String)>, prop: String, value: String| match list
        .iter_mut()
        .find(|(p, _)| *p == prop)
    {
        Some(slot) => slot.1 = value,
        None => list.push((prop, value)),
    };
    for decl in declarations {
        let decl = decl.as_ref();
        let Some(at) = decl.find(':').filter(|&at| at > 0) else {
            continue;
        };
        let prop = js::trim(&decl[..at]).to_lowercase();
        let value = RE_IMPORTANT.replace(&decl[at + 1..], |_| String::new());
        let value = js::trim(&value).to_string();
        if value.is_empty() || RE_UNSAFE.test(&value) {
            continue;
        }
        if shape_props.contains(prop.as_str()) {
            set(&mut shape, prop.clone(), value.clone());
        }
        if text_props.contains(prop.as_str()) {
            let key = if prop == "color" {
                "fill".to_string()
            } else {
                prop
            };
            set(&mut text, key, value);
        }
    }
    if shape.is_empty() && text.is_empty() {
        return ResolvedStyle::default();
    }
    let mut shape_css = String::new();
    let mut line_css = String::new();
    for (prop, value) in &shape {
        let _ = write!(shape_css, "{prop}:{value};");
        if prop != "fill" && prop != "fill-opacity" {
            let _ = write!(line_css, "{prop}:{value};");
        }
    }
    let mut text_css = String::new();
    for (prop, value) in &text {
        let _ = write!(text_css, "{prop}:{value};");
    }
    let get = |list: &[(String, String)], key: &str| {
        list.iter().find(|(p, _)| p == key).map(|(_, v)| v.clone())
    };
    let weight = get(&text, "font-weight");
    let size = get(&text, "font-size");
    let px = size
        .as_deref()
        .filter(|size| RE_PX.test(size))
        .map(js::parse_float);
    let attr = |css: &str| {
        if css.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", Esc(css))
        }
    };
    ResolvedStyle {
        shape: attr(&shape_css),
        line: attr(&line_css),
        text: attr(&text_css),
        stroke: get(&shape, "stroke"),
        bold: matches!(weight.as_deref(), Some("bold" | "bolder"))
            || weight.as_deref().is_some_and(|w| js::number(w) >= 600.0),
        italic: get(&text, "font-style").as_deref() == Some("italic"),
        font_size: px.filter(|&px| px > 0.0 && px <= 200.0),
    }
}

/// `raw` as class names: anything but word characters and `-` separates
/// them. ` a b`, or `""` for none.
pub fn class_names(raw: &str) -> String {
    let cleaned = RE_CLASS.replace(raw, |_| " ".to_string());
    let cleaned = js::trim(&cleaned);
    if cleaned.is_empty() {
        String::new()
    } else {
        format!(" {cleaned}")
    }
}

/// The colour for the `index`th data series in a chart, cycling through
/// eight tokens.
pub fn series_color(index: i64) -> SeriesColor {
    SeriesColor(index.rem_euclid(8) as usize)
}

/// A [`series_color`], written where it is formatted.
#[derive(Clone, Copy, Debug)]
pub struct SeriesColor(usize);

impl std::fmt::Display for SeriesColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "var(--bezel-series-{},{})", self.0 + 1, SERIES[self.0])
    }
}

/// An sRGB colour with alpha, each channel 0–255.
pub type Rgba = [u8; 4];

/// The colours a drawing names in place of its `--bezel-*` variables, for a
/// renderer without CSS custom properties. Each is the token of that name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Colors {
    pub bg: Rgba,
    pub fg: Rgba,
    pub muted: Rgba,
    pub line: Rgba,
    pub surface: Rgba,
    pub surface_alt: Rgba,
    pub border: Rgba,
    pub accent: Rgba,
    pub critical: Rgba,
    /// `--bezel-series-1` to `--bezel-series-8`.
    pub series: [Rgba; 8],
}

impl Colors {
    /// The colour of token `--bezel-{name}`, if it is one.
    fn token(&self, name: &str) -> Option<Rgba> {
        Some(match name {
            "bg" => self.bg,
            "fg" => self.fg,
            "muted" => self.muted,
            "line" => self.line,
            "surface" => self.surface,
            "surface-alt" => self.surface_alt,
            "border" => self.border,
            "accent" => self.accent,
            "critical" => self.critical,
            _ => {
                let index: usize = name.strip_prefix("series-")?.parse().ok()?;
                *self.series.get(index.checked_sub(1)?)?
            }
        })
    }
}

fn push_color(out: &mut String, [r, g, b, a]: Rgba) {
    let _ = write!(out, "#{r:02x}{g:02x}{b:02x}");
    if a != 255 {
        let _ = write!(out, "{a:02x}");
    }
}

/// A colour [`push_color`] wrote, or a CSS hex colour.
fn read_color(text: &str) -> Option<Rgba> {
    let hex = text.trim().strip_prefix('#')?;
    let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    let short = |i: usize| Some(u8::from_str_radix(hex.get(i..=i)?, 16).ok()? * 17);
    match hex.len() {
        3 => Some([short(0)?, short(1)?, short(2)?, 255]),
        6 => Some([channel(0)?, channel(2)?, channel(4)?, 255]),
        8 => Some([channel(0)?, channel(2)?, channel(4)?, channel(6)?]),
        _ => None,
    }
}

/// CSS `color-mix(in srgb, a p%, b)`, alpha premultiplied.
fn mix(a: Rgba, p: f64, b: Rgba) -> Rgba {
    let (wa, wb) = (p / 100.0, 1.0 - p / 100.0);
    let alpha = f64::from(a[3]) / 255.0 * wa + f64::from(b[3]) / 255.0 * wb;
    let mut out = [0, 0, 0, (alpha * 255.0).round() as u8];
    if alpha > 0.0 {
        for (i, channel) in out.iter_mut().take(3).enumerate() {
            let sum = f64::from(a[i]) * f64::from(a[3]) / 255.0 * wa
                + f64::from(b[i]) * f64::from(b[3]) / 255.0 * wb;
            *channel = (sum / alpha).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// `svg` with every `var(…)` replaced by its value and every `color-mix` by
/// its colour. A `--_*` alias is read from `root`, the root element's style;
/// a `--bezel-*` token from `colors`, else its fallback.
pub(crate) fn resolve(svg: &str, root: &str, colors: &Colors) -> String {
    let resolver = Resolver {
        declared: root
            .split(';')
            .filter_map(|declaration| declaration.split_once(':'))
            .filter(|(name, _)| name.starts_with("--"))
            .collect(),
        colors,
    };
    let mut vars = String::with_capacity(svg.len());
    resolver.substitute(svg, &mut vars, 0);
    mixed(&vars)
}

struct Resolver<'a> {
    declared: Vec<(&'a str, &'a str)>,
    colors: &'a Colors,
}

impl Resolver<'_> {
    /// `text` into `out`, each `var(name[,fallback])` replaced by its value.
    fn substitute(&self, text: &str, out: &mut String, depth: usize) {
        let mut rest = text;
        while let Some(at) = rest.find("var(--") {
            out.push_str(&rest[..at]);
            let inner = &rest[at + 4..];
            // The closing parenthesis, past any nested in the fallback.
            let mut level = 0usize;
            let Some(close) = inner.char_indices().find_map(|(i, c)| match c {
                '(' => {
                    level += 1;
                    None
                }
                ')' if level == 0 => Some(i),
                ')' => {
                    level -= 1;
                    None
                }
                _ => None,
            }) else {
                out.push_str(&rest[at..]);
                return;
            };
            let (name, fallback) = match inner[..close].split_once(',') {
                Some((name, fallback)) => (name.trim(), Some(fallback.trim())),
                None => (inner[..close].trim(), None),
            };
            self.value(name, fallback, out, depth);
            rest = &inner[close + 1..];
        }
        out.push_str(rest);
    }

    /// The value of `var(name, fallback)` into `out`.
    fn value(&self, name: &str, fallback: Option<&str>, out: &mut String, depth: usize) {
        if let Some(token) = name.strip_prefix("--bezel-")
            && let Some(color) = self.colors.token(token)
        {
            push_color(out, color);
        } else if let Some((_, value)) =
            self.declared.iter().find(|(declared, _)| *declared == name)
            && depth < NESTED
        {
            self.substitute(value, out, depth + 1);
        } else if let Some(fallback) = fallback {
            self.substitute(fallback, out, depth + 1);
        }
    }
}

/// `text` with each `color-mix(in srgb,a p%,b)` of colours replaced by the
/// colour it names.
fn mixed(text: &str) -> String {
    if !text.contains(COLOR_MIX) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(COLOR_MIX) {
        out.push_str(&rest[..at]);
        let inner = &rest[at + COLOR_MIX.len()..];
        let parsed = inner.find(')').and_then(|close| {
            let (first, second) = inner[..close].split_once(',')?;
            let (color, percent) = first.trim().rsplit_once(' ')?;
            let percent: f64 = percent.strip_suffix('%')?.parse().ok()?;
            Some((mix(read_color(color)?, percent, read_color(second)?), close))
        });
        match parsed {
            Some((color, close)) => {
                push_color(&mut out, color);
                rest = &inner[close + 1..];
            }
            None => {
                out.push_str(COLOR_MIX);
                rest = inner;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `color` as a shape's fill, in the ` style="…"` attribute
/// [`resolve_style`] writes, or `""`.
pub fn fill_style(color: &str) -> String {
    resolve_style(&[format!("fill:{color}")]).shape
}

/// [`resolve_style`] of `base` followed by `over`.
pub fn resolve_style_over(base: &[String], over: &[String]) -> ResolvedStyle {
    if over.is_empty() {
        return resolve_style(base);
    }
    let mut all = base.to_vec();
    all.extend_from_slice(over);
    resolve_style(&all)
}

static RE_DEFAULT_CLASS: Lazy = Lazy::new(r"^default\s?", "");

/// [`class_names`] of a node's `raw` classes, less the `default` class they
/// start with.
pub fn node_class_names(raw: &str) -> String {
    class_names(&RE_DEFAULT_CLASS.replace(raw, |_| String::new()))
}
