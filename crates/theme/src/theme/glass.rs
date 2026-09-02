//! Glass recipes: translucent chrome over the blurred window background, and
//! the modal scrim.

use gpui::{Hsla, WindowBackgroundAppearance, hsla};

use crate::{Appearance, color, paint, theme::Theme};

impl Theme {
    /// The frost tint painted over the blurred window background (macOS glass),
    /// at [`Brand::vibrancy_alpha`](crate::Brand::vibrancy_alpha). Dark: darker than
    /// `surface`, matched to the reference vibrancy scrim `hsl(0 0% 3%)`.
    /// Light: the material's own measured tone. Opaque, this IS the surface tone.
    pub fn vibrancy_tint(&self) -> Hsla {
        let alpha = self.vibrancy_alpha;
        if alpha >= 1.0 {
            return self.surface;
        }
        match self.appearance {
            Appearance::Dark => color::grey(8).opacity(alpha),
            Appearance::Light => self.material.tone.opacity(alpha),
        }
    }

    /// The app's root fill: the frost where glass is on, the opaque panel
    /// where it is not. What a root element paints instead of
    /// [`Self::bg`](Self#structfield.bg), so a window that opens blurred is not
    /// then covered over by the paint that made the blur pointless.
    pub fn window_bg(&self) -> Hsla {
        if self.vibrancy {
            self.vibrancy_tint()
        } else {
            self.bg
        }
    }

    /// The translucent tint floating cards paint over their backdrop blur
    /// (see `bezel::material`). Dark: the reference
    /// `.glass-surface` menu tint verbatim — `oklch(0.33 0 0 / 34%)`. The
    /// previous `surface_overlay` at 65% was tuned back when the tint had to
    /// *approximate* the composited recipe without a real blur; kept over the
    /// blur it buried the backdrop's colour and menus read as flat grey slabs
    /// next to the hue-inheriting chrome (user report). At 34% the blurred
    /// backdrop carries the card and the mid-grey only lifts it off the
    /// plane. Light: heavier — a translucent white tint left menu text
    /// ghosting over whatever sat behind the popover, so light coverage
    /// steps up to keep rows on a known background.
    pub fn glass_overlay(&self) -> Hsla {
        match self.appearance {
            Appearance::Dark => color::oklch(0.33, 0.0, 0.0).opacity(0.34),
            Appearance::Light => self.surface_overlay.opacity(0.85),
        }
    }

    /// The composer pill / question panel fill. Light's `input_bg` is opaque
    /// white (the elevation ladder on an opaque page) — over glass it read as
    /// a solid slab in front of the frosted blur, so it thins to a
    /// translucent tint there (0.6 and then 0.45 both still read too bright
    /// over the 0.80 frost — lowered on user request). Dark's 3% white wash
    /// is already glass-native.
    pub fn input_glass_bg(&self) -> Hsla {
        if self.glass && matches!(self.appearance, Appearance::Light) {
            self.input_bg.opacity(0.30)
        } else {
            self.input_bg
        }
    }

    /// Section-card fill — the group box, and the in-panel cards built like it.
    ///
    /// Each appearance plates in the direction it has room in: dark lifts on a
    /// white wash (`../desktop`'s `--color-card`), light lands a near-opaque
    /// white card on the grey frost, at the coverage [`Self::glass_overlay`]
    /// already needs to keep rows on a known background. An opaque platform has
    /// no frost beneath the card, so it takes the grey below its white page.
    pub fn card_glass_bg(&self) -> Hsla {
        if !self.glass {
            return self.surface;
        }
        match self.appearance {
            Appearance::Dark => hsla(0.0, 0.0, 1.0, 0.06),
            Appearance::Light => self.surface_card.opacity(0.85),
        }
    }

    /// The standard modal backdrop — see [`scrim`](crate::paint::scrim).
    pub fn scrim(&self) -> Hsla {
        paint::scrim_for(self.appearance, paint::SCRIM_ALPHA_DARK)
    }

    /// How the platform should composite the window behind our paint.
    ///
    /// This is a method rather than a constant because it has to be *re-applied* after
    /// every theme swap: gpui's macOS backend tears the `NSVisualEffectView`
    /// out of the hierarchy whenever the value is anything but `Blurred`, and
    /// the re-apply in `appearance::apply` is what restores vibrancy when the
    /// user switches back to dark. See zed's `crates/zed/src/main.rs`, which
    /// runs the same loop on every settings change.
    pub fn window_background_appearance(&self) -> WindowBackgroundAppearance {
        if self.vibrancy {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Opaque
        }
    }
}
