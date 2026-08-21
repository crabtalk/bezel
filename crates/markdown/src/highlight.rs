//! Who colors a fenced block.
//!
//! `markdown` names no highlighter. The one this crate would otherwise reach
//! for is tree-sitter, whose C cannot be built for `wasm32-unknown-unknown` —
//! there is no libc to compile it against — so a web build would carry a
//! dependency it can never link. Installed once at boot like the theme
//! palette, and read at paint.

use std::ops::Range;

use gpui::{App, Global, SharedString};
use theme::HighlightKind;

/// Spans over `code`, in bytes. `None` for a language the caller cannot color.
pub type Highlighter = fn(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>>;

struct Installed {
    highlighter: Highlighter,
    languages: Vec<SharedString>,
}

impl Global for Installed {}

/// `markdown::set_highlighter(cx, my_highlighter, my_languages)` — call once at
/// boot. Without it every fenced block paints in one plain run, which is what a
/// document looks like before anyone has an opinion about its code.
///
/// The names travel with the function because they are the same fact twice: a
/// picker that offers a language nothing can color is a promise the highlighter
/// does not keep.
pub fn set_highlighter(
    cx: &mut App,
    highlighter: Highlighter,
    languages: impl IntoIterator<Item = impl Into<SharedString>>,
) {
    cx.set_global(Installed {
        highlighter,
        languages: languages.into_iter().map(Into::into).collect(),
    });
}

/// What the installed highlighter can color — the list a language picker
/// offers, empty until someone installs one.
pub fn languages(cx: &App) -> &[SharedString] {
    cx.try_global::<Installed>()
        .map_or(&[], |installed| &installed.languages)
}

pub(crate) fn spans(
    cx: &App,
    language: Option<&str>,
    code: &str,
) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    (cx.try_global::<Installed>()?.highlighter)(language?, code)
}
