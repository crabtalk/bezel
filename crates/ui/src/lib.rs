//! ui — SwiftUI-flavored components for gpui. Reached as `bezel::ui`.
//!
//! Style flows through the environment, never through parameters: components
//! read [`theme::Theme`] (a gpui `Global`) at paint time, the way
//! SwiftUI views read `@Environment`. Motion comes from the named
//! `motion` catalog.

use std::borrow::Cow;

use gpui::App;

pub mod combobox;
pub mod control_bar;
pub mod date;
pub mod focus;
pub mod hover_card;
pub mod icons;
pub mod input;
pub mod list;
pub mod loaders;
pub mod material;
pub mod menubar;
pub mod pagination;
pub mod palette;
pub mod popover;
pub mod scroll;
pub mod table;
pub mod tooltip;
pub mod tree;
pub mod widgets;

/// Embedded UI fonts — Geist and Geist Mono (variable), © Vercel Inc.,
/// licensed under the SIL Open Font License 1.1 (https://openfontlicense.org).
/// Bundled so the type ships with the binary instead of depending on what the
/// host system happens to have installed.
#[cfg(feature = "geist-sans")]
static FONT_GEIST: &[u8] = include_bytes!("../assets/fonts/Geist.ttf");
#[cfg(feature = "geist-mono")]
static FONT_GEIST_MONO: &[u8] = include_bytes!("../assets/fonts/GeistMono.ttf");
/// Static Geist weights alongside the variable file: gpui's cosmic-text path
/// (Linux) rasterizes variable fonts at their default instance only — it never
/// applies `wght` coordinates — so medium/semibold/bold text silently paints
/// at 400 with just the variable TTF registered. The statics give the face
/// matcher real 500/600/700 faces (macOS/CoreText applies the variable axis
/// natively and simply never falls through to these).
/// They cover the sans only — Geist Mono ships as the variable file alone, so
/// the cosmic-text path paints every mono weight at 400.
#[cfg(feature = "geist-weights")]
static FONT_GEIST_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Geist-Medium.ttf");
#[cfg(feature = "geist-weights")]
static FONT_GEIST_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Geist-SemiBold.ttf");
#[cfg(feature = "geist-weights")]
static FONT_GEIST_BOLD: &[u8] = include_bytes!("../assets/fonts/Geist-Bold.ttf");

/// Register the bundled fonts with the gpui text system. Failure is non-fatal:
/// the theme's system fallbacks take over (same families the CSS stack names).
///
/// Your own fonts go through this same gpui API — `add_fonts` takes any bytes,
/// and [`theme::Theme::font_sans`] / [`theme::Theme::font_mono`] name which
/// family the components then paint with.
pub fn register_fonts(cx: &App) -> gpui::Result<()> {
    #[allow(unused_mut, reason = "every face below is behind a feature")]
    let mut fonts: Vec<Cow<'static, [u8]>> = Vec::new();
    #[cfg(feature = "geist-sans")]
    fonts.push(Cow::Borrowed(FONT_GEIST));
    #[cfg(feature = "geist-mono")]
    fonts.push(Cow::Borrowed(FONT_GEIST_MONO));
    #[cfg(feature = "geist-weights")]
    fonts.extend([
        Cow::Borrowed(FONT_GEIST_MEDIUM),
        Cow::Borrowed(FONT_GEIST_SEMIBOLD),
        Cow::Borrowed(FONT_GEIST_BOLD),
    ]);
    cx.text_system().add_fonts(fonts)
}
