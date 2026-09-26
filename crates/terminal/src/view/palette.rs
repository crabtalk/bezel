//! The ANSI palette on the terminal background, resolved per appearance.

use super::*;

/// Terminal background for an appearance.
///
/// Dark is `#090909`, one small step *up* from the app's `#060606` content
/// plane — the terminal reads as its own pane without becoming a lighter box.
/// Light mirrors that relationship rather than inverting the literal value:
/// the content plane is already pure white, so the terminal steps one notch
/// *down* to `#fafafa`. The step is bigger than dark's 3/255 for the reason the
/// theme's light surfaces are (`Theme::light`): a near-white delta that reads
/// as separation on near-black disappears entirely on white.
pub fn terminal_bg_for(appearance: Appearance) -> Hsla {
    match appearance {
        Appearance::Dark => rgb8(0x09, 0x09, 0x09),
        Appearance::Light => rgb8(0xfa, 0xfa, 0xfa),
    }
}

/// Terminal background in the appearance currently installed — the
/// context-free form, same shape as [`theme::ink`].
pub fn terminal_bg() -> Hsla {
    terminal_bg_for(theme::current_appearance())
}

/// The panel fill behind the grid. On glass the opaque tone thins to a
/// translucent wash so what sits behind the panel reads through like the rest
/// of the chrome (same move as [`Theme::card_glass_bg`]); opaque chrome keeps
/// the true tone. Explicit cell backgrounds (vim colorschemes etc.) still paint
/// their own opaque quads on top.
pub fn terminal_panel_bg(theme: &Theme) -> Hsla {
    if theme.glass {
        terminal_bg_for(theme.appearance).opacity(0.4)
    } else {
        terminal_bg_for(theme.appearance)
    }
}

/// Selection wash over the grid.
///
/// Deliberately *achromatic*, which is where this differs from
/// [`Theme::selection`] — the accent selection token. A saturated wash sits on
/// top of sixteen ANSI hues and drags every one of them toward itself: red text
/// under blue reads purple, green reads teal. A neutral veil changes lightness
/// only, so selected output keeps the colors the program asked for — the whole
/// point of tuning those palettes per appearance in the first place.
///
/// White on dark, black on light, the same direction [`theme::ink`] takes. The
/// alpha is heavier than a hover wash because this has to read as a deliberate
/// highlight at a glance, and lighter than a plate because the glyphs
/// underneath still have to be legible through it.
pub fn terminal_selection_for(appearance: Appearance) -> Hsla {
    match appearance {
        Appearance::Dark => gpui::hsla(0.0, 0.0, 1.0, 0.22),
        // Slightly heavier: an equal alpha of black on near-white reads fainter
        // than white on near-black, because the surround is brighter to begin
        // with.
        Appearance::Light => gpui::hsla(0.0, 0.0, 0.0, 0.16),
    }
}

/// The 16 ANSI colors tuned for the near-black background (indexes 0-7 normal,
/// 8-15 bright).
pub(super) const ANSI16_DARK: [(u8, u8, u8); 16] = [
    (0x24, 0x24, 0x24), // black — visible against #090909
    (0xf8, 0x71, 0x71), // red
    (0x4a, 0xde, 0x80), // green
    (0xfa, 0xcc, 0x15), // yellow
    (0x60, 0xa5, 0xfa), // blue
    (0xc0, 0x84, 0xfc), // magenta
    (0x22, 0xd3, 0xee), // cyan
    (0xd4, 0xd4, 0xd8), // white
    (0x52, 0x52, 0x5b), // bright black
    (0xfc, 0xa5, 0xa5), // bright red
    (0x86, 0xef, 0xac), // bright green
    (0xfd, 0xe0, 0x47), // bright yellow
    (0x93, 0xc5, 0xfd), // bright blue
    (0xd8, 0xb4, 0xfe), // bright magenta
    (0x67, 0xe8, 0xf9), // bright cyan
    (0xfa, 0xfa, 0xfa), // bright white
];

