//! Brand: one hue for the greys, one for the accent, one radius.
//!
//! The two palettes in `palettes.rs` are designed — every lightness in them was
//! tuned against a measured contrast ratio, and light is not dark inverted. A
//! brand does not replace that work; it rotates it. Lightness is never a knob
//! here, so a branded palette keeps the contrast the shipped one was verified
//! at, and the only thing that moves is hue.

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

/// What an app changes about the shipped palette without redesigning it.
///
/// Installed as a gpui [`Global`]; [`Theme::install`] applies it to whatever
/// palette is registered, so it survives a light/dark switch and composes with
/// [`set_palette`](crate::set_palette) rather than competing with it.
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
    /// How opaque the frost over the blurred window is — `1.0` is opaque, and
    /// turns glass off entirely. See [`Theme::GLASS_ALPHA`], which is where it
    /// starts, and [`Theme::glass`].
    pub glass: f32,
}

impl Global for Brand {}

impl Default for Brand {
    fn default() -> Self {
        Self {
            tint: Tint::NONE,
            accent: Tint::NONE,
            radius: Theme::BASE_RADIUS,
            glass: Theme::GLASS_ALPHA,
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
        // Every colour token, with the rule doing the choosing: a token that is
        // already grey takes the tint, and one that already carries a hue —
        // danger, warning, success — is semantic and keeps it. Translucent ink
        // is skipped because it paints over whatever is beneath it, which is
        // tinted already.
        let tokens: [&mut Hsla; 38] = [
            &mut theme.bg,
            &mut theme.surface,
            &mut theme.surface_raised,
            &mut theme.surface_card,
            &mut theme.surface_dialog,
            &mut theme.surface_overlay,
            &mut theme.element_hover,
            &mut theme.element_active,
            &mut theme.border,
            &mut theme.border_strong,
            &mut theme.text,
            &mut theme.text_muted,
            &mut theme.text_faint,
            &mut theme.text_dim,
            &mut theme.solid,
            &mut theme.on_solid,
            &mut theme.accent,
            &mut theme.accent_strong,
            &mut theme.on_accent,
            &mut theme.danger,
            &mut theme.danger_muted,
            &mut theme.warning,
            &mut theme.warning_muted,
            &mut theme.success,
            &mut theme.busy,
            &mut theme.success_muted,
            &mut theme.surface_raised_hover,
            &mut theme.band,
            &mut theme.input_bg,
            &mut theme.selection,
            &mut theme.cursor,
            &mut theme.caret,
            &mut theme.danger_strong,
            &mut theme.code_text,
            &mut theme.code_wash,
            &mut theme.diff_add,
            &mut theme.diff_del,
            &mut theme.diff_hunk_bg,
        ];
        let syntax: [&mut Hsla; 24] = [
            &mut theme.syntax.comment,
            &mut theme.syntax.keyword,
            &mut theme.syntax.string,
            &mut theme.syntax.string_special,
            &mut theme.syntax.escape,
            &mut theme.syntax.number,
            &mut theme.syntax.boolean,
            &mut theme.syntax.type_name,
            &mut theme.syntax.type_builtin,
            &mut theme.syntax.constructor,
            &mut theme.syntax.function,
            &mut theme.syntax.function_builtin,
            &mut theme.syntax.macro_name,
            &mut theme.syntax.property,
            &mut theme.syntax.constant,
            &mut theme.syntax.variable,
            &mut theme.syntax.variable_special,
            &mut theme.syntax.parameter,
            &mut theme.syntax.operator,
            &mut theme.syntax.punctuation,
            &mut theme.syntax.tag,
            &mut theme.syntax.attribute,
            &mut theme.syntax.label,
            &mut theme.syntax.invalid,
        ];
        for slot in tokens.into_iter().chain(syntax) {
            if slot.a == 1.0 && slot.s <= f32::EPSILON {
                *slot = color::tint(*slot, self.tint.hue, self.tint.chroma);
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
pub fn brand(cx: &App) -> Brand {
    cx.try_global::<Brand>().copied().unwrap_or_default()
}

/// Install a brand and repaint every window with it.
///
/// Colours are read imperatively at paint time, so nothing observes the theme
/// global — the same reason [`appearance::apply`](crate::appearance::apply)
/// refreshes rather than notifies.
pub fn set_brand(brand: Brand, cx: &mut App) {
    cx.set_global(brand);
    Theme::install(crate::paint::current_appearance(), cx);
    // Crossing 1.0 is what puts the `NSVisualEffectView` in or takes it out,
    // and nothing else does it — a repaint alone leaves a window that was
    // opaque at boot opaque forever.
    crate::appearance::reapply_window_background(cx);
    cx.refresh_windows();
}
