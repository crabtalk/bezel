//! Terminal colours resolved against the theme.

use super::*;

/// The panel fill behind the grid. On glass the opaque tone thins to a
/// translucent wash so what sits behind the panel reads through like the rest
/// of the chrome (same move as [`Theme::card_glass_bg`]); opaque chrome keeps
/// the true tone. Explicit cell backgrounds (vim colorschemes etc.) still paint
/// their own opaque quads on top.
pub fn terminal_panel_bg(theme: &Theme) -> Hsla {
    if theme.glass {
        theme.terminal_bg.opacity(0.4)
    } else {
        theme.terminal_bg
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

pub(super) fn rgb8(r: u8, g: u8, b: u8) -> Hsla {
    let (h, s, l) = theme::rgb_to_hsl(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    gpui::hsla(h, s, l, 1.0)
}

/// xterm 256-color cube component levels.
pub(super) const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Resolve an indexed color (16-255) to RGB components for an appearance;
/// `None` for 0-15, the named slots in [`Theme::terminal_ansi`].
///
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
pub fn extended_rgb(appearance: Appearance, index: u8) -> Option<(u8, u8, u8)> {
    match index {
        0..=15 => None,
        16..=231 => {
            let n = index as usize - 16;
            Some((
                CUBE_LEVELS[n / 36],
                CUBE_LEVELS[(n / 6) % 6],
                CUBE_LEVELS[n % 6],
            ))
        }
        232..=255 => {
            let step = index - 232;
            let step = match appearance {
                Appearance::Dark => step,
                Appearance::Light => 23 - step,
            };
            let v = 8 + 10 * step;
            Some((v, v, v))
        }
    }
}

/// Resolve an indexed color (0-255) against the theme.
pub fn indexed_color(theme: &Theme, index: u8) -> Hsla {
    match extended_rgb(theme.appearance, index) {
        Some((r, g, b)) => rgb8(r, g, b),
        None => theme.terminal_ansi[index as usize],
    }
}

/// Resolve a cell color to paint against the theme.
pub fn resolve_color(color: CellColor, theme: &Theme) -> Hsla {
    match color {
        CellColor::Foreground => theme.text,
        CellColor::Background => theme.terminal_bg,
        CellColor::Indexed(ix) => indexed_color(theme, ix),
        CellColor::Rgb(r, g, b) => rgb8(r, g, b),
    }
}
