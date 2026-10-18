//! The `<svg>` element around a drawing. Pele's `svg/root.ts`.

use super::{
    builder::{esc, esc_text},
    theme::{RADIUS, ROOT_STYLE, font_style},
};
use crate::mermaid::{Options, js::Lazy};

/// A rectangle's corner radius comes from a variable. Browsers only lately
/// take one in `rx`, and all take one in a style, so that is where it goes,
/// first, for a radius the author set to come after it and win.
const ROUNDED: &str = " rx=\"var(--bezel-radius,4px)\"";

/// The weights of bold text, each set only in a diagram that has such text.
const WEIGHTS: [(&str, &str); 3] = [("--_tw", "title"), ("--_hw", "heading"), ("--_w", "bold")];

static RE_ROUNDED: Lazy = Lazy::new(r"<rect\b[^>]*>", "g");

fn rounded(content: &str) -> String {
    if !content.contains(ROUNDED) {
        return content.to_string();
    }
    RE_ROUNDED.replace(content, |caps| {
        let tag = caps.str(0);
        let Some(at) = tag.find(ROUNDED) else {
            return tag.to_string();
        };
        let rest = format!("{}{}", &tag[..at], &tag[at + ROUNDED.len()..]);
        if let Some(style) = rest.find(" style=\"") {
            return format!("{}rx:var(--_r);{}", &rest[..style + 8], &rest[style + 8..]);
        }
        let end = if rest.ends_with("/>") {
            rest.len() - 2
        } else {
            rest.len() - 1
        };
        format!("{} style=\"rx:var(--_r)\"{}", &rest[..end], &rest[end..])
    })
}

/// The title and description a screen reader announces.
#[derive(Clone, Debug, Default)]
pub struct Accessible<'a> {
    pub acc_title: Option<&'a str>,
    pub acc_descr: Option<&'a str>,
}

pub fn svg_document(
    kind: &str,
    width: f64,
    height: f64,
    font_size: f64,
    options: &Options,
    acc: Accessible,
    content: &str,
) -> String {
    let prefix = esc(options
        .id_prefix
        .as_deref()
        .unwrap_or(crate::mermaid::PREFIX));
    let mut head = String::new();
    let mut aria = String::new();
    if let Some(title) = acc.acc_title.filter(|title| !title.is_empty()) {
        head.push_str(&format!(
            "<title id=\"{prefix}-title\">{}</title>",
            esc_text(title)
        ));
        aria.push_str(&format!(" aria-labelledby=\"{prefix}-title\""));
    }
    if let Some(descr) = acc.acc_descr.filter(|descr| !descr.is_empty()) {
        head.push_str(&format!(
            "<desc id=\"{prefix}-desc\">{}</desc>",
            esc_text(descr)
        ));
        aria.push_str(&format!(" aria-describedby=\"{prefix}-desc\""));
    }
    let fit = if options.responsive == Some(false) {
        ""
    } else {
        "max-width:100%;height:auto;"
    };
    let content = rounded(content);
    let mut weights = if content.contains("var(--_r)") {
        format!(";--_r:{RADIUS}")
    } else {
        String::new()
    };
    for (alias, name) in WEIGHTS {
        if content.contains(&format!("var({alias})")) {
            weights.push_str(&format!(";{alias}:var(--bezel-{name}-weight,bold)"));
        }
    }
    let p = crate::mermaid::PREFIX;
    let w = crate::mermaid::js::num_str(width);
    let h = crate::mermaid::js::num_str(height);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"{p} {p}-{kind}\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" style=\"{fit}{ROOT_STYLE}{weights}{}\" fill=\"var(--_fg)\" role=\"graphics-document document\" aria-roledescription=\"{kind}\"{aria}>{head}{content}</svg>",
        esc(&font_style(
            font_size,
            options.font_family.as_deref(),
            options.font_family_mono.as_deref()
        )),
    )
}
