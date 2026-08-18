//! The two concrete palettes: dark and light.

use gpui::hsla;

use crate::{
    Appearance, color, paint,
    theme::{Theme, syntax::SyntaxPalette},
};

impl Theme {
    /// Build the dark theme. The surface tones are sampled straight from the
    /// reference screenshots of the original app (docs/reference): main panel
    /// `#060606`, shell/sidebar `#0d0d0d`.
    pub fn dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            bg: color::grey(6),       // main panel — sampled #060606
            surface: color::grey(13), // shell / sidebar — sampled #0d0d0d
            surface_raised: color::neutral(0.235),
            surface_card: color::grey(0x0e),
            surface_dialog: color::grey(0x10),
            surface_overlay: color::grey(0x16),
            element_hover: hsla(0.0, 0.0, 0.92, 0.11),
            element_active: hsla(0.0, 0.0, 0.92, 0.16),
            border: hsla(0.0, 0.0, 1.0, 0.08),
            border_strong: hsla(0.0, 0.0, 1.0, 0.14),
            text: color::neutral(0.922),       // ~neutral-200
            text_muted: color::neutral(0.708), // ~neutral-400
            text_faint: color::neutral(0.556), // ~neutral-500
            text_dim: color::grey(0x98),
            solid: color::neutral(0.922),         // near-white plate
            on_solid: color::grey(0x0e),          // near-black label
            accent: color::neutral(0.673),        // indigo-400's lightness, no chroma
            accent_strong: color::neutral(0.922), // the solid plate
            on_accent: color::grey(0x0e),         // its inverse label
            danger: color::oklch(0.704, 0.191, 22.216), // red-400
            danger_muted: color::oklch(0.808, 0.114, 19.571), // red-300
            warning: color::oklch(0.828, 0.189, 84.429), // amber-400
            warning_muted: color::oklch(0.924, 0.12, 95.746), // amber-200
            success: color::oklch(0.765, 0.177, 163.223), // emerald-400
            busy: color::oklch(0.718, 0.202, 349.761), // pink-400
            success_muted: color::oklch(0.845, 0.143, 164.978), // emerald-300
            surface_raised_hover: color::neutral(0.29),
            band: paint::band_for(Appearance::Dark),
            input_bg: hsla(0.0, 0.0, 1.0, 0.03),
            selection: hsla(0.66, 0.6, 0.55, 0.35),
            cursor: hsla(0.0, 0.0, 1.0, 0.35),
            caret: color::neutral(0.922), // the body text colour
            danger_strong: color::oklch(0.58, 0.16, 25.0),
            code_text: color::neutral(0.94), // near-white, a shade above body text
            code_wash: hsla(0.0, 0.0, 1.0, 0.08), // white/8
            syntax: SyntaxPalette::dark(
                color::neutral(0.922),
                color::neutral(0.60),
                color::oklch(0.704, 0.191, 22.216),
            ),
            diff_add: color::oklch(0.765, 0.177, 163.223), // emerald-400
            diff_del: color::oklch(0.704, 0.191, 22.216),  // red-400
            diff_hunk_bg: hsla(0.6, 0.35, 0.6, 0.05),
            font_sans: "Geist".into(),
            font_mono: "Geist Mono".into(),
            font_sans_fallback: system_sans().into(),
            font_mono_fallback: system_mono().into(),
        }
    }

    /// Build the light theme.
    ///
    /// Neutrals are the same oklch scale read from the other end, but the *roles*
    /// are reassigned rather than mirrored (see the module docs): content plane
    /// white, chrome grey, raised surfaces white-plus-shadow. Text tones are
    /// picked to reproduce the dark theme's contrast ratios, and accents drop
    /// from the 400 to the 600 step at identical hue so they clear WCAG AA on
    /// white instead of glowing.
    pub fn light() -> Self {
        Self {
            appearance: Appearance::Light,
            bg: color::grey(0xff), // main panel — clean white
            // Deeper than ~neutral-100 looks on paper: the content card is pure
            // white and sits *inside* this surface, so too small a step leaves the
            // whole window one flat sheet with a hairline drawn on it.
            surface: color::neutral(0.968),
            // A real grey, NOT white. This is the opaque-plate tone — user
            // message bubbles, the jump-to-bottom pill — and those sit directly
            // on the white content plane with no border or shadow to save them.
            // White here made the user's own messages vanish into the page.
            // Popovers do not use this; they have their own ladder below.
            surface_raised: color::neutral(0.940),
            surface_card: color::grey(0xff),
            surface_dialog: color::grey(0xff),
            surface_overlay: color::grey(0xff),
            element_hover: hsla(0.0, 0.0, 0.10, 0.06),
            element_active: hsla(0.0, 0.0, 0.10, 0.10),
            border: hsla(0.0, 0.0, 0.0, 0.10),
            border_strong: hsla(0.0, 0.0, 0.0, 0.17),
            // ~neutral-850. Pure neutral-900 measures 17.9:1 on white — *more*
            // contrast than dark mode's 16.1:1, which reads as harsh rather than
            // crisp. Backing off to 0.25 lands at ~16:1: the same perceived
            // weight as the dark theme, not the maximum available.
            text: color::neutral(0.25),
            text_muted: color::neutral(0.439), // ~neutral-600 → ~7.7:1
            // A touch darker than dark mode's neutral-500 counterpart: the light
            // sidebar is a real grey, and faint text has to clear its floor there
            // too, not just on the white content plane.
            text_faint: color::neutral(0.535),
            text_dim: color::neutral(0.50),
            solid: color::neutral(0.205), // near-black plate, deeper than body text
            on_solid: color::neutral(0.985), // near-white label
            accent: color::neutral(0.511), // indigo-600's lightness, no chroma
            accent_strong: color::neutral(0.205), // the solid plate
            on_accent: color::neutral(0.985), // its inverse label
            danger: color::oklch(0.577, 0.245, 27.325), // red-600
            danger_muted: color::oklch(0.505, 0.213, 27.518), // red-700
            warning: color::oklch(0.555, 0.163, 48.998), // amber-700 — carries 12px text
            warning_muted: color::oklch(0.473, 0.137, 46.201), // amber-800
            success: color::oklch(0.596, 0.145, 163.225), // emerald-600
            busy: color::oklch(0.592, 0.249, 0.584), // pink-600
            success_muted: color::oklch(0.508, 0.118, 165.612), // emerald-700
            // Opaque pills darken on hover here rather than brighten — same
            // "brighten the plate, don't wash it out" rule, read the other way.
            surface_raised_hover: color::neutral(0.900),
            // A recessed strip on white needs far less ink than on near-black;
            // the dark 16% would read as a bruise.
            band: paint::band_for(Appearance::Light),
            input_bg: color::grey(0xff),
            selection: hsla(0.66, 0.75, 0.62, 0.28),
            cursor: hsla(0.0, 0.0, 0.0, 0.55),
            caret: color::neutral(0.25), // the body text colour
            danger_strong: color::oklch(0.51, 0.20, 25.0),
            code_text: color::neutral(0.18), // near-black, a shade under body text
            code_wash: hsla(0.0, 0.0, 0.0, 0.06), // black/6
            syntax: SyntaxPalette::light(
                color::neutral(0.25),
                color::neutral(0.48),
                color::oklch(0.505, 0.213, 27.518),
            ),
            diff_add: color::oklch(0.596, 0.145, 163.225), // emerald-600
            diff_del: color::oklch(0.577, 0.245, 27.325),  // red-600
            diff_hunk_bg: hsla(0.6, 0.35, 0.35, 0.07),
            font_sans: "Geist".into(),
            font_mono: "Geist Mono".into(),
            font_sans_fallback: system_sans().into(),
            font_mono_fallback: system_mono().into(),
        }
    }

    /// Build the theme for an appearance.
    pub fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Dark => Self::dark(),
            Appearance::Light => Self::light(),
        }
    }
}

fn system_sans() -> &'static str {
    if cfg!(target_os = "macos") {
        "Helvetica"
    } else if cfg!(target_os = "windows") {
        "Segoe UI"
    } else {
        "DejaVu Sans"
    }
}

fn system_mono() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(target_os = "windows") {
        "Consolas"
    } else {
        "DejaVu Sans Mono"
    }
}
