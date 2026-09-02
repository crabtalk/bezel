//! The app theme: one token set, two concrete instances.

use gpui::{Global, Hsla, SharedString};

use crate::{Appearance, paint};

mod glass;
mod install;
mod layout;
mod palettes;
mod syntax;
mod typography;

pub use install::set_palette;
pub use layout::{ControlSize, Sizing};
pub use syntax::{HighlightKind, SyntaxPalette};
pub use typography::{Metrics, TextStyle, Typeset, base_text_size, set_base_text_size};

/// The two shipped glasses — SwiftUI's `Glass.regular` and `Glass.clear`. A
/// closed variant rather than knobs: Apple exposes no numbers on glass either,
/// only the variant and a tint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glass {
    /// The everyday material: it blurs what it covers and dims it hard.
    Regular,
    /// Near-transparent — the backdrop reads through, bent only at the rim.
    Clear,
}

/// SwiftUI's frost scale. Measured 2026-08-31: the five thicknesses are ONE
/// material at five opacities — the tone implied by `tint / (1 - gain)` holds
/// to within 9% across the scale, and the sigma does not move at all
/// (23.1/20.2/21.0pt). So this is a knob, not five looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    UltraThin,
    Thin,
    Regular,
    Thick,
    UltraThick,
}

impl Material {
    /// How much of the backdrop the material covers. Measured off SwiftUI in
    /// dark; the steps come out even to within a point.
    pub fn opacity(self) -> f32 {
        match self {
            Material::UltraThin => 0.440,
            Material::Thin => 0.543,
            Material::Regular => 0.638,
            Material::Thick => 0.737,
            Material::UltraThick => 0.825,
        }
    }
}

/// Which surface a caller names. Material and glass are different things with
/// different vocabularies — a material has thickness, a glass has a variant —
/// and they meet only at the numbers they resolve to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceStyle {
    Material(Material),
    Glass(Glass),
}

/// The frost material, before a thickness picks its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialSpec {
    /// The material's own tone, at full coverage.
    pub tone: Hsla,
    /// Its chroma push, as [`SurfaceSpec::saturation`].
    pub saturation: f32,
    /// Its sigma, which does not move with thickness.
    pub blur: f32,
    /// SwiftUI's frost has no lit rim at all — measured +0 at the boundary.
    /// What is here is bezel's own hairline, the one the popover card used to
    /// draw, moved to the surface that owes it.
    pub edge: f32,
    pub edge_width: f32,
    pub edge_aa: f32,
}

impl MaterialSpec {
    /// This material at one thickness.
    pub fn at(&self, thickness: Material) -> SurfaceSpec {
        let opacity = thickness.opacity();
        SurfaceSpec {
            gain: 1.0 - opacity,
            saturation: self.saturation,
            tint: self.tone.opacity(opacity),
            blur: self.blur,
            rim: 0.0,
            reach: 0.0,
            edge: self.edge,
            edge_width: self.edge_width,
            edge_aa: self.edge_aa,
            // A wash has nothing to bend, so it needs one to lift off the page.
            shadow: true,
        }
    }
}

/// One surface's numbers. `out = gain * saturated(backdrop) + tint`, over a
/// backdrop blurred at `blur`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSpec {
    /// Slope of the transfer line. Below 1 compresses contrast toward
    /// `tint`; above 1 it brightens and slightly expands, which is what light
    /// `Clear` measures. Named for what it is, not for one of its directions.
    pub gain: f32,
    /// How far the backdrop's chroma is pushed from its own grey, before the
    /// gain drops the level. A gain alone moves level and colour together, so
    /// this is the only way a surface goes dark and keeps its colours. 1 is
    /// the pass-through `Clear` measures.
    pub saturation: f32,
    /// Its offset — the look's own tone.
    pub tint: Hsla,
    /// Gaussian sigma under the lens, in logical pixels. It stands in for the
    /// average attenuation of fine detail, which the real material gets from
    /// snapshotting its backdrop coarsely.
    pub blur: f32,
    /// How far in from the rim the lens bends the backdrop, in logical pixels.
    /// A length rather than a share of the box: the displacement curve measured
    /// 2026-08-30 is one curve, the same from a 96pt box to a 320pt one and
    /// from r24 to r84. Past it the backdrop is untouched.
    pub rim: f32,
    /// How far the outermost pixel drags what it samples, in logical pixels —
    /// the far end of that same curve, measured at ~47pt.
    pub reach: f32,
    /// How much white the lit rim adds at the edge itself, 0..1.
    pub edge: f32,
    /// How far in that light falls off to nothing, in logical pixels. Even the
    /// whole way round: measured 2026-08-30, the real rim reads the same on all
    /// four sides.
    pub edge_width: f32,
    /// The coverage ramp at the shape's own boundary, in logical pixels. 0.5
    /// is one device pixel at 2x — the plain rasteriser's answer; 0 is the hard
    /// edge it replaced.
    pub edge_aa: f32,
    /// Whether the surface casts a shadow. A lens separates itself from the
    /// page by bending it, and reads as a slab under one; a wash has nothing
    /// to bend and needs it.
    pub shadow: bool,
}

