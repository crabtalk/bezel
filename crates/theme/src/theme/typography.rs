//! The system type ladder: eleven roles, each carrying a size and a weight.
//!
//! Sizes measured on macOS 26, 2026-08-31, through
//! `NSFont.preferredFont(forTextStyle:)`; line heights 2026-09-01, through
//! `NSLayoutManager.defaultLineHeight(for:)` on the same fonts.

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

    /// The role's measured line height in points, at its measured [`Self::size`].
    ///
    /// A table beside `size`, because the ratio is not one number: it runs 1.18
    /// at `Title` up to 1.33 at `Title3`, and does not move monotonically with
    /// the size. Left unset, gpui leads every line at phi — 21pt on a 13pt body
    /// against the platform's 16.
    pub const fn line_height(self) -> f32 {
        match self {
            Self::LargeTitle => 32.0,
            Self::Title => 26.0,
            Self::Title2 => 22.0,
            Self::Title3 => 20.0,
            Self::Headline | Self::Body => 16.0,
            Self::Callout => 15.0,
            Self::Subheadline => 14.0,
            Self::Footnote | Self::Caption | Self::Caption2 => 13.0,
        }
    }

    /// The line box this role paints in, which [`set_base_text_size`] moves.
    pub fn painted_line_height(self) -> f32 {
        self.line_height() * base_text_size() / Self::Body.size()
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

/// One role as it is actually set: a rung on the ladder, the leading it carries,
/// and the weight it is set in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub role: TextStyle,
    /// Line height as a multiple of the painted size, so leading follows the
    /// type wherever [`set_base_text_size`] puts it.
    pub leading: f32,
    /// The ladder carries one bold cell, so a set needing several heading
    /// weights names its own here rather than reading it off the role.
    pub weight: FontWeight,
}

impl Metrics {
    pub const fn new(role: TextStyle, leading: f32, weight: FontWeight) -> Self {
        Self {
            role,
            leading,
            weight,
        }
    }

    pub fn size(self) -> f32 {
        self.role.painted()
    }

    pub fn line_height(self) -> f32 {
        self.size() * self.leading
    }
}

/// The ladder, on anything styled.
pub trait Typeset: Styled + Sized {
    /// Size and weight together, from [`TextStyle`].
    fn text_style(self, style: TextStyle) -> Self {
        self.text_size(px(style.painted()))
            .line_height(px(style.painted_line_height()))
            .font_weight(style.weight())
    }
}

impl<E: Styled> Typeset for E {}
