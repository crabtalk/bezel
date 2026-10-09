//! The two concrete palettes: dark and light.

use gpui::{hsla, rgb};

use crate::{
    Appearance, Ink, TextInk, color, paint,
    theme::{
        Glass, Material, MaterialSpec, SurfaceSpec, SurfaceStyle, Theme, syntax::SyntaxPalette,
    },
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
            // `../desktop`'s `--color-hover`, pure ink: white at 8%.
            element_hover: hsla(0.0, 0.0, 1.0, 0.08),
            // `--color-active`, a rung above the hover: white at 12%.
            element_active: hsla(0.0, 0.0, 1.0, 0.12),
            border_faint: paint::hairline_for(Appearance::Dark, 0.06),
            border: paint::hairline_for(Appearance::Dark, 0.08),
            border_strong: paint::hairline_for(Appearance::Dark, 0.14),
            text: TextInk::color(Appearance::Dark, Ink::APPKIT.dark.text),
            text_muted: TextInk::color(Appearance::Dark, Ink::APPKIT.dark.muted),
            text_faint: TextInk::color(Appearance::Dark, Ink::APPKIT.dark.faint),
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
            selection: rgb(0x3f638b).into(),
            cursor: color::neutral(0.94), // near-white, opaque
            caret: color::neutral(0.673), // the accent
            ring: paint::hairline_for(Appearance::Dark, 0.35), // ~2.5× border_strong
            drop_line: paint::hairline_for(Appearance::Dark, 0.35),
            drop_target: hsla(0.0, 0.0, 1.0, 0.10),
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
            // One small step up from `bg`: the terminal reads as its own pane
            // without becoming a lighter box.
            terminal_bg: color::grey(0x09),
            terminal_ansi: ansi(ANSI_DARK),
            vibrancy_alpha: Self::VIBRANCY_ALPHA,
            // Darker than `surface`: the reference vibrancy scrim, `hsl(0 0% 3%)`.
            vibrancy_tone: color::grey(8),
            window_blur: Self::WINDOW_BLUR,
            vibrancy: crate::frosted_window(),
            glass: crate::LENSED,
            // SwiftUI's frost, measured 2026-08-31: `tint / (1 - gain)` implies
            // one tone across all five thicknesses (49.8 down to 45.3), and the
            // sigma does not move with them. The rim is bezel's, not Apple's —
            // SwiftUI's material has no lit edge at all, and the popover card
            // used to draw this hairline itself.
            material: MaterialSpec {
                tone: color::grey(47),
                saturation: 2.1,
                blur: 21.0,
                edge: 0.10,
                edge_width: 1.0,
                edge_aa: 0.5,
            },
            // Measured 2026-08-31 off a real NSGlassEffectView, one
            // whole-canvas tone at a time: a fill the size of the probe cannot
            // be contaminated by a 10pt blur, which is what every earlier
            // reading of this look got wrong. Six greys give the line at rms
            // 0.9 levels, four saturated tones agree on the saturation to 0.1,
            // and the sigma is the gaussian that best fits a 70pt bar edge at
            // the centre of a 320pt glass, rms 0.4 levels. Over a backdrop that
            // is NOT locally flat the real material saturates less than these
            // numbers reproduce, so its saturation is not the per-pixel one
            // this models. Rim and lit edge are `Clear`'s.
            glass_regular: SurfaceSpec {
                gain: 0.311,
                saturation: 2.55,
                tint: gpui::hsla(0.0, 0.0, 1.0, 11.0 / 255.0),
                // An on-screen choice, 2026-09-01. The measurement above reads
                // 10.8, at which the interior is a flat wash and every seam in
                // the lens shows as a step in it.
                blur: 4.0,
                rim: 18.75,
                reach: 47.0,
                // Measured 2026-08-31 over a black canvas, where the coverage
                // blend can only pull down, so anything above the interior is
                // rim light: +25 levels at the boundary and gone by 1pt, the
                // same in both appearances. Clear's 0.26 was read over a bright
                // backdrop, where the blend toward a brighter outside inflates
                // it — at that value the rim reads as a drawn border. It scales
                // with what is behind it (+25 over black, +36 over mid grey)
                // where this is a constant, so it is one point on their curve.
                edge: 0.10,
                edge_width: 1.0,
                edge_aa: 0.5,
                shadow: false,
            },
            // `Clear` refit 2026-08-30 over the gallery's own backdrops, rms
            // 0.4 levels on backdrop 0..212. A window that is not key carries a
            // different material, and this is the key one. The sigma is off a
            // 2pt rule, which a 48pt band is too wide to resolve. The rim is
            // off the position-coded backdrop, pooled over four shapes from
            // 96pt to 320pt and r24 to r84: one curve, rms 1.8pt.
            glass_clear: SurfaceSpec {
                gain: 1.029,
                saturation: 1.0,
                tint: gpui::hsla(0.0, 0.0, 1.0, 16.0 / 255.0),
                blur: 1.2,
                rim: 18.75,
                reach: 47.0,
                edge: 0.26,
                edge_width: 1.4,
                edge_aa: 0.5,
                shadow: false,
            },
            // Matched on screen against a real NSGlassEffectView, which is a
            // higher bar than the dome's algebra: fitting the formula to their
            // measured curve lands ~15% short of what the shader then renders.
            popover_surface: SurfaceStyle::Glass(Glass::Regular),
            drop_preview: SurfaceStyle::Material(Material::Thin),
            carried_surface: SurfaceStyle::Glass(Glass::Regular),
            glass_magnify: 1.1,
            glass_dispersion: 0.005,
            font_sans: SYSTEM_SANS.into(),
            font_body: SYSTEM_SANS.into(),
            font_mono: system_mono().into(),
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
            // The same token in light: black at 4%.
            element_hover: hsla(0.0, 0.0, 0.0, 0.04),
            // The same rung in light: black at 6%.
            element_active: hsla(0.0, 0.0, 0.0, 0.06),
            border_faint: paint::hairline_for(Appearance::Light, 0.06),
            border: paint::hairline_for(Appearance::Light, 0.08),
            border_strong: paint::hairline_for(Appearance::Light, 0.14),
            text: TextInk::color(Appearance::Light, Ink::APPKIT.light.text),
            text_muted: TextInk::color(Appearance::Light, Ink::APPKIT.light.muted),
            text_faint: TextInk::color(Appearance::Light, Ink::APPKIT.light.faint),
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
            selection: rgb(0xb3d7ff).into(),
            cursor: color::neutral(0.205), // near-black, opaque
            caret: color::neutral(0.511),  // the accent
            ring: paint::hairline_for(Appearance::Light, 0.35),
            drop_line: paint::hairline_for(Appearance::Light, 0.35),
            drop_target: hsla(0.0, 0.0, 0.0, 0.06),
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
            // One step down from the white `bg`. Larger than dark's 3/255: a
            // near-white delta that separates on near-black vanishes on white.
            terminal_bg: color::grey(0xfa),
            terminal_ansi: ansi(ANSI_LIGHT),
            vibrancy_alpha: Self::VIBRANCY_ALPHA,
            // The material's own measured tone.
            vibrancy_tone: color::grey(235),
            window_blur: Self::WINDOW_BLUR,
            vibrancy: crate::frosted_window(),
            glass: crate::LENSED,
            // Measured 2026-08-30, macOS 26.3 LIGHT, same instruments. The
            // material is not a tone-flip of dark: `regular` keeps its opacity
            // (86%) and swaps a 19% grey base for a 97% white one, which is why
            // it reads as ordinary frost here. `clear` stops compressing
            // altogether — it is very nearly a pure lift.
            // Material: the menu surface before glass, kept as a look of its own
            // because it stays readable over content a lens would only bend.
            // Light's own tone, same instrument: gain 0.378 at `Regular`, so
            // opacity 0.622 against dark's 0.638 — the scale is shared and only
            // the tone moves. The sigma is dark's; it is a SwiftUI constant,
            // not an appearance choice.
            material: MaterialSpec {
                tone: color::grey(235),
                saturation: 2.1,
                blur: 21.0,
                edge: 0.10,
                edge_width: 1.0,
                edge_aa: 0.5,
            },
            // The real material measured 2026-08-31, same instrument as dark:
            // gain 0.139, tint white at 214/255, saturation 4.27 (within 0.1
            // over four hues). 84% of its output is tint.
            //
            // Shipped off that pair since 2026-09-23: a third of the tint
            // traded for the backdrop it was covering, holding the product
            // `tint.a / (1 - gain)` that [`SurfaceSpec::flat`] reads, so a
            // build with no lens keeps the white panel it had. Saturation
            // comes down with it — 4.27 was fit at 13.9% transmission and
            // oversaturates at 40%. Rim and lit edge are dark's, unmeasured
            // here.
            glass_regular: SurfaceSpec {
                gain: 0.40,
                saturation: 2.2,
                tint: gpui::hsla(0.0, 0.0, 1.0, 150.0 / 255.0),
                blur: 8.9,
                rim: 18.75,
                reach: 47.0,
                edge: 0.10,
                edge_width: 1.0,
                edge_aa: 0.5,
                shadow: false,
            },
            // Carries dark's rim, sigma and edge: light has not been
            // re-measured since the instrument learned to hold the window key.
            glass_clear: SurfaceSpec {
                gain: 1.041,
                saturation: 1.0,
                tint: gpui::hsla(0.0, 0.0, 1.0, 18.8 / 255.0),
                blur: 1.2,
                rim: 18.75,
                reach: 47.0,
                edge: 0.26,
                edge_width: 1.4,
                edge_aa: 0.5,
                shadow: false,
            },
            // At 1-3pt inside the rim the real material drags the backdrop 26pt
            // or more, and lets go by 5.5pt; on the shader's profile that is 8.
            popover_surface: SurfaceStyle::Glass(Glass::Regular),
            drop_preview: SurfaceStyle::Material(Material::Thin),
            carried_surface: SurfaceStyle::Glass(Glass::Regular),
            glass_magnify: 1.1,
            glass_dispersion: 0.005,
            font_sans: SYSTEM_SANS.into(),
            font_body: SYSTEM_SANS.into(),
            font_mono: system_mono().into(),
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

/// The 16 ANSI colours on the dark terminal background.
const ANSI_DARK: [u32; 16] = [
    0x242424, // black — visible against #090909
    0xf87171, // red
    0x4ade80, // green
    0xfacc15, // yellow
    0x60a5fa, // blue
    0xc084fc, // magenta
    0x22d3ee, // cyan
    0xd4d4d8, // white
    0x52525b, // bright black
    0xfca5a5, // bright red
    0x86efac, // bright green
    0xfde047, // bright yellow
    0x93c5fd, // bright blue
    0xd8b4fe, // bright magenta
    0x67e8f9, // bright cyan
    0xfafafa, // bright white
];

/// The same slots on the light background: the dark table's hue families at
/// their 600/700 steps. "Bright" is darker here, not lighter, so it stays the
/// more prominent half; bright black steps lighter than black in both tables.
const ANSI_LIGHT: [u32; 16] = [
    0x1f1f1f, // black
    0xdc2626, // red — red-600
    0x16a34a, // green — green-600
    // Amber-700, not yellow-600: yellow-600 is 2.8:1 on white.
    0xb45309, // yellow — amber-700
    0x2563eb, // blue — blue-600
    0x9333ea, // magenta — purple-600
    0x0e7490, // cyan — cyan-700 (600 is too pale on white)
    0x3f3f46, // white — the body-text tone, zinc-700
    0x71717a, // bright black — zinc-500
    0xb91c1c, // bright red — red-700
    0x15803d, // bright green — green-700
    0x92400e, // bright yellow — amber-800
    0x1d4ed8, // bright blue — blue-700
    0x7e22ce, // bright magenta — purple-700
    0x155e75, // bright cyan — cyan-800
    0x18181b, // bright white — max emphasis, zinc-900
];

fn ansi(table: [u32; 16]) -> [gpui::Hsla; 16] {
    table.map(|c| rgb(c).into())
}

/// gpui's alias for whatever the platform calls its UI font, resolved per
/// backend by `font_name_with_fallbacks`.
const SYSTEM_SANS: &str = ".SystemUIFont";

/// The mono face has no alias of its own, so each backend is named here.
/// macOS wants the dot-name — "SF Mono" does not resolve. wasm has only what
/// gpui-web bundles, which `.ZedMono` names.
fn system_mono() -> &'static str {
    if cfg!(target_family = "wasm") {
        ".ZedMono"
    } else if cfg!(target_os = "macos") {
        ".AppleSystemUIFontMonospaced"
    } else if cfg!(target_os = "windows") {
        // Ships with Windows 11 and with Terminal. Nothing names a second
        // choice behind it — `font_name_with_fallbacks` resolves the dot-names
        // and nothing else — so a system without it gets whatever DirectWrite
        // substitutes.
        "Cascadia Mono"
    } else {
        "DejaVu Sans Mono"
    }
}
