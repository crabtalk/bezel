//! What a link is to the app: what opens it, what it shows on hover, and
//! what its chip and its card say.
//!
//! Installed once at boot with [`crate::AppExt::set_links`]. Without it a
//! link goes to [`App::open_url`] and shows nothing on hover. A `#fragment`
//! link reaches none of it: it names a heading of the document it is in — see
//! [`heading`].

use std::{collections::HashSet, path::Path, rc::Rc};

use gpui::{AnyElement, App, Global, Window};

use crate::{BlockKind, Doc, Form, Preview, render::OnJump};

/// A link where it is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Link<'a> {
    /// As written.
    pub url: &'a str,
    /// The document's [`crate::render::Editing::base`].
    pub base: Option<&'a Path>,
}

impl<'a> Link<'a> {
    pub fn new(url: &'a str, base: Option<&'a Path>) -> Self {
        Self { url, base }
    }

    /// Whether it names a web page: `http://` or `https://`.
    pub fn is_web(&self) -> bool {
        let scheme = self.url.split_once("://").map_or("", |(scheme, _)| scheme);
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    }
}

/// The app's side of every link a document paints.
///
/// Associated functions rather than methods: [`crate::AppExt::set_links`]
/// keeps each one as a plain function pointer.
pub trait Links: 'static {
    /// Opens a link a reader clicked: an inline link or a bookmark card.
    fn open(link: Link<'_>, _window: &mut Window, cx: &mut App) {
        cx.open_url(link.url);
    }

    /// What a chip, a bookmark and an embed say about the link. Asked at
    /// paint. `None` for a link the app has nothing for *yet*: the bookmark
    /// paints its host, and repaints when the app notifies.
    fn preview(_link: Link<'_>, _cx: &App) -> Option<Preview> {
        None
    }

    /// The card painted for a bookmark block, in place of the bookmark.
    /// `None` leaves the bookmark.
    ///
    /// The element sits inside [`crate::render::PAINTED_CONTEXT`]. A press it
    /// does not stop reaches the editor.
    fn card(
        _link: Link<'_>,
        _form: Form,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<AnyElement> {
        None
    }

    /// The card shown while the pointer rests on the link, in a tooltip:
    /// nothing in it scrolls or takes a press. `None` shows nothing.
    fn hover(link: Link<'_>, _window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        link.is_web()
            .then(|| crate::render::hover_bookmark(link, cx))
    }
}

/// [`Links`] with nothing installed.
struct Unhandled;

impl Links for Unhandled {}

#[derive(Clone, Copy)]
struct Installed {
    open: fn(Link<'_>, &mut Window, &mut App),
    preview: fn(Link<'_>, &App) -> Option<Preview>,
    card: fn(Link<'_>, Form, &mut Window, &mut App) -> Option<AnyElement>,
    hover: fn(Link<'_>, &mut Window, &mut App) -> Option<AnyElement>,
}

impl Global for Installed {}

impl Installed {
    fn of<L: Links>() -> Self {
        Self {
            open: L::open,
            preview: L::preview,
            card: L::card,
            hover: L::hover,
        }
    }

    fn get(cx: &App) -> Self {
        cx.try_global::<Self>()
            .copied()
            .unwrap_or_else(Self::of::<Unhandled>)
    }
}

pub(crate) fn set_links<L: Links>(cx: &mut App) {
    cx.set_global(Installed::of::<L>());
}

pub(crate) fn open(link: Link<'_>, window: &mut Window, cx: &mut App) {
    (Installed::get(cx).open)(link, window, cx)
}

pub(crate) fn preview(link: Link<'_>, cx: &App) -> Option<Preview> {
    (Installed::get(cx).preview)(link, cx)
}

pub(crate) fn card(
    link: Link<'_>,
    form: Form,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    (Installed::get(cx).card)(link, form, window, cx)
}

pub(crate) fn hover(link: Link<'_>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    (Installed::get(cx).hover)(link, window, cx)
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
pub(crate) fn follow(link: Link<'_>, jump: Option<&Jump>, window: &mut Window, cx: &mut App) {
    let Some(fragment) = link.url.strip_prefix('#') else {
        open(link, window, cx);
        return;
    };
    if let Some(jump) = jump
        && let Some(&(_, block)) = jump.anchors.iter().find(|(anchor, _)| anchor == fragment)
    {
        (jump.to)(block, window, cx);
    }
}
