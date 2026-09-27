//! Selection, and the lines and cursor a host paints.

use super::*;

impl Emulator {
    //
    // `Term` owns the selection outright, which is what makes this cheap: it
    // rotates the anchors when output scrolls the grid and drops them on clear
    // and resize, so a selection tracks live output without any bookkeeping
    // here. The host supplies pointer positions; everything below is a thin
    // translation into grid coordinates.

    /// The grid point under a viewport cell (row 0 = top of the visible area).
    ///
    /// Viewport rows are what the pointer hits; grid lines are what a selection
    /// anchors to, and the two differ by the scrollback offset. Anchoring in
    /// grid space is what lets a selection stay on its text while the view
    /// scrolls out from under it.
    pub fn grid_point(&self, viewport_row: usize, col: usize) -> Point {
        Point::new(
            Line(viewport_row as i32 - self.display_offset() as i32),
            Column(col.min(self.cols().saturating_sub(1))),
        )
    }

    /// Begin a selection. `ty` picks the granularity: [`SelectionType::Simple`]
    /// for a drag, `Semantic` for a double-click word, `Lines` for a triple-
    /// click row.
    pub fn start_selection(&mut self, ty: SelectionType, point: Point, side: Side) {
        self.term.selection = Some(Selection::new(ty, point, side));
    }

    /// Extend the in-progress selection to `point`. No-op without one.
    pub fn update_selection(&mut self, point: Point, side: Side) {
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// The selected text, or `None` when there is no selection or it covers
    /// nothing (a click without a drag leaves an empty one behind).
    pub fn selection_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            // `selection_to_string` folds a cell's zerowidth chars into the
            // text, anchors included. Copying across an image would otherwise
            // paste a private-use character nobody can see.
            .map(|text| text.replace(|ch: char| is_anchor(ch), ""))
            .filter(|s| !s.is_empty())
    }

    /// Whether a non-empty selection is active — drives the copy action and
    /// the "clear it" branch on the next click.
    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    pub(super) fn selection_range(&self) -> Option<SelectionRange> {
        self.term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term))
    }

    /// Snapshot one viewport row (0 = top) honoring the scrollback offset.
    pub fn line(&self, viewport_row: usize) -> Vec<CellSnapshot> {
        let selection = self.selection_range();
        let Some(held) = &self.held else {
            return self.line_inner(viewport_row, selection);
        };
        held.lines
            .get(viewport_row)
            .map(|cells| self.reselect(held, viewport_row, cells, selection))
            .unwrap_or_default()
    }

    /// A held row with `selected` recomputed against the live selection. A
    /// hold stops output, not the pointer.
    pub(super) fn reselect(
        &self,
        held: &Held,
        viewport_row: usize,
        cells: &[CellSnapshot],
        selection: Option<SelectionRange>,
    ) -> Vec<CellSnapshot> {
        let line = Line(viewport_row as i32 - held.offset as i32);
        cells
            .iter()
            .enumerate()
            .map(|(col, cell)| CellSnapshot {
                selected: selection
                    .is_some_and(|range| range.contains(Point::new(line, Column(col)))),
                ..*cell
            })
            .collect()
    }

    /// The shared body of [`Self::line`], taking the selection range as an
    /// argument so [`Self::lines`] resolves it once per frame rather than once
    /// per row — `to_range` re-walks the grid for semantic and line selections.
    pub(super) fn line_inner(
        &self,
        viewport_row: usize,
        selection: Option<SelectionRange>,
    ) -> Vec<CellSnapshot> {
        let offset = self.display_offset() as i32;
        let line = Line(viewport_row as i32 - offset);
        let grid = self.term.grid();
        let row = &grid[line];
        (0..self.cols())
            .map(|col| {
                let cell = &row[Column(col)];
                CellSnapshot {
                    // Stands for an image; the glyph itself is never drawn.
                    ch: match cell.c {
                        crate::placeholder::PLACEHOLDER => ' ',
                        ch => ch,
                    },
                    fg: map_color(cell.fg),
                    bg: map_color(cell.bg),
                    bold: cell.flags.intersects(Flags::BOLD),
                    dim: cell.flags.intersects(Flags::DIM),
                    italic: cell.flags.intersects(Flags::ITALIC),
                    underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
                    inverse: cell.flags.intersects(Flags::INVERSE),
                    hidden: cell.flags.intersects(Flags::HIDDEN),
                    wide: cell.flags.intersects(Flags::WIDE_CHAR),
                    wide_spacer: cell
                        .flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER),
                    selected: selection
                        .is_some_and(|range| range.contains(Point::new(line, Column(col)))),
                }
            })
            .collect()
    }

    /// All viewport rows, top to bottom.
    pub fn lines(&self) -> Vec<Vec<CellSnapshot>> {
        let selection = self.selection_range();
        let Some(held) = &self.held else {
            return self.live_lines();
        };
        held.lines
            .iter()
            .enumerate()
            .map(|(row, cells)| self.reselect(held, row, cells, selection))
            .collect()
    }

    pub(super) fn live_lines(&self) -> Vec<Vec<CellSnapshot>> {
        let selection = self.selection_range();
        (0..self.rows())
            .map(|r| self.line_inner(r, selection))
            .collect()
    }

    /// Cursor in viewport coordinates; `None` when hidden or scrolled out.
    pub fn cursor(&self) -> Option<CursorSnapshot> {
        match &self.held {
            Some(held) => held.cursor,
            None => self.live_cursor(),
        }
    }

    pub(super) fn live_cursor(&self) -> Option<CursorSnapshot> {
        let content = self.term.renderable_content();
        if content.cursor.shape == CursorShape::Hidden {
            return None;
        }
        let Point { line, column } = content.cursor.point;
        let row = line.0 + self.display_offset() as i32;
        if row < 0 || row >= self.rows() as i32 {
            return None;
        }
        Some(CursorSnapshot {
            row: row as usize,
            col: column.0,
        })
    }

    /// Test/diagnostic helper: a viewport row as trimmed text (wide-char
    /// spacers skipped).
    pub fn row_text(&self, viewport_row: usize) -> String {
        let mut text: String = self
            .line(viewport_row)
            .iter()
            .filter(|c| !c.wide_spacer)
            .map(|c| c.ch)
            .collect();
        while text.ends_with(' ') {
            text.pop();
        }
        text
    }
}
