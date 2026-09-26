//! Hit-testing and caret geometry over the rows a frame painted.

use super::*;

/// Where each block's text landed, recorded as it painted.
///
/// A caret has to be placeable by pointer, and only paint knows where a glyph
/// ended up. An editor hands one of these in, the renderer fills it, and the
/// next click resolves against it. Read-only callers pass nothing and pay
/// nothing.
#[derive(Clone, Default)]
pub struct BlockLayouts(Rc<RefCell<Frames>>);

#[derive(Default)]
pub(super) struct Frames {
    texts: Vec<Painted>,
    rows: Vec<PaintedRow>,
    /// Each block's whole box, which a text layout does not give: a rule and
    /// an image hold no text at all, and a gutter handle still has to find them.
    blocks: Vec<(usize, Bounds<Pixels>)>,
    /// A fenced block's language label, which a host may want to hang a
    /// picker on.
    languages: Vec<(usize, Bounds<Pixels>)>,
    /// An image block's picture, which is not its block: the block runs the
    /// full column and carries the caption, and a resize handle belongs on the
    /// edge of the picture itself.
    pictures: Vec<(usize, Bounds<Pixels>)>,
    /// A task block's checkbox, which is not its marker column: the column is
    /// gutter either side of the box, and a click there places a caret.
    checkboxes: Vec<(usize, Bounds<Pixels>)>,
    /// Kept across frames: [`Self::clear`] leaves it.
    heights: Heights,
}

/// Block heights kept across frames, so a block off-screen is placed without
/// being built.
#[derive(Default)]
pub(super) struct Heights {
    /// A block's height by [`block_key`], as last measured at whatever width
    /// the column had then.
    by_key: HashMap<u64, Pixels>,
}

/// One shaped run and the slice of its part it covers.
///
/// A paragraph is one entry over all of its text; a code block is one entry per
/// line. The range is what lets both resolve a click the same way — the layout
/// answers in its own coordinates and the base puts the answer back into the
/// part's.
pub(super) struct Painted {
    block: usize,
    part: Part,
    range: Range<usize>,
    layout: TextLayout,
}

pub(super) struct PaintedRow {
    painted: usize,
    block: usize,
    part: Part,
    range: Range<usize>,
    bounds: Bounds<Pixels>,
    line_start: usize,
    wrapped_row: usize,
}

impl BlockLayouts {
    /// The position under `point`.
    ///
    /// Falls back to the nearest text vertically, so clicking the margin
    /// beside a line — or below the last one — still lands somewhere useful
    /// rather than doing nothing.
    pub fn hit(&self, point: Point<Pixels>) -> Option<Cursor> {
        let frames = self.0.borrow();
        if let Some(row) = frames.rows.iter().find(|row| row.bounds.contains(&point)) {
            return cursor_in_row(&frames, row, point.x);
        }
        frames
            .rows
            .iter()
            .min_by_key(|row| {
                let bounds = row.bounds;
                let above = (bounds.origin.y - point.y).abs();
                let below = (bounds.origin.y + bounds.size.height - point.y).abs();
                f32::from(above.min(below)) as i64
            })
            .and_then(|row| cursor_in_row(&frames, row, point.x))
    }

    /// Where a position painted last frame, and how tall its line is.
    ///
    /// Vertical motion is geometry rather than arithmetic on line numbers, so
    /// a wrapped row and a hard newline are the same case and neither needs
    /// counting — the rule `ui::TextField` arrived at.
    pub fn position(&self, at: Cursor) -> Option<(Point<Pixels>, Pixels)> {
        let frames = self.0.borrow();
        let painted = frames.texts.iter().find(|painted| {
            painted.block == at.block
                && painted.part == at.part
                && painted.range.start <= at.offset
                && at.offset <= painted.range.end
        })?;
        let point = painted
            .layout
            .position_for_index(at.offset - painted.range.start)?;
        Some((point, painted.layout.line_height()))
    }

