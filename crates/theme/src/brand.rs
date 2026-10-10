//! Brand: one hue for the greys, one for the accent, one radius, and the
//! text ink.
//!
//! Apart from the text ink, a brand moves hue alone: every other lightness in
//! `palettes.rs` stays as the palette set it.

use gpui::{App, Global, Hsla};

use crate::{Appearance, color, theme::Theme};

/// A hue and how much of it, in oklch terms. `chroma: 0.0` is the shipped
/// neutral, so [`Brand::default`] reproduces the built-in palette exactly.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tint {
    /// oklch hue, in degrees.
    pub hue: f32,
    /// oklch chroma. Neutral ramps live near 0.01–0.05; an accent carries more.
    pub chroma: f32,
}

impl Tint {
    pub const NONE: Self = Self {
        hue: 0.0,
        chroma: 0.0,
    };

    pub const fn new(hue: f32, chroma: f32) -> Self {
        Self { hue, chroma }
    }
}

/// The greys a UI is built on, as oklch hue and chroma.
///
/// Tailwind's five neutral families at their 500 step (tailwindcss.com/docs/colors,
/// read 2026-08-24) — the same list shadcn offers as its base colour, and the
/// reason these are quoted rather than invented: a neutral that carries hue is
/// a judgement someone else has already made five times.
pub const BASE_COLORS: [(&str, Tint); 5] = [
    ("Neutral", Tint::NONE),
    ("Stone", Tint::new(58.071, 0.013)),
    ("Zinc", Tint::new(285.938, 0.016)),
    ("Gray", Tint::new(264.364, 0.027)),
    ("Slate", Tint::new(257.417, 0.046)),
];

/// The alpha of each text role over the palette's ink — the colour of
/// [`Theme::text`] at full opacity. The shipped palettes' ink is white in dark
/// and black in light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextInk {
    /// [`Theme::text`].
    pub text: f32,
    /// [`Theme::text_muted`].
    pub muted: f32,
    /// [`Theme::text_faint`].
    pub faint: f32,
}

impl TextInk {
    /// The role at `alpha` over `appearance`'s ink.
    pub fn color(appearance: Appearance, alpha: f32) -> Hsla {
        match appearance {
            Appearance::Dark => gpui::hsla(0.0, 0.0, 1.0, alpha),
            Appearance::Light => gpui::hsla(0.0, 0.0, 0.0, alpha),
        }
    }

    /// Writes [`Theme::text`], [`Theme::text_muted`] and [`Theme::text_faint`]
    /// at these alphas over the colour `theme.text` already carries.
    pub fn paint(&self, theme: &mut Theme) {
        let ink = theme.text;
        theme.text = ink.alpha(self.text);
        theme.text_muted = ink.alpha(self.muted);
        theme.text_faint = ink.alpha(self.faint);
    }
}

/// Text ink for both appearances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ink {
    pub dark: TextInk,
    pub light: TextInk,
}

impl Ink {
    /// AppKit's label, secondary and tertiary label colours. Muted is under
    /// 4.5:1 on white, and faint is under 3:1 in both appearances.
    pub const APPKIT: Self = Self {
        dark: TextInk {
            text: 0.85,
            muted: 0.55,
            faint: 0.25,
        },
        light: TextInk {
            text: 0.85,
            muted: 0.50,
            faint: 0.26,
        },
    };

    /// Text and muted at 4.5:1 or more and faint at 3:1 or more, on
    /// [`Theme::bg`] and [`Theme::surface`] in both appearances.
    pub const ACCESSIBLE: Self = Self {
        dark: TextInk {
            text: 0.85,
            muted: 0.52,
            faint: 0.40,
        },
        light: TextInk {
            text: 0.85,
            muted: 0.56,
            faint: 0.43,
        },
    };

    pub fn for_appearance(&self, appearance: Appearance) -> TextInk {
        match appearance {
            Appearance::Dark => self.dark,
            Appearance::Light => self.light,
        }
    }
}

