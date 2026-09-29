//! Shared caret geometry for text fields and document renderers.

use gpui::{App, Bounds, Global, Hsla, PaintQuad, Pixels, WrappedLineLayout, fill, px};

/// The app-wide caret shape. Terminal cursors follow their terminal instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaretShape {
    #[default]
    Bar,
    Block,
    Underline,
}

impl Global for CaretShape {}

pub fn caret_shape(cx: &App) -> CaretShape {
    cx.try_global::<CaretShape>().copied().unwrap_or_default()
}

/// Changes the shape and repaints open windows.
pub fn set_caret_shape(shape: CaretShape, cx: &mut App) {
    cx.set_global(shape);
    cx.refresh_windows();
}

impl CaretShape {
    /// Adapts a renderer's existing bar to the character advance. Missing
    /// characters use half an em; a block is translucent to keep text readable.
    pub fn quad(self, mut bar: Bounds<Pixels>, advance: Option<Pixels>, color: Hsla) -> PaintQuad {
        if self != Self::Bar {
            bar.size.width = advance
                .filter(|width| *width > px(0.0))
                .unwrap_or(bar.size.height / 2.0);
        }
        if self == Self::Underline {
            let thickness = px(2.0).min(bar.size.height);
            bar.origin.y += bar.size.height - thickness;
            bar.size.height = thickness;
        }
        fill(
            bar,
            if self == Self::Block {
                color.opacity(0.35)
            } else {
                color
            },
        )
    }
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
