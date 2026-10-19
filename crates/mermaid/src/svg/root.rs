//! The `<svg>` element around a drawing. Pele's `svg/root.ts`.

use crate::svg::builder::{Esc, EscText};
use std::{fmt::Write as _, sync::LazyLock};

use memchr::memmem::Finder;

use super::theme::{RADIUS, ROOT_STYLE, push_font_style, resolve};
use crate::js;
use crate::{Ctx, Host};

/// A rectangle's corner radius comes from a variable. Browsers only lately
/// take one in `rx`, and all take one in a style, so that is where it goes,
/// first, for a radius the author set to come after it and win.
const ROUNDED: &str = " rx=\"var(--bezel-radius,4px)\"";
const STYLE: &str = " style=\"";

/// The weights of bold text, each set only in a diagram that has such text.
const WEIGHTS: [(&str, &str); 3] = [
    ("var(--_tw)", ";--_tw:var(--bezel-title-weight,bold)"),
    ("var(--_hw)", ";--_hw:var(--bezel-heading-weight,bold)"),
    ("var(--_w)", ";--_w:var(--bezel-bold-weight,bold)"),
];

/// Searchers for [`ROUNDED`], [`STYLE`], `var(--_r)` and the [`WEIGHTS`],
/// built once.
static FINDERS: LazyLock<Finders> = LazyLock::new(|| Finders {
    rounded: Finder::new(ROUNDED),
    style: Finder::new(STYLE),
    radius: Finder::new("var(--_r)"),
    weights: WEIGHTS.map(|(used, _)| Finder::new(used)),
});

struct Finders {
    rounded: Finder<'static>,
    style: Finder<'static>,
    radius: Finder<'static>,
    weights: [Finder<'static>; 3],
}

/// The tags whose radius moves into their style: the first radius in each
/// `<rect` tag, as `<rect\b[^>]*>` finds them, as (where the tag opens,
/// where its radius is, where it ends). A tag holds no `<`.
fn radii(content: &str) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
    let bytes = content.as_bytes();
    let mut tag_done = None;
    FINDERS.rounded.find_iter(bytes).filter_map(move |at| {
        let open = memchr::memrchr(b'<', &bytes[..at])?;
        let tag = &bytes[open..at];
        let bounded = bytes
            .get(open + 5)
            .is_some_and(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        if !tag.starts_with(b"<rect")
            || !bounded
            || memchr::memchr(b'>', tag).is_some()
            || tag_done == Some(open)
        {
            return None;
        }
        let close = memchr::memchr(b'>', &bytes[at..])?;
        tag_done = Some(open);
        Some((open, at, at + close + 1))
    })
}

/// `content` with each of its [`radii`] moved into the tag's style.
fn push_rounded(out: &mut String, content: &str) {
    let style_of = |text: &str| {
        FINDERS
            .style
            .find(text.as_bytes())
            .map(|at| at + STYLE.len())
    };
    let mut last = 0;
    for (open, at, end) in radii(content) {
        let after = &content[at + ROUNDED.len()..end];
        out.push_str(&content[last..open]);
        if let Some(style) = style_of(&content[open..at]) {
            let style = open + style;
            out.push_str(&content[open..style]);
            out.push_str("rx:var(--_r);");
            out.push_str(&content[style..at]);
            out.push_str(after);
        } else if let Some(style) = style_of(after) {
            out.push_str(&content[open..at]);
            out.push_str(&after[..style]);
            out.push_str("rx:var(--_r);");
            out.push_str(&after[style..]);
        } else {
            let cut = after.len() - if after.ends_with("/>") { 2 } else { 1 };
            out.push_str(&content[open..at]);
            out.push_str(&after[..cut]);
            out.push_str(" style=\"rx:var(--_r)\"");
            out.push_str(&after[cut..]);
        }
        last = end;
    }
    out.push_str(&content[last..]);
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
    ctx: &Ctx<'_, impl Host>,
    acc: Accessible,
    content: &str,
) -> String {
    let title = acc.acc_title.filter(|title| !title.is_empty());
    let descr = acc.acc_descr.filter(|descr| !descr.is_empty());
    let prefix = Esc(ctx.options.id_prefix.as_str());
    let fit = if !ctx.responsive {
        ""
    } else {
        "max-width:100%;height:auto;"
    };
    let p = crate::PREFIX;
    let mut out = String::with_capacity(content.len() + 1024);
    let (w, h) = (js::Number(width), js::Number(height));
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"{p} {p}-{kind}\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" style=\""
    );
    let style = out.len();
    out.push_str(fit);
    out.push_str(ROOT_STYLE);
    if radii(content).next().is_some() || FINDERS.radius.find(content.as_bytes()).is_some() {
        out.push_str(";--_r:");
        out.push_str(RADIUS);
    }
    for ((_, weight), used) in WEIGHTS.iter().zip(&FINDERS.weights) {
        if used.find(content.as_bytes()).is_some() {
            out.push_str(weight);
        }
    }
    push_font_style(
        &mut out,
        font_size,
        ctx.options.font_family.as_deref(),
        ctx.options.font_family_mono.as_deref(),
    );
    let style = style..out.len();
    let _ = write!(
        out,
        "\" fill=\"var(--_fg)\" role=\"graphics-document document\" aria-roledescription=\"{kind}\""
    );
    if title.is_some() {
        let _ = write!(out, " aria-labelledby=\"{prefix}-title\"");
    }
    if descr.is_some() {
        let _ = write!(out, " aria-describedby=\"{prefix}-desc\"");
    }
    out.push('>');
    if let Some(title) = title {
        let _ = write!(
            out,
            "<title id=\"{prefix}-title\">{}</title>",
            EscText(title)
        );
    }
    if let Some(descr) = descr {
        let _ = write!(out, "<desc id=\"{prefix}-desc\">{}</desc>", EscText(descr));
    }
    match &ctx.options.colors {
        Some(_) => out.push_str(content),
        None => push_rounded(&mut out, content),
    }
    out.push_str("</svg>");
    if let Some(colors) = &ctx.options.colors {
        return resolve(&out, &out[style], colors);
    }
    out
}