/// Whether the window composites translucent — AppKit's vibrancy, and Mica.
///
/// Three-valued rather than a bool because the honest answer depends on the
/// appearance, and the appearance moves under the app: the OS switches at
/// sunset. A bool would have to be rewritten by whoever noticed, and nothing
/// outside this crate is told when a palette is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vibrancy {
    /// Frost where the palette is built for it, which is dark alone.
    ///
    /// Light's glass tokens sit near 85% opacity — see [`Theme::glass_overlay`],
    /// [`Theme::card_glass_bg`] and [`Theme::input_glass_bg`], each tuned there
    /// because a translucent white tint left text ghosting over whatever sat
    /// behind it. Light pays for the blur and shows almost none of it, so it is
    /// not asked for unless an app says so.
    #[default]
    Auto,
    /// Frost in both appearances. What an app asks for when it has tuned its
    /// own light palette to carry one.
    On,
    /// Opaque in both. What a Reduce-transparency switch sets.
    Off,
}

impl Vibrancy {
    /// Whether to composite translucent for `appearance`.
    ///
    /// The platform gate rides on [`Vibrancy::Auto`] rather than on the caller:
    /// it asks for a frost only where the compositor paints one behind the
    /// window ([`frosted_window`](crate::frosted_window)), since a window that is merely transparent
    /// shows raw desktop through the sidebar. An app that knows its compositor
    /// says [`Vibrancy::On`] and gets it anywhere.
    pub fn on(self, appearance: Appearance) -> bool {
        match self {
            Self::Auto => crate::frosted_window() && matches!(appearance, Appearance::Dark),
            Self::On => true,
            Self::Off => false,
        }
    }
}

/// What an app changes about the shipped palette without redesigning it.
///
/// Installed as a gpui [`Global`]; [`Theme::install`] applies it to whatever
/// palette is registered, so it survives a light/dark switch and composes with
/// [`crate::AppExt::set_palette`](crate::AppExt::set_palette) rather than competing with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brand {
    /// The hue every grey in the palette carries.
    pub tint: Tint,
    /// The emphasis hue. Left neutral, the accent follows [`Self::tint`] like
    /// any other grey — which is what the shipped palette already is.
    pub accent: Tint,
    /// The base corner radius; every other corner is a ratio of it. See
    /// [`Theme::BASE_RADIUS`].
    pub radius: f32,
    /// How opaque the tint over the blurred window is. See
    /// [`Theme::VIBRANCY_ALPHA`], where it starts, and [`Theme::vibrancy_tint`].
    pub vibrancy_alpha: f32,
    /// The blur behind the translucent window. See [`Theme::window_blur`].
    pub window_blur: f32,
    /// Whether the window composites translucent, which is a question about
    /// the appearance as much as about the app — see [`Vibrancy`].
    pub vibrancy: Vibrancy,
    /// Whether components paint glass. Separate from [`Self::vibrancy`]:
    /// SwiftUI's material blends within the window, so an opaque window can
    /// still carry glass — which is what a Reduce-transparency setting asks
    /// for and the system's own does not do. The converse is a platform
    /// answer: a Mica window carries no glass, since the blur a card lays over
    /// the content it covers is [`LENSED`](crate::LENSED) and Mica is not.
    pub glass: bool,
    /// The text roles' alphas. [`Ink::APPKIT`] is what the palettes ship.
    pub ink: Ink,
}

impl Global for Brand {}

impl Default for Brand {
    fn default() -> Self {
        Self {
            tint: Tint::NONE,
            accent: Tint::NONE,
            radius: Theme::BASE_RADIUS,
            vibrancy_alpha: Theme::VIBRANCY_ALPHA,
            window_blur: Theme::WINDOW_BLUR,
            vibrancy: Vibrancy::Auto,
            glass: crate::LENSED,
            ink: Ink::APPKIT,
        }
    }
}

/// The accent's lightness in each appearance — indigo-400's and indigo-600's,
/// the two steps `palettes.rs` picked so an accent clears WCAG AA on its own
/// background rather than glowing on one and vanishing on the other.
const ACCENT_L: (f32, f32) = (0.673, 0.511);

