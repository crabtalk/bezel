//! The gallery's link previews, which `markdown` asks for its bookmark blocks.
//!
//! A favicon needs only the host, so every link gets one from a service. A
//! picture and a blurb need the page itself — an HTTP fetch and an HTML parse,
//! which a component gallery has no business carrying — so those come from a
//! table, which is what an app's own cache holds before its fetch lands. A URL
//! the table has never heard of paints its favicon and its host, and that is
//! the honest half-resolved card.
//!
//! The pictures are URLs rather than bundled assets because a card fetching one
//! is the case worth showing: gpui routes a URL-shaped string to
//! `Resource::Uri`, and `with_fallback` covers the times it does not arrive.

use markdown::Preview;

/// URL prefix, title, blurb, `og:image` — the rows a real cache would have
/// filled in by now.
const KNOWN: &[(&str, &str, &str, &str)] = &[(
    "https://crabtalk.ai",
    "crabtalk",
    "Always something worth saying.",
    "https://crabtalk.ai/og-home.png",
)];

/// Install with `markdown::set_link_preview(cx, preview::of)`.
pub fn of(url: &str) -> Option<Preview> {
    let after = url.split_once("://")?.1;
    let host = after.split(['/', '?', '#']).next()?;
    let host = host.strip_prefix("www.").unwrap_or(host);

    let mut preview = Preview {
        icon: Some(format!("https://icons.duckduckgo.com/ip3/{host}.ico").into()),
        label: Some(host.to_string().into()),
        ..Preview::default()
    };

    // A repository's own card, which GitHub renders on demand — and the path
    // names the unit here, which the bare host does not.
    if let Some((owner, rest)) = url
        .strip_prefix("https://github.com/")
        .and_then(|repo| repo.split_once('/'))
    {
        let name = rest.split('/').next().unwrap_or(rest);
        preview.title = Some(name.to_string().into());
        preview.label = Some(format!("{owner}/{name}").into());
        preview.image = Some(format!("https://opengraph.githubassets.com/1/{owner}/{name}").into());
        return Some(preview);
    }

    if let Some((_, title, blurb, image)) = KNOWN.iter().find(|(at, ..)| url.starts_with(at)) {
        preview.title = Some((*title).into());
        preview.description = Some((*blurb).into());
        preview.image = Some((*image).into());
    }
    Some(preview)
}
