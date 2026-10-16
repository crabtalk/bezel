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

/// What is painted under a text, in its offsets: washes (a highlight, a find
/// match, a comment) and a selection, each in the shape `S` its painter draws
/// it in. A painter draws [`Layers::painted`] and [`overdraws`] reads the same
/// order.
#[derive(Clone, Debug)]
pub struct Layers<S> {
    washes: Vec<(Range<usize>, Hsla, S)>,
    selection: Option<(Range<usize>, Hsla, S)>,
}

impl<S> Default for Layers<S> {
    fn default() -> Self {
        Self {
            washes: Vec::new(),
            selection: None,
        }
    }
}

impl<S> Layers<S> {
    pub fn new() -> Self {
        Self::default()
    }

    /// A wash over `range`, painted over the washes before it.
    pub fn wash(mut self, range: Range<usize>, color: Hsla, shape: S) -> Self {
        self.washes.push((range, color, shape));
        self
    }

    /// The selection, replacing any before it.
    pub fn select(mut self, range: Range<usize>, color: Hsla, shape: S) -> Self {
        self.selection = Some((range, color, shape));
        self
    }

    /// In the order they are painted: the selection, then the washes in the
    /// order given.
    pub fn painted(&self) -> impl Iterator<Item = (&Range<usize>, Hsla, &S)> {
        self.selection
            .iter()
            .chain(&self.washes)
            .map(|(range, color, shape)| (range, *color, shape))
    }

    /// What the text over `piece` was coloured for: the washes covering it
    /// over the opaque `page`.
    pub fn surface(&self, page: Hsla, piece: &Range<usize>) -> Hsla {
        theme::stack(
            page,
            self.washes
                .iter()
                .filter(|(range, ..)| covers(range, piece))
                .map(|(_, color, _)| *color),
        )
    }

    /// Everything [`Self::painted`] covering `piece`, over the opaque `page`,
    /// or `None` where nothing does.
    pub fn plate(&self, page: Hsla, piece: &Range<usize>) -> Option<Hsla> {
        let mut over = self
            .painted()
            .filter(|(range, ..)| covers(range, piece))
            .map(|(_, color, _)| color)
            .peekable();
        over.peek()?;
        Some(theme::stack(page, over))
    }

    /// These layers cut to `span` of the text, in its offsets.
    pub fn clipped(&self, span: &Range<usize>) -> Self
    where
        S: Clone,
    {
        let clip = |(range, color, shape): &(Range<usize>, Hsla, S)| {
            let (from, to) = (range.start.max(span.start), range.end.min(span.end));
            (from < to).then(|| (from - span.start..to - span.start, *color, shape.clone()))
        };
        Self {
            washes: self.washes.iter().filter_map(clip).collect(),
            selection: self.selection.as_ref().and_then(clip),
        }
    }

    fn edges(&self) -> impl Iterator<Item = usize> + '_ {
        self.washes
            .iter()
            .map(|(range, ..)| range)
            .chain(self.selection.as_ref().map(|(range, ..)| range))
            .flat_map(|range| [range.start, range.end])
    }
}

/// The pieces of `runs` to draw again over `layers` on the opaque `page`: each
/// on its [`Layers::plate`], its ink [`theme::ink_over`] that plate from its
/// [`Layers::surface`]. Text that reads there already, and text under nothing,
/// is left out.
pub fn overdraws<S>(runs: &[TextRun], page: Hsla, layers: &Layers<S>) -> Vec<Overdraw> {
    let mut out: Vec<Overdraw> = Vec::new();
    let mut at = 0;
    for run in runs {
        let end = at + run.len;
        let mut cuts: Vec<usize> = layers
            .edges()
            .filter(|cut| (at + 1..end).contains(cut))
            .chain([at, end])
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        for piece in cuts.windows(2) {
            let (from, to) = (piece[0], piece[1]);
            let Some(plate) = layers.plate(page, &(from..to)) else {
                continue;
            };
            let surface = layers.surface(page, &(from..to));
            let ink = theme::ink_over(run.color, page, surface, plate);
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

fn covers(range: &Range<usize>, piece: &Range<usize>) -> bool {
    range.start <= piece.start && piece.end <= range.end
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
