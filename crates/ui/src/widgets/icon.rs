//! Icons, as the environment paints them.
//!
//! A catalog trait, like every widget group: import it to unlock
//! `theme.icon(..)` and `theme.icon_at(..)`.

use gpui::{Svg, prelude::*, px};
use icons::Icon;
use theme::{TextStyle, ThemeExt};

pub trait Icons: ThemeExt {
    /// An icon at the ladder's body step, in the plain text tone.
    ///
    /// Both are defaults, not decisions: `Svg` is `Styled`, so a later
    /// `.size(..)` or `.text_color(..)` wins, and a component's own metric
    /// stays with the component. What this buys is the floor —
    /// [`icons::icon`] paints *nothing* when no colour is set anywhere,
    /// because gpui resolves an svg's tone from its own style and never
    /// inherits one, so an icon with a forgotten `text_color` is invisible
    /// rather than wrong.
    fn icon(&self, icon: impl Into<Icon>) -> Svg {
        self.icon_at(TextStyle::Body, icon)
    }

    /// The same, sized to a named rung — a caption's icon beside caption text.
    /// The ladder is the one type reads, so the two move together when
    /// [`theme::set_base_text_size`] does.
    fn icon_at(&self, role: TextStyle, icon: impl Into<Icon>) -> Svg {
        icons::icon(icon)
            .size(px(role.painted()))
            .text_color(self.theme().text)
    }
}

impl Icons for theme::Theme {}
