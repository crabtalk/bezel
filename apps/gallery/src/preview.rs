//! The gallery's link previews, which `markdown` asks for its bookmark blocks.
//!
//! A real app resolves these over the network and caches them by URL. A gallery
//! has none, so the one link on the document page answers from a table and
//! every other URL answers `None` — which is exactly the card an app paints
//! while its own fetch is still in flight.

use markdown::Preview;

/// Install with `markdown::set_link_preview(cx, preview::of)`.
pub fn of(url: &str) -> Option<Preview> {
    (url == "https://bezel.rs").then(|| Preview {
        title: Some("bezel".into()),
        description: Some(
            "A gpui component library, SwiftUI-lean: style flows through the environment, never through parameters."
                .into(),
        ),
        label: Some("crabtalk/bezel".into()),
        image: None,
        icon: None,
    })
}
