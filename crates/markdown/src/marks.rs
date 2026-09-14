//! The inline marks an app spells itself.
//!
//! [`Mark`](crate::Mark) is closed — bold, italic, strike, code, links — because
//! every one of those has a spelling CommonMark already reads. Underline,
//! highlight and a colour do not, so they cannot be variants here without this
//! crate inventing markdown for everyone. An app that wants them registers a
//! name and the delimiter that spells it, and the parse and the serializer take
//! it from there:
//!
//! ```
//! let marks = markdown::Marks::new().with("highlight", "==");
//! let doc = markdown::parse_with("a ==lit== word", &marks);
//! assert_eq!(markdown::serialize_with(&doc, &marks), "a ==lit== word");
//! ```
//!
//! The registry is a *parameter* rather than a global because [`parse_with`]
//! and [`serialize_with`] are pure — the same reason the highlighter is a
//! function pointer rather than a dependency. [`set_marks`] is the gpui-side
//! half, for the editing surface, which has a `cx` and no other way to know.
//!
//! A delimiter markdown already spells (`*`, `_`, `` ` ``, `~`, `[`) is yours to
//! avoid: CommonMark reads it first and the registration never fires.
//!
//! [`parse_with`]: crate::parse_with
//! [`serialize_with`]: crate::serialize_with

use gpui::{App, FontWeight, Global, Hsla, SharedString};
use theme::Theme;

/// The custom marks a document is read and written with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) name: SharedString,
    pub(crate) delimiter: SharedString,
}

impl Marks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `name`, spelled by `delimiter` on both sides — `==lit==`.
    ///
    /// An empty delimiter, or a second registration of a name or a delimiter
    /// already taken, is ignored: a registry that can hold two answers for one
    /// spelling has no reading of a document to offer.
    pub fn with(
        mut self,
        name: impl Into<SharedString>,
        delimiter: impl Into<SharedString>,
    ) -> Self {
        let (name, delimiter) = (name.into(), delimiter.into());
        let taken = self
            .entries
            .iter()
            .any(|entry| entry.name == name || entry.delimiter == delimiter);
        if !delimiter.is_empty() && !taken {
            self.entries.push(Entry { name, delimiter });
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every registered name, in the order they were added.
    pub fn names(&self) -> impl Iterator<Item = &SharedString> {
        self.entries.iter().map(|entry| &entry.name)
    }

    /// What spells `name`, for a caller writing its own markdown.
    pub fn delimiter(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.delimiter.as_ref())
    }

    /// The entries, longest delimiter first — `===` has to be tried before `==`
    /// or it is never reached.
    pub(crate) fn sorted(&self) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries.iter().collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.delimiter.len()));
        entries
    }

    pub(crate) fn index(&self, ix: usize) -> Option<&Entry> {
        self.entries.get(ix)
    }

    pub(crate) fn position(&self, entry: &Entry) -> Option<usize> {
        self.entries.iter().position(|row| row == entry)
    }

    /// What the editing surface reads a document with, or nothing registered.
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0.clone())
    }
}

struct Installed(Marks);

impl Global for Installed {}

/// `markdown::set_marks(cx, my_marks)` — call once at boot, so the editing
/// surface reads and writes the same markdown the app's own calls do.
pub fn set_marks(cx: &mut App, marks: Marks) {
    cx.set_global(Installed(marks));
}

/// How a custom mark paints. Everything a [`gpui::TextRun`] can carry, and
/// nothing a layout would have to move for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MarkPaint {
    pub color: Option<Hsla>,
    pub background: Option<Hsla>,
    pub weight: Option<FontWeight>,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

/// What a registered mark looks like. `None` for a name this build does not
/// paint, which reads as ordinary text.
pub type Painter = fn(name: &str, theme: &Theme) -> Option<MarkPaint>;

struct InstalledPaint(Painter);

impl Global for InstalledPaint {}

/// `markdown::set_mark_paint(cx, my_paint)` — call once at boot. Without it a
/// custom mark round trips and paints as the text it wraps, which is what an
/// unknown mark should look like rather than a hole.
pub fn set_mark_paint(cx: &mut App, paint: Painter) {
    cx.set_global(InstalledPaint(paint));
}

pub(crate) fn paint_of(cx: &App, name: &str, theme: &Theme) -> Option<MarkPaint> {
    (cx.try_global::<InstalledPaint>()?.0)(name, theme)
}
