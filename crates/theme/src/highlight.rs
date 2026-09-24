//! The colours a reader marks text with.
//!
//! A closed set of names rather than colour values, so a highlight saved under
//! one theme paints correctly under another. [`Theme::highlight`] turns a name
//! into the wash for the current appearance.

use gpui::Hsla;

use crate::{Appearance, Theme, color};

/// A highlight colour, by name. The set Apple Books and Notes offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HighlightColor {
    #[default]
    Yellow,
    Green,
    Blue,
    Pink,
    Purple,
}

impl HighlightColor {
    /// Every colour, in the order a picker shows them.
    pub const ALL: [Self; 5] = [
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Pink,
        Self::Purple,
    ];

    /// The name to store. Stable across releases; [`Self::from_name`] reads it
    /// back.
    pub fn name(self) -> &'static str {
        match self {
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Pink => "pink",
            Self::Purple => "purple",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.name() == name)
    }

    /// oklch hue, in degrees.
    fn hue(self) -> f32 {
        match self {
            Self::Yellow => 95.0,
            Self::Green => 150.0,
            Self::Blue => 245.0,
            Self::Pink => 350.0,
            Self::Purple => 300.0,
        }
    }
}

impl Theme {
    /// The wash `color` paints under text. Translucent, so a selection over it
    /// still shows.
    pub fn highlight(&self, color: HighlightColor) -> Hsla {
        let hue = color.hue();
        match self.appearance {
            Appearance::Dark => color::oklch(0.72, 0.14, hue).opacity(0.34),
            Appearance::Light => color::oklch(0.88, 0.13, hue).opacity(0.70),
        }
    }
}
