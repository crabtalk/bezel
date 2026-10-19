//! The URLs a diagram links to and shows images from. Pele's `util/url.ts`,
//! after `@braintree/sanitize-url`, which Mermaid applies to click targets.

use crate::{Ctx, Host};
use crate::{
    Link,
    js::{self, Lazy},
    svg::builder::esc,
    text::decode_entities,
};

const BLANK: &str = "about:blank";
pub(crate) const LINK_SCHEMES: [&str; 4] = ["http", "https", "mailto", "tel"];
pub(crate) const IMAGE_SCHEMES: [&str; 2] = ["http", "https"];

static RE_INVALID_PROTOCOL: Lazy = Lazy::new(r"^([^\w]*)(javascript|data|vbscript)", "im");
static RE_HTML_ENTITIES: Lazy = Lazy::new(r"&#(\w+)(^\w|;)?", "g");
static RE_HTML_CTRL_ENTITY: Lazy = Lazy::new(r"&(newline|tab);", "gi");
static RE_CTRL: Lazy = Lazy::new(
    "[\\u0000-\\u001F\\u007F-\\u009F\\u2000-\\u200D\\uFEFF]",
    "gim",
);
static RE_SCHEME: Lazy = Lazy::new(r"^.+(:|&colon;)", "gim");
static RE_WS_ESCAPE: Lazy = Lazy::new(r"(\\|%5[cC])((%(6[eE]|72|74))|[nrt])", "g");
static RE_SCHEME_NAME: Lazy = Lazy::new(r"^([a-z][a-z0-9+.-]*):", "i");
static RE_NETWORK_PATH: Lazy = Lazy::new(r"^[\\/]{2}", "");

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
    let mut decoded = decode(js::trim(url));
    let mut rounds = 0;
    loop {
        // Each round peels one layer of encoding. A URL wrapped this many
        // times is not a real one.
        rounds += 1;
        if rounds > 32 {
            return BLANK.into();
        }
        let step = RE_CTRL.replace(&decoded, |_| String::new());
        let step = RE_HTML_ENTITIES.replace(&step, |caps| {
            let code = js::number(caps.str(1));
            char::from_u32(if code.is_finite() {
                code as u32 & 0xffff
            } else {
                0
            })
            .map_or(String::new(), |c| c.to_string())
        });
        let step = RE_HTML_CTRL_ENTITY.replace(&step, |_| String::new());
        let step = RE_CTRL.replace(&step, |_| String::new());
        let step = RE_WS_ESCAPE.replace(&step, |_| String::new());
        decoded = decode(js::trim(&step));
        let more = RE_CTRL.test(&decoded)
            || RE_HTML_ENTITIES.test(&decoded)
            || RE_HTML_CTRL_ENTITY.test(&decoded)
            || RE_WS_ESCAPE.test(&decoded);
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

    let trimmed = js::trim_start(&decoded);
    let Some(scheme) = RE_SCHEME.exec(trimmed) else {
        return decoded;
    };
    let protocol = js::trim(&scheme.str(0).to_lowercase()).to_string();
    if RE_INVALID_PROTOCOL.test(&protocol) {
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
    if RE_NETWORK_PATH.test(url) {
        return None;
    }
    match RE_SCHEME_NAME.exec(url) {
        None => Some(url.to_string()),
        Some(scheme) => {
            let name = scheme.str(1).to_lowercase();
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

/// `body` inside a link to `url`, or `body` alone where the link is refused.
pub fn linked(
    body: &str,
    url: &str,
    id: &str,
    links: &mut Vec<Link>,
    ctx: &Ctx<'_, impl Host>,
    attrs: &str,
) -> String {
    let Some(href) = link_url(url, ctx) else {
        return body.to_string();
    };
    links.push(Link {
        id: id.to_string(),
        href: href.clone(),
        internal: false,
    });
    format!(
        "<a href=\"{}\"{attrs}{}>{body}</a>",
        esc(&href),
        rel_attr(ctx)
    )
}

pub fn rel_attr(ctx: &Ctx<'_, impl Host>) -> String {
    let rel = ctx.options.link_rel.as_str();
    if rel.is_empty() {
        String::new()
    } else {
        format!(" rel=\"{}\"", esc(rel))
    }
}
