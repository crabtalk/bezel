//! How big a document is set, and the chords that nudge it.
//!
//! Two numbers, the way Xcode and Zed split them. The *base* is the app's, in
//! points: a settings field, handed to [`Editor::with_text_size`], and never
//! touched from in here. The *adjustment* is the reader's — what `cmd-+` moves,
//! shared by every open document, and held in memory only. A reset clears the
//! adjustment, and the base is what is left.
//!
//! Nothing here is persisted: storage is the app's, not a component library's.
//! The base comes from whatever settings the app keeps, and an app that wants
//! the adjustment to outlive the process stores [`text_size_adjustment`]
//! beside it — beside, not folded into the base, or a reset has nowhere left
//! to return to.
//!
//! [`Editor::with_text_size`]: crate::Editor::with_text_size

use gpui::{App, Global};

use theme::TextStyle;

/// How far the chords move a document, in points — the unit a settings field
/// is written in, so the two never need converting between.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSize {
    /// Points per press. Whole points by default, so every stop is a size that
    /// could be written into a settings file.
    pub step: f32,
    pub min: f32,
    pub max: f32,
}

impl TextSize {
    /// How the editor sizes, or [`TextSize::default`] before anything is
    /// installed. Mirrors [`theme::Theme::of`].
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }

    /// A size brought inside the range, and rounded to a tenth of a point so a
    /// run of steps lands on the same numbers each time.
    pub(crate) fn clamp(self, points: f32) -> f32 {
        (points.clamp(self.min, self.max) * 10.0).round() / 10.0
    }
}

impl Default for TextSize {
    /// A point per press, over the range the ladder itself stays legible in —
    /// its smallest measured role at the bottom, twice the body at the top.
    fn default() -> Self {
        Self {
            step: 1.0,
            min: TextStyle::Caption2.size(),
            max: TextStyle::Body.size() * 2.0,
        }
    }
}

struct Installed(TextSize);

impl Global for Installed {}

/// `editor::set_text_size(cx, my_steps)` — call once at boot.
pub fn set_text_size(cx: &mut App, text_size: TextSize) {
    cx.set_global(Installed(text_size));
}

/// What every document is currently set away from its base by, in points.
///
/// Zero unless a reader has reached for the chords. A host showing the size in
/// a status bar reads this; one that does not care never has to know it exists.
pub fn text_size_adjustment(cx: &App) -> f32 {
    cx.try_global::<Adjustment>().map_or(0.0, |held| held.0)
}

/// Move every open document by `points`, each staying inside the range against
/// its own base.
pub fn adjust_text_size(cx: &mut App, points: f32) {
    set_adjustment(text_size_adjustment(cx) + points, cx);
}

/// Give every document back to the size its app set it at.
pub fn reset_text_size(cx: &mut App) {
    set_adjustment(0.0, cx);
}

pub(crate) fn set_adjustment(points: f32, cx: &mut App) {
    cx.set_global(Adjustment(points));
    // The adjustment is shared, so this is what moves the documents in every
    // other window with it.
    cx.refresh_windows();
}

/// The size a document with this base is actually set at.
pub(crate) fn resolve(base: Option<f32>, cx: &App) -> f32 {
    let base = base.unwrap_or_else(theme::base_text_size);
    TextSize::of(cx).clamp(base + text_size_adjustment(cx))
}

struct Adjustment(f32);

impl Global for Adjustment {}
