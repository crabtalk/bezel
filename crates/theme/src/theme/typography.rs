//! The system type ladder: eleven roles, each carrying a size and a weight.
//!
//! Measured on macOS 26, 2026-08-31, through `NSFont.preferredFont(forTextStyle:)`.

use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{App, FontWeight, Styled, px};

/// The body size every painted role is scaled against, as raw `f32` bits.
static BASE: AtomicU32 = AtomicU32::new(TextStyle::Body.size().to_bits());

/// Set the body size in points; every other role keeps its ratio to it, the way
/// every corner is a ratio of [`Brand::radius`](crate::Brand::radius).
///
/// A probe for the chrome that does not grow with the text —
/// [`Theme::HEADER_HEIGHT`], [`Theme::STATUS_STRIP_HEIGHT`] and every fixed
/// `py`. The measured ramp is non-linear per role, so one ratio finds that
/// coupling without describing the ramp; [`TextStyle::size`] stays the measured
/// table at any setting.
///
/// [`Theme::HEADER_HEIGHT`]: crate::Theme::HEADER_HEIGHT
/// [`Theme::STATUS_STRIP_HEIGHT`]: crate::Theme::STATUS_STRIP_HEIGHT
pub fn set_base_text_size(points: f32, cx: &mut App) {
    BASE.store(points.to_bits(), Ordering::Relaxed);
    cx.refresh_windows();
}

/// The body size in points. [`TextStyle::Body`]'s own size paints the measured
/// ladder.
pub fn base_text_size() -> f32 {
    f32::from_bits(BASE.load(Ordering::Relaxed))
}

/// A role in the type ladder — SwiftUI's `Font.TextStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextStyle {
    LargeTitle,
    Title,
    Title2,
    Title3,
    Headline,
    Subheadline,
    Body,
    Callout,
    Footnote,
    Caption,
    Caption2,
}

impl TextStyle {
    /// The role's measured size in points.
    pub const fn size(self) -> f32 {
        match self {
            Self::LargeTitle => 26.0,
            Self::Title => 22.0,
            Self::Title2 => 17.0,
            Self::Title3 => 15.0,
            Self::Headline | Self::Body => 13.0,
            Self::Callout => 12.0,
            Self::Subheadline => 11.0,
            Self::Footnote | Self::Caption | Self::Caption2 => 10.0,
        }
    }

    /// The size this role paints at, which [`set_base_text_size`] moves.
    pub fn painted(self) -> f32 {
        self.size() * base_text_size() / Self::Body.size()
    }

    /// The role's weight. Three roles share 13pt and three share 10pt, so this
    /// is what separates them.
    pub const fn weight(self) -> FontWeight {
        match self {
            Self::Headline => FontWeight::BOLD,
            Self::Caption2 => FontWeight::MEDIUM,
            _ => FontWeight::NORMAL,
        }
    }
}

/// The ladder, on anything styled.
pub trait Typeset: Styled + Sized {
    /// Size and weight together, from [`TextStyle`].
    fn text_style(self, style: TextStyle) -> Self {
        self.text_size(px(style.painted()))
            .font_weight(style.weight())
    }
}

impl<E: Styled> Typeset for E {}
