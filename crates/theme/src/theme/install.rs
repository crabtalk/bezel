//! Installing a palette as the gpui global, and the per-app palette builder.

use gpui::{App, Global};

use crate::{Appearance, paint, theme::Theme};

impl Theme {
    /// Install the palette for `appearance` as the gpui global and point the
    /// context-free paint helpers at it. The **only** way the appearance should
    /// change — setting the global directly leaves
    /// [`current_appearance`](crate::paint::current_appearance) stale.
    ///
    /// Which palette that is comes from [`set_palette`], so an app with its own
    /// colours keeps them across a light/dark switch.
    pub fn install(appearance: Appearance, cx: &mut App) {
        let build = cx
            .try_global::<Palette>()
            .map_or(Self::for_appearance as fn(Appearance) -> Theme, |p| p.0);
        Self::install_custom(build(appearance), cx);
    }

    /// Install a palette the caller built: brand colours, one retuned token, or
    /// a wholesale replacement. `Theme` is a plain struct with public fields, so
    /// the usual shape is `Theme::light()` with a few fields overwritten.
    ///
    /// Use this rather than `cx.set_global(theme)`. The context-free paint
    /// helpers ([`ink`], [`hairline`], [`wash`], …) read [`current_appearance`]
    /// and not the global, so a palette installed around this one leaves them
    /// painting for whatever appearance was last installed — light washes over a
    /// dark palette, and nothing to point at.
    ///
    /// One-shot: [`appearance::apply`] rebuilds the palette whenever the
    /// appearance changes, so what is installed here is replaced on a light/dark
    /// switch. For colours that survive that, register a builder with
    /// [`set_palette`] instead.
    ///
    /// [`appearance::apply`]: crate::appearance::apply
    /// [`ink`]: crate::paint::ink
    /// [`hairline`]: crate::paint::hairline
    /// [`wash`]: crate::paint::wash
    /// [`current_appearance`]: crate::paint::current_appearance
    pub fn install_custom(theme: Theme, cx: &mut App) {
        paint::set_current_appearance(theme.appearance);
        cx.set_global(theme);
    }

    /// Read the theme global.
    pub fn of(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }
}

/// How the app builds a palette for an appearance. See [`set_palette`].
struct Palette(fn(Appearance) -> Theme);

impl Global for Palette {}

/// Teach bezel how this app builds its palette, so light/dark switching rebuilds
/// *your* colours instead of replacing them with the built-in ones.
///
/// A palette installed with [`Theme::install_custom`] alone lasts only until the
/// appearance changes, because [`appearance::apply`] rebuilds from scratch.
/// Registering the builder is what makes brand colours survive:
///
/// ```ignore
/// fn palette(appearance: Appearance) -> Theme {
///     let mut theme = Theme::for_appearance(appearance);
///     theme.accent = my_brand_accent(appearance);
///     theme
/// }
/// theme::set_palette(palette, cx);          // before appearance::init
/// ```
///
/// Call it before [`appearance::init`], which installs the first palette. Later
/// than that, follow it with [`appearance::apply`] to repaint.
///
/// [`appearance::init`]: crate::appearance::init
/// [`appearance::apply`]: crate::appearance::apply
pub fn set_palette(build: fn(Appearance) -> Theme, cx: &mut App) {
    cx.set_global(Palette(build));
}