    /// The painted rows of a range, in document order — what a bar centred over
    /// a selection is placed against, and what a highlight of a caller's own is
    /// drawn from.
    ///
    /// One rect per visual row rather than one box: a selection that wraps or
    /// crosses blocks has no single box, and a caller given one would paint
    /// over the gaps.
    pub fn rects(&self, selection: Selection) -> Vec<Bounds<Pixels>> {
        let (start, end) = selection.ordered();
        self.0
            .borrow()
            .texts
            .iter()
            .filter_map(|painted| {
                let here = Cursor::new(painted.block, painted.part, 0);
                let (from, to) = (
                    Cursor::new(start.block, start.part, 0),
                    Cursor::new(end.block, end.part, 0),
                );
                if here < from || here > to {
                    return None;
                }
                // The offsets only matter at the two ends: in between, the
                // whole of the text is covered.
                let len = painted.range.len();
                let first = if here == from { start.offset } else { 0 };
                let last = if here == to { end.offset } else { usize::MAX };
                let range = first.saturating_sub(painted.range.start).min(len)
                    ..last.saturating_sub(painted.range.start).min(len);
                (range.start < range.end).then(|| range_rects(&painted.layout, &range, 0.0, 0.0))
            })
            .flatten()
            .collect()
    }

    /// The position one painted row above or below `at`, and the row it landed
    /// on. Walks the recorded runs in paint order — which is document order.
    ///
    /// Two things make this refuse to be a hit test. The gap between blocks
    /// belongs to no run, so a probe there answers with whichever run is
    /// nearest — and at a boundary that is the block being *left*, whose bottom
    /// edge is zero pixels away while the next block's top is a whole gap. And
    /// `from` is passed in rather than derived from `at`, because an offset at
    /// a soft wrap belongs to two rows and `position_for_index` always answers
    /// with the first: derive it and every step down recomputes the same row.
    pub fn step_row(
        &self,
        at: Cursor,
        from: Point<Pixels>,
        down: bool,
    ) -> Option<(Cursor, Pixels)> {
        let frames = self.0.borrow();
        let ix = frames
            .rows
            .iter()
            .position(|row| {
                row.block == at.block
                    && row.part == at.part
                    && row_contains(row, at.offset)
                    && row.bounds.origin.y <= from.y
                    && from.y < row.bounds.origin.y + row.bounds.size.height
            })
            .or_else(|| {
                frames.rows.iter().position(|row| {
                    row.block == at.block && row.part == at.part && row_contains(row, at.offset)
                })
            })?;
        let next = match down {
            true => frames.rows.get(ix + 1)?,
            false => frames.rows.get(ix.checked_sub(1)?)?,
        };
        Some((cursor_in_row(&frames, next, from.x)?, next.bounds.origin.y))
    }

    /// Whether `point` is inside painted text.
    ///
    /// [`Self::hit`] answers with the nearest run wherever it is asked, which
    /// is what a click wants and what a *pointer* must not have: an I-beam
    /// belongs where a caret would land, not everywhere an editor's box
    /// reaches.
    pub fn over_text(&self, point: Point<Pixels>) -> bool {
        self.0
            .borrow()
            .texts
            .iter()
            .any(|painted| painted.layout.bounds().contains(&point))
    }

    /// The block under `point`, for a gutter handle and a drop target.
    pub fn block_at(&self, point: Point<Pixels>) -> Option<usize> {
        let blocks = &self.0.borrow().blocks;
        blocks
            .iter()
            .find(|(_, bounds)| bounds.contains(&point))
            .or_else(|| {
                blocks.iter().min_by_key(|(_, bounds)| {
                    let above = (bounds.origin.y - point.y).abs();
                    let below = (bounds.origin.y + bounds.size.height - point.y).abs();
                    f32::from(above.min(below)) as i64
                })
            })
            .map(|(ix, _)| *ix)
    }

    /// Where a block's first painted row sits, and how tall that row is — what
    /// a mark in the gutter has to line up with.
    ///
    /// [`Self::block_bounds`] is not that: it spans every row the block holds,
    /// and a heading's single row is taller than a paragraph's, so anything
    /// placed from the top of the box rides above the text it points at.
    /// `None` for a block that paints no text at all, a rule being the one
    /// that does.
    pub fn first_row(&self, ix: usize) -> Option<(Pixels, Pixels)> {
        let texts = &self.0.borrow().texts;
        let painted = texts.iter().find(|painted| painted.block == ix)?;
        Some((
            painted.layout.bounds().origin.y,
            painted.layout.line_height(),
        ))
    }