/// The same 16 slots for the light background — same hue families as
/// [`ANSI16_DARK`], moved down the tonal scale (the 400/300 steps the dark
/// table uses fail on white; these are the 600/700 steps, the same swap
/// `Theme::light` makes for its accents).
///
/// "Bright" stays *more prominent*, which on a light field means **darker**,
/// not lighter — a literal translation of the dark table would make the bright
/// half the invisible half, which is the bug this fixes in its purest form.
pub(super) const ANSI16_LIGHT: [(u8, u8, u8); 16] = [
    (0x1f, 0x1f, 0x1f), // black
    (0xdc, 0x26, 0x26), // red — red-600
    (0x16, 0xa3, 0x4a), // green — green-600
    // Amber-700, not yellow-600: yellow is the one hue whose 600 step is still
    // bright enough to fail AA on white (2.8:1). `Theme::light` drops its
    // `warning` token to the same step for the same reason.
    (0xb4, 0x53, 0x09), // yellow — amber-700
    (0x25, 0x63, 0xeb), // blue — blue-600
    (0x93, 0x33, 0xea), // magenta — purple-600
    (0x0e, 0x74, 0x90), // cyan — cyan-700 (600 is too pale on white)
    (0x3f, 0x3f, 0x46), // white — the body-text tone, zinc-700
    (0x71, 0x71, 0x7a), // bright black — zinc-500
    (0xb9, 0x1c, 0x1c), // bright red — red-700
    (0x15, 0x80, 0x3d), // bright green — green-700
    (0x92, 0x40, 0x0e), // bright yellow — amber-800
    (0x1d, 0x4e, 0xd8), // bright blue — blue-700
    (0x7e, 0x22, 0xce), // bright magenta — purple-700
    (0x15, 0x5e, 0x75), // bright cyan — cyan-800
    (0x18, 0x18, 0x1b), // bright white — max emphasis, zinc-900
];

pub(super) fn rgb8(r: u8, g: u8, b: u8) -> Hsla {
    let (h, s, l) = theme::rgb_to_hsl(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    gpui::hsla(h, s, l, 1.0)
}

/// xterm 256-color cube component levels.
pub(super) const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Resolve an indexed color (0-255) to RGB components for an appearance.
///
/// Three ranges, treated differently on purpose:
///
/// - **0-15** are *named* slots ("red", "bright blue"), not literal values —
///   every terminal emulator re-tints them per theme, so they swap tables.
/// - **16-231** is the 6×6×6 cube: a program asking for index 196 is asking for
///   `#ff0000` by arithmetic, and remapping it would be inventing colors the
///   caller did not pick. Left alone in both appearances, same as iTerm/Apple
///   Terminal light themes.
/// - **232-255** is the grayscale ramp, which tools use for *de-emphasis*
///   rather than for a specific grey. Its dark→light direction only reads as
///   "dim" on a dark background, so light mode mirrors the ramp: index 232
///   stays the faintest and 255 the strongest in both. Without this the ramp is
///   the single biggest legibility hole on white, because its bright end —
///   where most "dim hint" text lands — is the end that vanishes.
pub fn indexed_rgb(appearance: Appearance, index: u8) -> (u8, u8, u8) {
    match index {
        0..=15 => match appearance {
            Appearance::Dark => ANSI16_DARK[index as usize],
            Appearance::Light => ANSI16_LIGHT[index as usize],
        },
        16..=231 => {
            let n = index as usize - 16;
            (
                CUBE_LEVELS[n / 36],
                CUBE_LEVELS[(n / 6) % 6],
                CUBE_LEVELS[n % 6],
            )
        }
        232..=255 => {
            let step = index - 232;
            let step = match appearance {
                Appearance::Dark => step,
                Appearance::Light => 23 - step,
            };
            let v = 8 + 10 * step;
            (v, v, v)
        }
    }
}

/// Resolve a cell color to paint against the theme.
pub fn resolve_color(color: CellColor, theme: &Theme) -> Hsla {
    match color {
        CellColor::Foreground => theme.text,
        CellColor::Background => terminal_bg_for(theme.appearance),
        CellColor::Indexed(ix) => {
            let (r, g, b) = indexed_rgb(theme.appearance, ix);
            rgb8(r, g, b)
        }
        CellColor::Rgb(r, g, b) => rgb8(r, g, b),
    }
}
