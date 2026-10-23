//! How a selected row is drawn against the focus.

use gpui::{BoxShadow, Hsla};

use crate::{paint, theme::Theme};

/// A row's selected state. Emphasized while the area it is in holds the
/// keyboard focus and unemphasized otherwise, as AppKit's `isEmphasized`.
/// Both are neutral washes, never the accent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Emphasized,
    Unemphasized,
}

impl Selection {
    pub fn new(selected: bool, emphasized: bool) -> Self {
        match (selected, emphasized) {
            (false, _) => Self::None,
            (true, true) => Self::Emphasized,
            (true, false) => Self::Unemphasized,
        }
    }

    pub fn is_selected(self) -> bool {
        self != Self::None
    }

    /// The row's fill: [`Theme::card_selected_bg`] emphasized,
    /// [`Theme::element_hover`](Theme#structfield.element_hover) unemphasized.
    pub fn fill(self, theme: &Theme) -> Option<Hsla> {
        match self {
            Self::None => None,
            Self::Emphasized => Some(theme.card_selected_bg()),
            Self::Unemphasized => Some(theme.element_hover),
        }
    }

    /// The ring over the fill, for a row that carries one: emphasized only.
    pub fn shadows(self) -> Vec<BoxShadow> {
        match self {
            Self::Emphasized => paint::glass_selected_shadows(),
            Self::None | Self::Unemphasized => Vec::new(),
        }
    }
}
