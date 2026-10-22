//! The URLs a diagram links to and shows images from. Pele's `util/url.ts`,
//! after `@braintree/sanitize-url`, which Mermaid applies to click targets.

use crate::{string, util::chars};
use js::number::Number;
use regex::{Captures, Regex};
use std::{fmt, sync::LazyLock};

use crate::{
    Ctx, Host, Link,
    svg::builder::{Esc, esc, fragment},
    text::decode_entities,
};

const BLANK: &str = "about:blank";
pub(crate) const LINK_SCHEMES: [&str; 4] = ["http", "https", "mailto", "tel"];
pub(crate) const IMAGE_SCHEMES: [&str; 2] = ["http", "https"];

static RE_INVALID_PROTOCOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^0-9A-Z_a-z]*(?i-u:javascript|data|vbscript)").expect("pattern")
});
static RE_HTML_ENTITIES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&#([0-9A-Z_a-z]+);?").expect("pattern"));
static RE_HTML_CTRL_ENTITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i-u)&(?:newline|tab);").expect("pattern"));
static RE_CTRL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\x00-\x1F\x7F-\x{9F}\x{2000}-\x{200D}\x{FEFF}]").expect("pattern")
});
static RE_WS_ESCAPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\\|%5[cC])(?:%(?:6[eE]|72|74)|[nrt])").expect("pattern"));
static RE_SCHEME_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^((?i-u)[a-z][a-z0-9+.-]*):").expect("pattern"));

/// The scheme part of `text`, as `/^.+(:|&colon;)/im` finds it: on the first
/// line that has one, through its last `:` or `&colon;` after the line's
/// first character.
fn scheme(text: &str) -> Option<&str> {
    text.split(chars::is_line_end).find_map(|line| {
        (1..line.len())
            .rev()
            .filter(|&at| line.is_char_boundary(at))
            .find_map(|at| {
                if line[at..].starts_with(':') {
                    Some(at + 1)
                } else {
                    chars::starts_with_caseless(line, at, "&colon;").then_some(at + 7)
                }
            })
            .map(|end| &line[..end])
    })
}

/// `decodeURIComponent`, or the text as it is where that would throw.
fn decode(uri: &str) -> String {
    let bytes = uri.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = uri
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            match hex {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                }
                None => return uri.to_string(),
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| uri.to_string())
}

pub fn sanitize_url(url: &str) -> String {
    if url.is_empty() {
        return BLANK.into();
    }
    let mut decoded = decode(string::trim(url));
    let mut rounds = 0;
    loop {
        // Each round peels one layer of encoding. A URL wrapped this many
        // times is not a real one.
        rounds += 1;
        if rounds > 32 {
            return BLANK.into();
        }
        let step = RE_CTRL.replace_all(&decoded, "");
        let step = RE_HTML_ENTITIES.replace_all(&step, |caps: &Captures| {
            let code = Number::from(&caps[1]).0;
            char::from_u32(if code.is_finite() {
                code as u32 & 0xffff
            } else {
                0
            })
            .map_or(String::new(), |c| c.to_string())
        });
        let step = RE_HTML_CTRL_ENTITY.replace_all(&step, "");
        let step = RE_CTRL.replace_all(&step, "");
        let step = RE_WS_ESCAPE.replace_all(&step, "");
        decoded = decode(string::trim(&step));
        let more = RE_CTRL.is_match(&decoded)
            || RE_HTML_ENTITIES.is_match(&decoded)
            || RE_HTML_CTRL_ENTITY.is_match(&decoded)
            || RE_WS_ESCAPE.is_match(&decoded);
        if !more {
            break;
        }
    }

    if decoded.is_empty() {
        return BLANK.into();
    }
    if decoded.starts_with(['.', '/']) {
        return decoded;
    }

    let trimmed = string::trim_start(&decoded);
    let Some(scheme) = scheme(trimmed) else {
        return decoded;
    };
    let protocol = string::trim(&scheme.to_lowercase()).to_string();
    if RE_INVALID_PROTOCOL.is_match(&protocol) {
        return BLANK.into();
    }

    let normalized = trimmed.replace('\\', "/");
    if protocol == "mailto:" || protocol.contains("://") {
        return normalized;
    }
    if protocol == "http:" || protocol == "https:" {
        return special(&normalized, &protocol).unwrap_or_else(|| BLANK.into());
    }
    normalized
}

