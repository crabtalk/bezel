//! Who describes a link.
//!
//! `markdown` fetches nothing. Resolving a URL's Open Graph data is an HTTP
//! request and an HTML parse, neither of which a component library has any
//! business carrying, and `wasm32-unknown-unknown` would spell both differently
//! anyway. The app answers [`crate::Links::preview`] from its own cache and
//! notifies when a fetch lands.

use gpui::SharedString;

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
    /// An app's own mark for a link it owns, painted in the text colour where
    /// a favicon would go.
    pub glyph: Option<ui::icons::Icon>,
}

/// The host, without its `www.` — all a card can say about a URL nobody has
/// resolved.
pub(crate) fn host(url: &str) -> &str {
    let after = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = after.split(['/', '?', '#']).next().unwrap_or(after);
    host.strip_prefix("www.").unwrap_or(host)
}
