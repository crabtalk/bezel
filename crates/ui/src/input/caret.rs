//! Shared caret geometry for text fields and document renderers.

use std::ops::Range;

use gpui::{
    App, BorderStyle, Bounds, Font, Global, Hsla, PaintQuad, Pixels, TextRun, Window,
    WrappedLineLayout, fill, outline, px,
};

/// The app-wide caret shape. Terminal cursors follow their terminal instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaretShape {
    #[default]
    Bar,
    Block,
    Underline,
}

impl Global for CaretShape {}

pub(crate) fn caret_shape(cx: &App) -> CaretShape {
    cx.try_global::<CaretShape>().copied().unwrap_or_default()
}

/// Changes the shape and repaints open windows.
pub(crate) fn set_caret_shape(shape: CaretShape, cx: &mut App) {
    cx.set_global(shape);
    cx.refresh_windows();
}

impl CaretShape {
    /// Adapts a renderer's existing bar, centred in a line `line_height` tall,
    /// to `width`, which a bar ignores. A block fills the line's height, solid
    /// or, when `hollow`, outlined.
    pub fn quad(
        self,
        mut bar: Bounds<Pixels>,
        line_height: Pixels,
        width: Pixels,
        color: Hsla,
        hollow: bool,
    ) -> PaintQuad {
        if self != Self::Bar {
            bar.size.width = width;
        }
        match self {
            Self::Bar => fill(bar, color),
            Self::Block => {
                bar.origin.y -= (line_height - bar.size.height) / 2.0;
                bar.size.height = line_height;
                match hollow {
                    true => outline(bar, color, BorderStyle::Solid),
                    false => fill(bar, color),
                }
            }
            Self::Underline => {
                let thickness = px(2.0).min(bar.size.height);
                bar.origin.y += bar.size.height - thickness;
                bar.size.height = thickness;
                fill(bar, color)
            }
        }
    }

    /// Whether the glyph under the caret is painted in the background colour,
    /// over the caret.
    pub fn cuts_out(self, hollow: bool) -> bool {
        self == Self::Block && !hollow
    }
}

/// The shaped width of `0` in `font`: a wide caret's width where no character
/// follows it.
pub fn zero_width(font: Font, font_size: Pixels, window: &Window) -> Pixels {
    let run = TextRun {
        len: 1,
        font,
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line("0".into(), font_size, &[run], None)
        .width
}

/// `runs` cut at `range`'s edges, the pieces inside it in `color`.
pub fn recoloured(runs: Vec<TextRun>, range: &Range<usize>, color: Hsla) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut at = 0;
    for run in runs {
        let end = at + run.len;
        for (start, stop, inside) in [
            (at, end.min(range.start), false),
            (at.max(range.start), end.min(range.end), true),
            (at.max(range.end), end, false),
        ] {
            if stop <= start {
                continue;
            }
            out.push(TextRun {
                len: stop - start,
                color: if inside { color } else { run.color },
                ..run.clone()
            });
        }
        at = end;
    }
    out
}

/// The next grapheme's shaped advance, or no character at a hard/soft line end.
pub fn character_advance(line: &WrappedLineLayout, text: &str, offset: usize) -> Option<Pixels> {
    if offset >= text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    if line
        .wrap_boundaries()
        .iter()
        .any(|boundary| line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index == offset)
    {
        return None;
    }
    let end = super::next_boundary(text, offset);
    Some((line.unwrapped_layout.x_for_index(end) - line.unwrapped_layout.x_for_index(offset)).abs())
}
