//! Who opens a link.
//!
//! Installed once at boot like the highlighter, and read at click. Without it a
//! link goes to [`App::open_url`].

use gpui::{App, Global, Window};

/// Opens a link a reader clicked: an inline link or a bookmark card.
pub type LinkHandler = fn(url: &str, &mut Window, &mut App);

struct Installed(LinkHandler);

impl Global for Installed {}

/// `markdown::set_link_handler(cx, my_links)` — call once at boot.
pub fn set_link_handler(cx: &mut App, handler: LinkHandler) {
    cx.set_global(Installed(handler));
}

pub(crate) fn open(url: &str, window: &mut Window, cx: &mut App) {
    match cx.try_global::<Installed>().map(|installed| installed.0) {
        Some(handler) => handler(url, window, cx),
        None => cx.open_url(url),
    }
}
