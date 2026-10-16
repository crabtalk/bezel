//! Color picking: a preset [`Swatch`] set the app configures, and
//! [`ColorPicker`] for anything outside it.
//!
//! The set is read through [`crate::AppExt::color_swatches`] and replaced with
//! [`crate::AppExt::set_color_swatches`]; until then it is [`default_swatches`].

mod picker;

use std::rc::Rc;

use gpui::{App, Global, Hsla, SharedString};
use theme::{Appearance, Theme};

pub use picker::{ColorPicker, ColorPickerEvent, parse_hex, to_hex};

/// One preset color, with a value per appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    /// What a tooltip or an accessibility label calls it.
    pub name: SharedString,
    pub light: Hsla,
    pub dark: Hsla,
}

impl Swatch {
    pub fn new(
        name: impl Into<SharedString>,
        light: impl Into<Hsla>,
        dark: impl Into<Hsla>,
    ) -> Self {
        Self {
            name: name.into(),
            light: light.into(),
            dark: dark.into(),
        }
    }

    /// The same color under both appearances.
    pub fn fixed(name: impl Into<SharedString>, color: impl Into<Hsla>) -> Self {
        let color = color.into();
        Self::new(name, color, color)
    }

    /// The value for `theme`'s appearance.
    pub fn resolve(&self, theme: &Theme) -> Hsla {
        match theme.appearance {
            Appearance::Light => self.light,
            Appearance::Dark => self.dark,
        }
    }
}

/// One of Apple's macOS system colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SystemColor {
    Red,
    Orange,
    Yellow,
    Green,
    Mint,
    Teal,
    Cyan,
    Blue,
    Indigo,
    Purple,
    Pink,
    Brown,
}

impl SystemColor {
    /// Every one, red through brown.
    pub const ALL: [Self; 12] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Mint,
        Self::Teal,
        Self::Cyan,
        Self::Blue,
        Self::Indigo,
        Self::Purple,
        Self::Pink,
        Self::Brown,
    ];

    pub fn swatch(self) -> Swatch {
        let (name, light, dark) = match self {
            Self::Red => ("Red", 0xFF3B30, 0xFF453A),
            Self::Orange => ("Orange", 0xFF9500, 0xFF9F0A),
            Self::Yellow => ("Yellow", 0xFFCC00, 0xFFD60A),
            Self::Green => ("Green", 0x28CD41, 0x32D74B),
            Self::Mint => ("Mint", 0x00C7BE, 0x63E6E2),
            Self::Teal => ("Teal", 0x59ADC4, 0x6AC4DC),
            Self::Cyan => ("Cyan", 0x55BEF0, 0x5AC8F5),
            Self::Blue => ("Blue", 0x007AFF, 0x0A84FF),
            Self::Indigo => ("Indigo", 0x5856D6, 0x5E5CE6),
            Self::Purple => ("Purple", 0xAF52DE, 0xBF5AF2),
            Self::Pink => ("Pink", 0xFF2D55, 0xFF375F),
            Self::Brown => ("Brown", 0xA2845E, 0xAC8E68),
        };
        Swatch::new(name, gpui::rgb(light), gpui::rgb(dark))
    }

    /// The value for `theme`'s appearance.
    pub fn resolve(self, theme: &Theme) -> Hsla {
        self.swatch().resolve(theme)
    }

    /// The name to store. Stable across releases; [`Self::from_name`] reads it
    /// back.
    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Mint => "mint",
            Self::Teal => "teal",
            Self::Cyan => "cyan",
            Self::Blue => "blue",
            Self::Indigo => "indigo",
            Self::Purple => "purple",
            Self::Pink => "pink",
            Self::Brown => "brown",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.name() == name)
    }
}

/// Every [`SystemColor`], red through brown.
pub fn default_swatches() -> Rc<[Swatch]> {
    SystemColor::ALL.map(SystemColor::swatch).into()
}

struct Swatches(Rc<[Swatch]>);

impl Global for Swatches {}

pub(crate) fn swatches(cx: &App) -> Rc<[Swatch]> {
    cx.try_global::<Swatches>()
        .map_or_else(default_swatches, |set| set.0.clone())
}

pub(crate) fn set_swatches(set: impl Into<Rc<[Swatch]>>, cx: &mut App) {
    cx.set_global(Swatches(set.into()));
    cx.refresh_windows();
}
