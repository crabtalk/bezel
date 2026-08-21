//! Who describes a link.
//!
//! `markdown` fetches nothing. Resolving a URL's Open Graph data is an HTTP
//! request and an HTML parse, neither of which a component library has any
//! business carrying, and `wasm32-unknown-unknown` would spell both differently
//! anyway. Installed once at boot like the highlighter and read at paint: the
//! app answers from its own cache and notifies when a fetch lands.

use gpui::{App, Global, SharedString};

/// What a bookmark paints beyond the URL it already has.
///
/// Every field is optional because a preview arrives in pieces, and a card
/// holding none of them still shows its host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    pub title: Option<SharedString>,
    pub description: Option<SharedString>,
    pub image: Option<SharedString>,
    pub icon: Option<SharedString>,
    /// The footer's identity where the host is not the most specific one — a
    /// repository, a subreddit. Which path names a *unit* is the app's
    /// knowledge, not this crate's.
    pub label: Option<SharedString>,
}

/// `None` for a URL the caller has nothing for *yet*: the card paints its host
/// and repaints when the answer arrives.
pub type LinkPreview = fn(url: &str) -> Option<Preview>;

struct Installed(LinkPreview);

impl Global for Installed {}

/// `markdown::set_link_preview(cx, my_previews)` — call once at boot. Without
/// it a bookmark shows its host and its URL, which is what a link looks like
/// before anyone has resolved it.
pub fn set_link_preview(cx: &mut App, preview: LinkPreview) {
    cx.set_global(Installed(preview));
}

pub(crate) fn of(cx: &App, url: &str) -> Option<Preview> {
    (cx.try_global::<Installed>()?.0)(url)
}

/// The host, without its `www.` — all a card can say about a URL nobody has
/// resolved.
pub(crate) fn host(url: &str) -> &str {
    let after = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = after.split(['/', '?', '#']).next().unwrap_or(after);
    host.strip_prefix("www.").unwrap_or(host)
}
