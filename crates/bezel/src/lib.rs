//! bezel — the whole library behind one dependency.
//!
//! Each layer stays a crate of its own and is re-exported here as a peer
//! namespace, so the paths read the same whichever way you depend:
//!
//! ```ignore
//! use bezel::{motion, theme::Theme, ui::widgets};
//! ```
//!
//! # Why the facade exists at all
//!
//! [`gpui`] is re-exported, and that is the load-bearing part. A consumer that
//! writes its own `gpui = "0.2.2"` alongside bezel can end up with a *second*
//! copy in the graph — two incompatible type universes whose failure is a
//! trait-bound error at best and, at worst, a window that paints shapes but no
//! text, one text system holding the fonts while the other draws the frame.
//! Going through `bezel::gpui` makes that impossible by construction.
//!
//! # What it does not do yet
//!
//! No feature flags. They are the reason `ARCHITECTURE.md` planned this crate —
//! gating `markdown` (pulldown-cmark), `syntax` (28 tree-sitter grammars) and
//! `terminal` (alacritty) so nobody compiles a grammar to get a button. Those
//! layers do not exist yet, and features that gate nothing are machinery for
//! its own sake. They arrive with the crates they protect; until then all three
//! layers are light and unconditional.
//!
//! Depending on a single layer directly (`bezel-theme` for tokens alone, which
//! is useful to anyone writing their own gpui components) stays supported and
//! is not going away.

pub use bezel_motion as motion;
pub use bezel_theme as theme;
pub use bezel_ui as ui;

/// The exact gpui these components were built against. Depend on this rather
/// than declaring your own — see the crate docs.
pub use gpui;