impl SurfaceStyle {
    /// The numbers this style resolves to against a theme.
    pub fn spec(self, theme: &Theme) -> SurfaceSpec {
        match self {
            SurfaceStyle::Material(thickness) => theme.material.at(thickness),
            SurfaceStyle::Glass(Glass::Regular) => theme.glass_regular,
            SurfaceStyle::Glass(Glass::Clear) => theme.glass_clear,
        }
    }
}

/// The app theme. Two concrete instances — [`Theme::dark`] and [`Theme::light`].
#[derive(Debug, Clone)]
pub struct Theme {
    /// Which appearance these tokens were built for.
    pub appearance: Appearance,

    // ---- paint: neutral surfaces ----
    /// Main content panel. Dark: the deepest plane (#060606). Light: pure white —
    /// long-form content reads best on an unbroken white field.
    pub bg: Hsla,
    /// Shell / sidebar surface. Dark: one step *up* from `bg`. Light: one step
    /// *down* (grey) — chrome recedes from the content plane in both, which is
    /// the direction a naive invert gets backwards.
    pub surface: Hsla,
    /// Raised surface: opaque pills and chips that sit proud of the panel.
    /// Dark: lighter than `surface`. Light: white, separated by `border` +
    /// shadow rather than by lightness.
    pub surface_raised: Hsla,

    // ---- paint: elevation ladder ----
    //
    // Dark mode distinguishes floating planes by lightness, and the steps are
    // *small* (#0e → #10 → #16 → #1e). They are not interchangeable: collapsing
    // them onto one token visibly lifts popovers off their intended plane.
    //
    // Light mode cannot use the same trick, because the content plane is already
    // white and there is nothing lighter to climb to. All three land on white and
    // let `border` + shadow carry the separation instead — the standard light-UI
    // answer, and the reason this is a ladder of tokens rather than an arithmetic
    // offset applied to one.
    /// Inline card resting on the main panel (auth gate, empty-state cards).
    pub surface_card: Hsla,
    /// Modal dialog, floating over a [`Theme::scrim`].
    pub surface_dialog: Hsla,
    /// Popover, menu and command-palette surface — the highest plane.
    pub surface_overlay: Hsla,
    /// Hover wash for interactive rows and buttons, on glass and off it alike:
    /// `../desktop`'s `--color-hover`.
    pub element_hover: Hsla,
    /// Active/selected wash, one rung over the hover — `--color-active`.
    pub element_active: Hsla,
    /// Hairline border.
    pub border: Hsla,
    /// Stronger border for focused/raised edges.
    pub border_strong: Hsla,

    // ---- paint: text ----
    /// Primary text. ~17.5:1 on its own background in both appearances.
    pub text: Hsla,
    /// Muted text: timestamps, secondary labels. ~7.5–8:1.
    pub text_muted: Hsla,
    /// Faint text: placeholders, disabled. ~4.5:1 — AA for body copy.
    pub text_faint: Hsla,
    /// One notch below `text_muted` — the diff file-path tone. It exists as its
    /// own token rather than being folded into `text_muted` because the dark
    /// value was sampled (#989898) and folding it would shift that label, which
    /// is a palette change dressed up as a refactor.
    pub text_dim: Hsla,

    // ---- paint: high-contrast solid (primary buttons) ----
    /// The maximum-contrast solid fill: near-white on dark, near-black on light.
    /// This is the primary button plate.
    pub solid: Hsla,
    /// Label/icon color on top of [`Self::solid`] — its inverse.
    pub on_solid: Hsla,

    // ---- paint: accents ----
    /// Accent — the emphasis weight for text and icons.
    ///
    /// **Neutral by default, and deliberately.** A component library that ships
    /// a hue puts that hue in every app that installs it, and bezel had an
    /// indigo running through spinners, pagination, date selection and list
    /// markers before anyone chose it. The default is now the same lightness
    /// with the chroma at zero.
    ///
    /// This is the token to brand:
    ///
    /// ```ignore
    /// let mut theme = Theme::for_appearance(appearance);
    /// theme.accent = my_brand_accent(appearance);
    /// ```
    ///
    /// See [`set_palette`](crate::theme::install::set_palette), which is what
    /// makes an override survive an appearance switch.
    pub accent: Hsla,
    /// Stronger accent for fills that carry [`Self::on_accent`] text. Neutral by
    /// default, it is the maximum-contrast plate — a mid grey would not carry a
    /// label the way the indigo it replaced did.
    pub accent_strong: Hsla,
    /// Label color on top of [`Self::accent_strong`].
    pub on_accent: Hsla,
    /// Danger — red (errors, stop button).
    pub danger: Hsla,
    /// Softer danger for secondary/inline error copy.
    pub danger_muted: Hsla,
    /// Warning — amber (offline notices, awaiting-input).
    pub warning: Hsla,
    /// Softer warning for secondary copy.
    pub warning_muted: Hsla,
    /// Success / online — emerald.
    pub success: Hsla,
    /// Working / streaming indicator — pink.
    pub busy: Hsla,
    /// Softer success for text on a success-tinted chip.
    pub success_muted: Hsla,

