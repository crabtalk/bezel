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
//! # What it carries, and what it will not
//!
//! The layers here are the ones every app paints with. `markdown`, `syntax` and
//! `terminal` are peer crates a consumer names itself, because each is an
//! *implementation* behind a seam the library already opens —
//! `markdown::set_highlighter` takes any `fn(&str, &str)`, so tree-sitter is one
//! answer and not the answer. Re-exporting `syntax` made every consumer of this
//! crate compile seven C grammars to get a button, and made the facade
//! unbuildable for `wasm32-unknown-unknown`, where that C has no libc — the very
//! failure `markdown` names no highlighter to avoid.
//!
//! Font gates are wired: `geist-sans`, `geist-mono` and `geist-weights`, all on
//! by default, forwarded to `ui` so an app shipping its own type stops paying
//! for the families it does not paint.
//!
//! Depending on a single layer directly (`theme` for tokens alone, which
//! is useful to anyone writing their own gpui components) stays supported and
//! is not going away.

pub use agent;
pub use motion;
pub use theme;
pub use ui;

/// The exact gpui these components were built against. Depend on this rather
/// than declaring your own — see the crate docs.
pub use gpui;
