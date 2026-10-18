//! The colours, fonts and user styles of a drawing, as CSS. Pele's
//! `svg/theme.ts`, with `--bezel-*` for `--pele-*`.

use std::collections::HashSet;

use super::builder::esc;
use crate::mermaid::js::{self, Lazy};

/// Private aliases set once on the root element, so each use is short and
/// the public `--bezel-*` tokens keep their fallbacks. The text colour is set
/// too: an icon is drawn in `currentColor`.
pub const ROOT_STYLE: &str = "--_bg:var(--bezel-bg,#fff);--_fg:var(--bezel-fg,#1f1f1f);--_m:var(--bezel-muted,#6e6e6e);--_l:var(--bezel-line,#1f1f1f);--_s:var(--bezel-surface,#f3f3f3);--_a:var(--bezel-surface-alt,#e6e6e6);--_b:var(--bezel-border,#8a8a8a);--_c:var(--bezel-accent,#5b7bd5);color:var(--_fg)";

pub const FONT_MONO: &str = "var(--_fm)";
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
pub fn font_style(size: f64, family: Option<&str>, mono: Option<&str>) -> String {
    format!(
        ";--_fm:var(--bezel-font-mono,{});font:{}px sans-serif;letter-spacing:normal;word-spacing:normal;text-transform:none;direction:ltr;font-family:var(--bezel-font,{})",
        families(mono, "monospace"),
        js::num_str(size),
        families(family, "sans-serif"),
    )
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
        shape_css.push_str(&format!("{prop}:{value};"));
        if prop != "fill" && prop != "fill-opacity" {
            line_css.push_str(&format!("{prop}:{value};"));
        }
    }
    let mut text_css = String::new();
    for (prop, value) in &text {
        text_css.push_str(&format!("{prop}:{value};"));
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
            format!(" style=\"{}\"", esc(css))
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
pub fn series_color(index: i64) -> String {
    let k = index.rem_euclid(8) as usize;
    format!("var(--bezel-series-{},{})", k + 1, SERIES[k])
}
