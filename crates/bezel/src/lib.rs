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

#[cfg(test)]
mod tests {
    /// The guarantee the facade exists for: everything reached through
    /// `bezel::*` speaks the *same* gpui. If a second copy ever entered the
    /// graph these annotations would stop type-checking — which is the failure
    /// worth catching at compile time, because at runtime it shows up as a
    /// window that paints shapes but no text.
    #[test]
    fn every_layer_shares_one_gpui() {
        let theme = crate::theme::Theme::dark();
        let _: crate::gpui::Hsla = theme.bg;
        let _: crate::gpui::SharedString = theme.font_sans.clone();

        let _: crate::gpui::Hsla = crate::theme::ink(0.05);
        let _: crate::gpui::Hsla = crate::motion::mix(theme.bg, theme.text, 0.5);

        // The components layer, reached through the facade, styles with the
        // same tokens.
        let _: crate::gpui::Hsla = crate::ui::popover::band();
    }

    /// Motion's catalog is reachable without naming `bezel-motion` directly.
    #[test]
    fn motion_catalog_is_reachable() {
        assert_eq!(crate::motion::MENU_IN.duration_ms, 140);
        assert!(crate::motion::MENU_OUT.total() < crate::motion::MENU_IN.total());
    }
}