/// The lightness of a *plate* carrying [`Theme::on_accent`], taken from
/// `danger_strong` — the palette's existing chromatic plate, already tuned to
/// hold a label in both appearances.
const PLATE_L: (f32, f32) = (0.58, 0.51);

impl Theme {
    /// The shipped palette for an appearance, rotated onto a brand. What
    /// [`Theme::install`] builds, without installing it — for previewing the
    /// appearance you are not currently painting.
    pub fn branded(brand: &Brand, appearance: Appearance) -> Self {
        let mut theme = Self::for_appearance(appearance);
        brand.apply(&mut theme);
        theme
    }
}

impl Brand {
    /// Rotate a palette onto this brand's hues.
    pub fn apply(&self, theme: &mut Theme) {
        // The one place the appearance and the brand are both in hand, and it
        // already runs on every install — which is every light/dark switch,
        // from the OS or from a person. Resolving here is what keeps vibrancy
        // right across a sunset without anyone outside watching for one.
        theme.vibrancy = self.vibrancy.on(theme.appearance);
        // Left where the brand put it, even when the window is opaque: every
        // reader of the tint is behind a `vibrancy` gate already
        // ([`Theme::window_bg`]), and an app that turns the frost back on
        // wants the coverage it tuned, not a value that was overwritten while
        // nothing was looking at it.
        theme.vibrancy_alpha = self.vibrancy_alpha;
        theme.window_blur = self.window_blur;
        theme.glass = self.glass;
        self.ink.for_appearance(theme.appearance).paint(theme);
        // Every colour token, with the rule doing the choosing: a token that is
        // already grey takes the tint, and one that already carries a hue —
        // danger, warning, success — is semantic and keeps it. Translucent ink
        // is skipped because it paints over whatever is beneath it, which is
        // tinted already.
        let tint = match theme.family {
            None => self.tint,
            Some(_) => Tint::NONE,
        };
        for (_, slot) in theme.tokens_mut() {
            if slot.a == 1.0 && slot.s <= f32::EPSILON {
                *slot = color::tint(*slot, tint.hue, tint.chroma);
            }
        }

        if self.accent.chroma > 0.0 {
            let light = theme.appearance == Appearance::Light;
            let (accent_l, plate_l) = if light {
                (ACCENT_L.1, PLATE_L.1)
            } else {
                (ACCENT_L.0, PLATE_L.0)
            };
            theme.accent = color::oklch(accent_l, self.accent.chroma, self.accent.hue);
            theme.accent_strong = color::oklch(plate_l, self.accent.chroma, self.accent.hue);
            // Whichever label the plate can actually hold. The shipped accent is
            // the maximum-contrast neutral, where the answer is always the
            // inverse; a chromatic plate at a yellow hue is bright enough that
            // the inverse would be the unreadable one.
            theme.on_accent = label_on(theme.accent_strong, theme);
        }
    }
}

/// Whichever of the palette's two extremes the plate can actually hold.
fn label_on(plate: Hsla, theme: &Theme) -> Hsla {
    let (a, b) = (theme.solid, theme.on_solid);
    if color::contrast_ratio(plate, a) >= color::contrast_ratio(plate, b) {
        a
    } else {
        b
    }
}

/// Read the installed brand (the default before one is set).
pub(crate) fn brand(cx: &App) -> Brand {
    cx.try_global::<Brand>().copied().unwrap_or_default()
}

/// Install a brand and repaint every window with it.
///
/// Colours are read imperatively at paint time, so nothing observes the theme
/// global — the same reason [`appearance::apply`](crate::appearance::apply)
/// refreshes rather than notifies.
pub(crate) fn set_brand(brand: Brand, cx: &mut App) {
    cx.set_global(brand);
    Theme::install(crate::paint::current_appearance(), cx);
    // Crossing 1.0 is what puts the `NSVisualEffectView` in or takes it out,
    // and nothing else does it — a repaint alone leaves a window that was
    // opaque at boot opaque forever.
    crate::appearance::reapply_window_background(cx);
    cx.refresh_windows();
}
