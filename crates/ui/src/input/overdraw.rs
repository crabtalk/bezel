//! Text drawn again over the layers painted under it — a selection, a
//! highlight, a find match — where its own colour does not read on them.
//!
//! gpui shapes each colour of a line apart, so a piece is recoloured by
//! drawing its glyphs again over the line rather than by cutting its runs.

use std::ops::Range;

use gpui::{
    BorderStyle, Bounds, Hsla, Pixels, Point, TextRun, Window, WrappedLineLayout, point, px, quad,
    size,
};

use super::caret;

/// A piece of text that the layers under it leave below its contrast.
#[derive(Clone, Debug, PartialEq)]
pub struct Overdraw {
    pub range: Range<usize>,
    /// The layers under the piece, flattened over the page.
    pub plate: Hsla,
    /// The colour its glyphs are drawn again in.
    pub ink: Hsla,
}

/// The pieces of `runs` to draw again over `layers`, which are painted in the
/// order given over the opaque `page`. A piece sits under one set of layers,
/// its ink [`theme::legible`] on their plate; text that reads there already,
/// and text under no layer, is left out.
pub fn overdraws(runs: &[TextRun], page: Hsla, layers: &[(Range<usize>, Hsla)]) -> Vec<Overdraw> {
    let mut out: Vec<Overdraw> = Vec::new();
    let mut at = 0;
    for run in runs {
        let end = at + run.len;
        let mut cuts: Vec<usize> = layers
            .iter()
            .flat_map(|(range, _)| [range.start, range.end])
            .filter(|cut| (at + 1..end).contains(cut))
            .chain([at, end])
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        for piece in cuts.windows(2) {
            let (from, to) = (piece[0], piece[1]);
            let mut over = layers
                .iter()
                .filter(|(range, _)| range.start <= from && to <= range.end)
                .map(|(_, color)| *color)
                .peekable();
            if over.peek().is_none() {
                continue;
            }
            let plate = theme::stack(page, over);
            let ink = theme::legible(run.color, plate, theme::kept(run.color, page));
            if ink == run.color {
                continue;
            }
            match out.last_mut() {
                Some(last) if last.range.end == from && last.plate == plate && last.ink == ink => {
                    last.range.end = to;
                }
                _ => out.push(Overdraw {
                    range: from..to,
                    plate,
                    ink,
                }),
            }
        }
        at = end;
    }
    out
}

/// Draws `pieces` again over what is already painted: each row of a piece
/// covered in its plate, corners rounded by `radius`, then its glyphs in its
/// ink. `lines` are a text's lines laid out one under the other from `origin`,
/// and `pieces` are in the text's offsets.
pub fn paint<'a>(
    lines: impl IntoIterator<Item = &'a WrappedLineLayout>,
    origin: Point<Pixels>,
    line_height: Pixels,
    radius: Pixels,
    pieces: &[Overdraw],
    window: &mut Window,
) {
    if pieces.is_empty() {
        return;
    }
    let (mut start, mut top) = (0, origin);
    for line in lines {
        let end = start + line.len();
        for piece in pieces {
            let (from, to) = (piece.range.start.max(start), piece.range.end.min(end));
            if from < to {
                paint_line(
                    line,
                    top,
                    line_height,
                    radius,
                    from - start..to - start,
                    piece,
                    window,
                );
            }
        }
        start = end + 1;
        top.y += line.size(line_height).height;
    }
}

/// `range` of one line of [`paint`], `origin` its top left.
fn paint_line(
    line: &WrappedLineLayout,
    origin: Point<Pixels>,
    line_height: Pixels,
    radius: Pixels,
    range: Range<usize>,
    piece: &Overdraw,
    window: &mut Window,
) {
    let layout = &line.unwrapped_layout;
    let starts: Vec<usize> = std::iter::once(0)
        .chain(
            line.wrap_boundaries()
                .iter()
                .map(|wrap| layout.runs[wrap.run_ix].glyphs[wrap.glyph_ix].index),
        )
        .collect();
    let ends = starts.iter().skip(1).copied().chain([line.len()]);
    for (row, (start, end)) in starts.iter().copied().zip(ends).enumerate() {
        let (from, to) = (range.start.max(start), range.end.min(end));
        if from >= to {
            continue;
        }
        let (left, right) = (layout.x_for_index(from), layout.x_for_index(to));
        let at = origin + point(left - layout.x_for_index(start), line_height * row as f32);
        window.paint_quad(quad(
            Bounds::new(at, size(right - left, line_height)),
            radius,
            piece.plate,
            px(0.),
            gpui::transparent_black(),
            BorderStyle::default(),
        ));
        caret::paint_covered(layout, &(from..to), at, line_height, piece.ink, window);
    }
}