    // ---- paint: components ----
    /// Hover tone for an *opaque* raised pill. Hover must brighten the plate in
    /// dark mode, never swap it for a translucent wash (that made pills go
    /// see-through — user-reported); in light mode it darkens instead, same idea.
    pub surface_raised_hover: Hsla,
    /// Recessed band behind a palette/picker header or footer strip. Translucent
    /// so the glass still reads through.
    pub band: Hsla,
    /// The composer pill and other input plates.
    ///
    /// Its own token because "lifted" inverts between appearances. On dark, a
    /// faint *white* wash over near-black reads as raised. The literal light
    /// translation — a faint *black* wash on white — reads as **recessed**, a dent
    /// rather than a plate, which is why the prompt looked like bare text on a
    /// smudge. Light mode lifts the way light UIs actually do: pure white, with
    /// the border and shadow carrying the elevation.
    pub input_bg: Hsla,
    /// Text-selection highlight in the composer and inputs.
    pub selection: Hsla,
    /// Terminal block cursor.
    pub cursor: Hsla,
    /// Text caret — [`Self::accent`]'s lightness.
    ///
    /// Measured macOS 26, 2026-08-31: `NSColor.textInsertionPointColor` is the
    /// accent in both appearances, where `NSColor.textColor` is white and
    /// black. So a caret is not the next glyph before you type it, which is
    /// what this used to carry; the platform gives it its own role, and a brand
    /// tints it here the way the system accent tints it there.
    pub caret: Hsla,
    /// Keyboard focus ring — a hairline, so it marks the control without
    /// restating the label inside it.
    pub ring: Hsla,
    /// Destructive-action button fill (danger plate, carries [`Self::on_accent`]).
    pub danger_strong: Hsla,

    // ---- paint: code & diff ----
    /// Inline-code text. Neutral: code is already set apart by the mono face
    /// and its wash, and a hue on top reads as a link rather than as code.
    pub code_text: Hsla,
    /// Inline-code wash behind [`Self::code_text`].
    pub code_wash: Hsla,
    /// Shared paint-only syntax palette.
    pub syntax: SyntaxPalette,
    /// Diff: added lines.
    pub diff_add: Hsla,
    /// Diff: deleted lines.
    pub diff_del: Hsla,
    /// Diff: hunk-header wash (bluish grey).
    pub diff_hunk_bg: Hsla,

    // ---- glass ----
    //
    // Numbers, so they flow with the appearance the way every other token
    // does. `Glass::glass_effect` reads them off the theme it is handed;
    // nothing here is a parameter on a component.
    /// How opaque the tint over the blurred window is.
    pub vibrancy_alpha: f32,
    /// Whether the window composites translucent, so the desktop reaches what
    /// is painted over it — AppKit's vibrancy.
    pub vibrancy: bool,
    /// Whether components paint glass — translucent popovers and cards, and
    /// the lens. An opaque window can still carry it.
    pub glass: bool,
    /// The surfaces this theme can paint. Blur belongs to the look, not to
    /// the caller: Apple exposes no blur parameter on either family, only the
    /// thickness or the variant.
    pub material: MaterialSpec,
    pub glass_regular: SurfaceSpec,
    pub glass_clear: SurfaceSpec,
    /// What the popover surfaces — menus, dialogs, sheets, tooltips — mount
    /// on. They take no theme of their own, so this is where the choice
    /// lives; a component that owns its surface names its own style instead.
    pub popover_surface: SurfaceStyle,
    /// Lens displacement amplitude, signed; negative inverts it.
    pub glass_magnify: f32,
    /// Per-channel spread of that displacement — the chromatic fringe.
    pub glass_dispersion: f32,

    // ---- fonts ----
    /// UI font family — the name the text system resolves, not the bytes. Point
    /// it at your own family once you have registered that font with the text
    /// system; a family nothing registered falls through to the fallback below.
    pub font_sans: SharedString,
    /// Monospace family for code/terminal.
    pub font_mono: SharedString,
    /// Explicit system fallbacks, for callers that want to skip the lookup.
    pub font_sans_fallback: SharedString,
    pub font_mono_fallback: SharedString,
}

impl Theme {
    /// Overlay ink at `alpha` — see [`ink`](crate::paint::ink).
    pub fn ink(&self, alpha: f32) -> Hsla {
        paint::ink_for(self.appearance, alpha)
    }

    /// Hairline ink at `alpha` — see [`hairline`](crate::paint::hairline).
    pub fn hairline(&self, alpha: f32) -> Hsla {
        paint::hairline_for(self.appearance, alpha)
    }

    /// State wash at `alpha` — see [`wash`](crate::paint::wash).
    pub fn wash(&self, alpha: f32) -> Hsla {
        paint::wash_for(self.appearance, alpha)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Global for Theme {}
