//! Glass recipes: translucent chrome over the blurred window background, and
//! the modal scrim.

use gpui::{Hsla, WindowBackgroundAppearance};

use crate::{Appearance, color, paint, theme::Theme};

impl Theme {
    /// The frost tint painted over the blurred window background (macOS glass).
    /// Dark: darker than `surface`, matched to the reference vibrancy scrim
    /// `hsl(0 0% 3%)`. Light: a near-white frost run heavier than dark's — see
    /// [`Self::GLASS_ALPHA_LIGHT`]. On opaque platforms this IS the surface
    /// tone (no tint swap).
    pub fn glass(&self) -> Hsla {
        match self.appearance {
            Appearance::Dark => {
                if Self::GLASS_ALPHA < 1.0 {
                    color::grey(8).opacity(Self::GLASS_ALPHA)
                } else {
                    self.surface
                }
            }
            Appearance::Light => {
                if Self::GLASS_ALPHA_LIGHT < 1.0 {
                    // 0xfa, not the surface's 0xf4-ish grey: at 90% coverage
                    // the tint IS the sidebar tone, and the darker grey read
                    // as a dingy pane next to the white content card.
                    color::grey(0xfa).opacity(Self::GLASS_ALPHA_LIGHT)
                } else {
                    self.surface
                }
            }
        }
    }

    /// Whether this appearance paints translucent chrome over the blurred
    /// desktop. Glass-only recipes — backdrop blurs, translucent popover
    /// tints, per-glyph edge fades — must gate on this, not on
    /// [`Self::GLASS_ALPHA`]: that constant is platform-wide, while the frost
    /// alpha (and with it whether glass is on at all) is per-appearance.
    pub fn is_glass(&self) -> bool {
        self.glass().a < 1.0
    }

    /// Hover wash for chrome that sits ON GLASS (sidebar rows, tabs, titlebar
    /// buttons). One recipe, both appearances: the 11% [`wash`](crate::paint::wash),
    /// tone-flipped by the palette convention (soft-white on dark, soft-black
    /// on light).
    ///
    /// Hover and selection share the SAME fill (selection adds only the ring).
    /// Light previously ran heavy white washes here (hover 0.55, selection
    /// 0.92) after a black-hover-next-to-white-selection mismatch report; now
    /// hover and selection are *both* the tone-flipped wash, so they lift the
    /// same way again. Light's alpha sits under dark's: dark's 11% at the
    /// light tone read too dark over the bright frost (user report).
    pub fn glass_hover(&self) -> Hsla {
        match self.appearance {
            Appearance::Dark => paint::wash_for(Appearance::Dark, 0.11),
            Appearance::Light => paint::wash_for(Appearance::Light, 0.06),
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
        if self.is_glass() && matches!(self.appearance, Appearance::Light) {
            self.input_bg.opacity(0.30)
        } else {
            self.input_bg
        }
    }

    /// Section-card fill (settings cards and similar in-panel cards). The
    /// opaque `surface` tone read as a harsh solid slab floating on the
    /// frosted blur (user report), so glass thins it to a translucent tint;
    /// opaque platforms keep the true card tone.
    pub fn card_glass_bg(&self) -> Hsla {
        if self.is_glass() {
            self.surface.opacity(0.40)
        } else {
            self.surface
        }
    }

    /// The standard modal backdrop — see [`scrim`](crate::paint::scrim).
    pub fn scrim(&self) -> Hsla {
        paint::scrim_for(self.appearance, paint::SCRIM_ALPHA_DARK)
    }

    /// How the platform should composite the window behind our paint.
    ///
    /// Only dark macOS wants the blurred desktop — light chrome is opaque by
    /// design ([`Self::GLASS_ALPHA_LIGHT`]), so it keeps opaque compositing
    /// (subpixel-friendly, no vibrancy cost for a blur nothing shows). This is
    /// a method rather than a constant because it has to be *re-applied* after
    /// every theme swap: gpui's macOS backend tears the `NSVisualEffectView`
    /// out of the hierarchy whenever the value is anything but `Blurred`, and
    /// the re-apply in `appearance::apply` is what restores vibrancy when the
    /// user switches back to dark. See zed's `crates/zed/src/main.rs`, which
    /// runs the same loop on every settings change.
    pub fn window_background_appearance(&self) -> WindowBackgroundAppearance {
        if self.is_glass() {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Opaque
        }
    }
}
