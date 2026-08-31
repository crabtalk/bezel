//! What a document is set in.
//!
//! Installed once at boot like the highlighter and the link preview, and read
//! at paint: how a document is set is the app's decision, and this crate holds
//! only what it defaults to.

use gpui::{App, FontWeight, Global};
use theme::{Metrics, TextStyle};

/// What a document is set in, role by role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    pub body: Metrics,
    pub h1: Metrics,
    pub h2: Metrics,
    pub h3: Metrics,
    /// Every heading past the third.
    pub h4: Metrics,
    /// Code, in a fence and inline.
    pub code: Metrics,
    /// A bookmark card's blurb and footer.
    pub card: Metrics,
    /// An image's caption.
    pub caption: Metrics,
}

impl Typography {
    /// What documents are set in, or [`Typography::default`] before anything is
    /// installed. Mirrors [`theme::Theme::of`].
    pub fn of(cx: &App) -> Self {
        cx.try_global::<Installed>()
            .map_or_else(Self::default, |installed| installed.0)
    }

    pub fn heading(&self, level: u8) -> Metrics {
        match level {
            1 => self.h1,
            2 => self.h2,
            3 => self.h3,
            _ => self.h4,
        }
    }
}

impl Default for Typography {
    /// Each leading is written as the pixel pair it came from, so the ratio the
    /// document was tuned at survives a change of size.
    fn default() -> Self {
        Self {
            body: Metrics::new(TextStyle::Body, 22.0 / 14.0, FontWeight::NORMAL),
            h1: Metrics::new(TextStyle::Title, 27.0 / 19.0, FontWeight::SEMIBOLD),
            h2: Metrics::new(TextStyle::Title2, 24.0 / 16.0, FontWeight::SEMIBOLD),
            h3: Metrics::new(TextStyle::Title3, 22.0 / 15.0, FontWeight::SEMIBOLD),
            h4: Metrics::new(TextStyle::Headline, 22.0 / 14.0, FontWeight::SEMIBOLD),
            code: Metrics::new(TextStyle::Callout, 18.0 / 12.5, FontWeight::NORMAL),
            card: Metrics::new(TextStyle::Callout, 17.0 / 12.0, FontWeight::NORMAL),
            caption: Metrics::new(TextStyle::Subheadline, 17.0 / 11.5, FontWeight::NORMAL),
        }
    }
}

struct Installed(Typography);

impl Global for Installed {}

/// `markdown::set_typography(cx, my_typography)` — call once at boot.
pub fn set_typography(cx: &mut App, typography: Typography) {
    cx.set_global(Installed(typography));
}
