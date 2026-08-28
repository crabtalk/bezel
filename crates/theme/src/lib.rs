//! The app theme — two concrete appearances, one token set.
//!
//! Colors are precomputed from an oklch-derived neutral scale (perceptually even
//! lightness steps; the same scale the reference Tailwind theme used) into gpui
//! [`Hsla`](gpui::Hsla).
//! **Numbers drive layout, colors are paint**: layout constants live in
//! `theme/layout.rs` as plain numbers and never depend on which color is painted.
//!
//! # Light is designed, not inverted
//!
//! Mirroring lightness produces the classic "washed-out inverted" look, for three
//! reasons this module handles explicitly:
//!
//! 1. **Surface order flips meaning.** In dark, the main content panel is the
//!    *darkest* plane and raised surfaces get *lighter*. In light, the content
//!    panel is *white* and the shell/sidebar goes *grey* — chrome recedes by
//!    getting darker, not lighter. Popovers stay white and earn separation from a
//!    border and shadow rather than from lightness.
//! 2. **Elevation reverses.** On dark, a faint *white* wash means "raised". Its
//!    literal translation — a faint *black* wash on white — means "recessed", so
//!    the composer read as a dent instead of a plate. Light lifts with white plus
//!    a border and shadow ([`Theme::input_bg`], the elevation ladder). Fill
//!    *alphas* carry over unchanged ([`INK_FILL_SCALE`]); only hairlines scale, so
//!    a 1px edge survives a bright surround ([`INK_HAIRLINE_SCALE`]).
//! 3. **Accents must move down the scale.** The dark palette's 400-level accents
//!    (red/amber, and whatever hue an app sets [`Theme::accent`] to) are chosen
//!    for contrast against near-black; on white they fall to 2–4:1 and fail WCAG
//!    AA. Light mode uses the 600-level siblings at the same hue, which restores
//!    the *contrast ratio* the dark token had.
//!
//! Text tones are chosen so each light token lands within ~0.5 of its dark
//! counterpart's contrast ratio against its own background — the pairing is
//! verified in `tests/theme.rs`, not eyeballed.
//!
//! Installed as a gpui [`Global`](gpui::Global) at boot; read with [`Theme::of`].

pub mod appearance;

mod brand;
mod color;
mod paint;
mod theme;

pub use brand::{BASE_COLORS, Brand, Tint, brand, set_brand};

pub use color::{
    contrast_ratio, flatten, grey, hsl_to_rgb, lightness, mix, neutral, oklch, oklch_to_srgb,
    relative_luminance, rgb_to_hsl, tint,
};
pub use paint::{
    INK_FILL_SCALE, INK_HAIRLINE_SCALE, SCRIM_ALPHA_DARK, band, card_selected_bg,
    card_selected_shadows, current_appearance, glass_selected_bg, glass_selected_shadows, hairline,
    ink, lock_appearance, scrim, set_current_appearance, theme_generation, user_bubble_bg, wash,
};
pub use theme::{HighlightKind, SyntaxPalette, Theme, set_glass_bevel, set_glass_magnify, set_palette};

/// The carrier seam for catalog traits: any type holding a [`Theme`] exposes
/// it through this one method, and every component group
/// (`ui::widgets::Scaffolding`, …) extends it, so group methods read
/// the environment through `self.theme()`.
pub trait ThemeExt {
    fn theme(&self) -> &Theme;
}

impl ThemeExt for Theme {
    fn theme(&self) -> &Theme {
        self
    }
}

/// Which appearance the app is painting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}

impl Appearance {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    pub fn is_light(self) -> bool {
        matches!(self, Self::Light)
    }

    /// Map a gpui window appearance onto ours (both vibrant variants are just
    /// the blurred flavour of the same tone).
    pub fn from_window(appearance: gpui::WindowAppearance) -> Self {
        use gpui::WindowAppearance::*;
        match appearance {
            Light | VibrantLight => Self::Light,
            Dark | VibrantDark => Self::Dark,
        }
    }
}