/// What `new URL(url).toString()` makes of an `http:` or `https:` URL whose
/// scheme is not followed by `//`: the slashes added, the host in lower
/// case, an empty path as `/`. `None` where `new URL` throws.
fn special(url: &str, protocol: &str) -> Option<String> {
    let rest = url.get(protocol.len()..)?.trim_start_matches('/');
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].to_lowercase();
    if host.is_empty() || host.contains(char::is_whitespace) {
        return None;
    }
    let tail = &rest[end..];
    let tail = if tail.is_empty() || !tail.starts_with('/') {
        format!("/{tail}")
    } else {
        tail.to_string()
    };
    Some(format!("{protocol}//{host}{tail}"))
}

/// A URL as written in a diagram: entity codes are read first, so they cannot
/// hide a scheme.
pub fn safe_url(url: &str) -> String {
    sanitize_url(&decode_entities(url))
}

/// Stricter than Mermaid, which lets any scheme but three through: a diagram
/// in a shared note could otherwise open a local file, a network share, or
/// another application.
fn allow(url: &str, schemes: &[String]) -> Option<String> {
    if url.starts_with(['\\', '/']) && url[1..].starts_with(['\\', '/']) {
        return None;
    }
    match RE_SCHEME_NAME.captures(url) {
        None => Some(url.to_string()),
        Some(scheme) => {
            let name = scheme[1].to_lowercase();
            schemes.contains(&name).then(|| url.to_string())
        }
    }
}

/// A URL [`sanitize_url`] passed, kept only when it is relative or its scheme
/// is one the host allows.
pub fn link_url(url: &str, ctx: &Ctx<'_, impl Host>) -> Option<String> {
    if !ctx.options.links {
        return None;
    }
    allow(url, &ctx.options.link_schemes)
}

pub fn image_url(url: &str, ctx: &Ctx<'_, impl Host>) -> Option<String> {
    if !ctx.options.images {
        return None;
    }
    allow(url, &ctx.options.image_schemes)
}

/// `body` inside a link to `url`, or `body` alone where there is no `url` or
/// the link is refused. The link is added to `links` when this is called.
pub fn linked<'b, B: fmt::Display + 'b, A: fmt::Display + 'b>(
    body: B,
    url: Option<&str>,
    id: &str,
    links: &mut Vec<Link>,
    ctx: &'b Ctx<'_, impl Host>,
    attrs: A,
) -> impl fmt::Display + 'b {
    let href = url.and_then(|url| link_url(url, ctx));
    if let Some(href) = &href {
        links.push(Link {
            id: id.to_string(),
            href: href.clone(),
            internal: false,
        });
    }
    let rel = rel_attr(ctx);
    fragment(move |f| match &href {
        Some(href) => write!(f, "<a href=\"{}\"{attrs}{rel}>{body}</a>", Esc(href)),
        None => write!(f, "{body}"),
    })
}

/// An internal link to note `name`: the anchor goes around the label, with
/// `data-href` naming the note and an `href` only where the name passes as a
/// URL.
pub struct NoteLink {
    target: String,
    safe: bool,
}

impl NoteLink {
    /// Whether `classes`, as [`crate::svg::theme::class_names`] writes them,
    /// mark a label as a note name.
    pub fn marked(classes: &str) -> bool {
        classes
            .split(' ')
            .skip(1)
            .any(|class| class == "internal-link")
    }

    /// The link from element `id` to note `name`, added to `links`.
    pub fn new(name: String, id: &str, links: &mut Vec<Link>, ctx: &Ctx<'_, impl Host>) -> Self {
        let safe = link_url(&sanitize_url(&name), ctx).is_some();
        let target = esc(&name);
        links.push(Link {
            id: id.to_string(),
            href: name,
            internal: true,
        });
        Self { target, safe }
    }

    /// `text` inside this link's anchor.
    pub fn around<'b>(
        &'b self,
        text: impl fmt::Display + 'b,
        ctx: &'b Ctx<'_, impl Host>,
    ) -> impl fmt::Display + 'b {
        let rel = rel_attr(ctx);
        fragment(move |f| {
            let target = &self.target;
            f.write_str("<a class=\"internal-link\"")?;
            if self.safe {
                write!(f, " href=\"{target}\"{rel}")?;
            }
            write!(f, " data-href=\"{target}\">{text}</a>")
        })
    }
}

/// A link's `rel` attribute, or nothing where the host gives none.
pub fn rel_attr<'b>(ctx: &'b Ctx<'_, impl Host>) -> impl fmt::Display + 'b {
    let rel = ctx.options.link_rel.as_str();
    fragment(move |f| {
        if rel.is_empty() {
            Ok(())
        } else {
            write!(f, " rel=\"{}\"", Esc(rel))
        }
    })
}