    /// Where a block painted last frame, in window coordinates.
    pub fn block_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .blocks
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    /// Where a fenced block's language label painted — the box a host hangs its
    /// picker on. Recorded rather than derived: the word's width is the text
    /// system's answer, and padding arithmetic would be wrong the first time
    /// any of it changed.
    pub fn language_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .languages
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    /// Where an image block's picture painted, which
    /// [`BlockLayouts::block_bounds`] does not give: that box spans the column
    /// and takes in the caption, so a handle placed from it sits off the edge
    /// of any picture narrower than the page.
    pub fn picture_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .pictures
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    /// Where a task block's checkbox painted, which is the box itself and not
    /// the marker column it sits in — a press outside it is a press on the
    /// gutter, and belongs to whatever handles one.
    pub fn checkbox_bounds(&self, ix: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .checkboxes
            .iter()
            .find(|(block, _)| *block == ix)
            .map(|(_, bounds)| *bounds)
    }

    pub(super) fn record(&self, block: usize, part: Part, range: Range<usize>, layout: TextLayout) {
        let mut frames = self.0.borrow_mut();
        let painted = frames.texts.len();
        record_rows(&mut frames.rows, painted, block, part, &range, &layout);
        frames.texts.push(Painted {
            block,
            part,
            range,
            layout,
        });
    }

    pub(super) fn record_block(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().blocks.push((ix, bounds));
    }

    pub(super) fn record_language(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().languages.push((ix, bounds));
    }

    pub(super) fn record_picture(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().pictures.push((ix, bounds));
    }

    pub(super) fn record_checkbox(&self, ix: usize, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().checkboxes.push((ix, bounds));
    }

    pub(super) fn height(&self, key: u64) -> Option<Pixels> {
        self.0.borrow().heights.by_key.get(&key).copied()
    }

    pub(super) fn record_height(&self, key: u64, height: Pixels) {
        self.0.borrow_mut().heights.by_key.insert(key, height);
    }

    /// Drops the heights of blocks no longer in the document once they
    /// outnumber the ones that are.
    pub(super) fn prune(&self, keys: &[u64]) {
        let by_key = &mut self.0.borrow_mut().heights.by_key;
        if by_key.len() > 2 * keys.len() {
            let keep: std::collections::HashSet<u64> = keys.iter().copied().collect();
            by_key.retain(|key, _| keep.contains(key));
        }
    }

    pub(super) fn clear(&self) {
        let mut frames = self.0.borrow_mut();
        frames.texts.clear();
        frames.rows.clear();
        frames.blocks.clear();
        frames.languages.clear();
        frames.pictures.clear();
        frames.checkboxes.clear();
    }
}

pub(super) fn row_contains(row: &PaintedRow, offset: usize) -> bool {
    row.range.start <= offset && offset <= row.range.end
}

pub(super) fn cursor_in_row(frames: &Frames, row: &PaintedRow, x: Pixels) -> Option<Cursor> {
    let painted = &frames.texts[row.painted];
    let line = painted
        .layout
        .line_layout_for_index(row.line_start - painted.range.start)?;
    let height = row.bounds.size.height;
    let local = point(
        x - row.bounds.origin.x,
        height * (row.wrapped_row as f32 + 0.5),
    );
    let (Ok(offset) | Err(offset)) = line.closest_index_for_position(local, height);
    Some(Cursor::new(
        row.block,
        row.part,
        (row.line_start + offset).min(row.range.end),
    ))
}

pub(super) fn record_rows(
    rows: &mut Vec<PaintedRow>,
    painted: usize,
    block: usize,
    part: Part,
    range: &Range<usize>,
    layout: &TextLayout,
) {
    let line_height = layout.line_height();
    let bounds = layout.bounds();
    let mut origin = bounds.origin;
    let mut line_start = range.start;
    for line in layout.line_layouts() {
        let shaped = &line.unwrapped_layout;
        let row_ends = line
            .wrap_boundaries()
            .iter()
            .map(|wrap| shaped.runs[wrap.run_ix].glyphs[wrap.glyph_ix].index)
            .chain([line.len()]);
        let mut row_start = 0;
        for (row, row_end) in row_ends.enumerate() {
            rows.push(PaintedRow {
                painted,
                block,
                part,
                range: line_start + row_start..line_start + row_end,
                bounds: Bounds::new(
                    origin + point(px(0.0), line_height * row as f32),
                    size(bounds.size.width, line_height),
                ),
                line_start,
                wrapped_row: row,
            });
            row_start = row_end;
        }
        origin.y += line.size(line_height).height;
        line_start += line.len() + 1;
    }
}
