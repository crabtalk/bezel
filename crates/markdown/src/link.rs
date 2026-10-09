//! Who opens a link.
//!
//! Installed once at boot like the highlighter, and read at click. Without it a
//! link goes to [`App::open_url`]. A `#fragment` link reaches neither: it names
//! a heading of the document it is in — see [`heading`].

use std::{collections::HashSet, rc::Rc};

use gpui::{App, Global, Window};

use crate::{BlockKind, Doc, render::OnJump};

/// Opens a link a reader clicked: an inline link or a bookmark card.
pub type LinkHandler = fn(url: &str, &mut Window, &mut App);

struct Installed(LinkHandler);

impl Global for Installed {}

/// `cx.set_link_handler(my_links)` — call once at boot.
pub(crate) fn set_link_handler(cx: &mut App, handler: LinkHandler) {
    cx.set_global(Installed(handler));
}

pub(crate) fn open(url: &str, window: &mut Window, cx: &mut App) {
    match cx.try_global::<Installed>().map(|installed| installed.0) {
        Some(handler) => handler(url, window, cx),
        None => cx.open_url(url),
    }
}

/// The anchor GitHub gives a heading: lowercased, punctuation dropped but for
/// `-` and `_`, each space a `-`.
pub fn slug(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// The block of the heading `fragment` (without its `#`) names in `doc`. A
/// slug already taken by an earlier heading gets `-1`, `-2`, … as on GitHub.
/// The fragment is compared as written: no percent-decoding.
pub fn heading(doc: &Doc, fragment: &str) -> Option<usize> {
    anchors(doc)
        .into_iter()
        .find_map(|(anchor, block)| (anchor == fragment).then_some(block))
}

fn anchors(doc: &Doc) -> Vec<(String, usize)> {
    let mut taken = HashSet::new();
    doc.blocks
        .iter()
        .enumerate()
        .filter_map(|(ix, block)| match &block.kind {
            BlockKind::Heading { text, .. } => Some((slug(&text.text), ix)),
            _ => None,
        })
        .map(|(base, ix)| {
            let anchor = std::iter::once(base.clone())
                .chain((1..).map(|n| format!("{base}-{n}")))
                .find(|anchor| !taken.contains(anchor))
                .expect("an unbounded run of suffixes");
            taken.insert(anchor.clone());
            (anchor, ix)
        })
        .collect()
}

/// A document's heading anchors and the host's [`OnJump`], taken together at
/// render so a click resolves against the document it was painted from.
#[derive(Clone)]
pub(crate) struct Jump {
    to: OnJump,
    anchors: Rc<[(String, usize)]>,
}

impl Jump {
    pub(crate) fn new(doc: &Doc, to: OnJump) -> Self {
        Self {
            to,
            anchors: anchors(doc).into(),
        }
    }
}

/// A clicked link: a `#fragment` jumps to its heading, or does nothing when it
/// names none or no [`Jump`] is given; anything else goes to [`open`].
pub(crate) fn follow(url: &str, jump: Option<&Jump>, window: &mut Window, cx: &mut App) {
    let Some(fragment) = url.strip_prefix('#') else {
        open(url, window, cx);
        return;
    };
    if let Some(jump) = jump
        && let Some(&(_, block)) = jump.anchors.iter().find(|(anchor, _)| anchor == fragment)
    {
        (jump.to)(block, window, cx);
    }
}
