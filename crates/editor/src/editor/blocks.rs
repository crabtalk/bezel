//! Whole-block operations and undo.

use super::*;

impl Editor {
    /// Give a block over to the link it holds — a card, or the picture it
    /// points at.
    ///
    /// One step, not two: turning the block and giving the caret somewhere to
    /// go are one gesture, and undo has to agree.
    pub(super) fn turn_into(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>) {
        self.edit(EditKind::Structure, cx, |this| {
            // The URL is the block's whole text and the new block shows it
            // already, so [`Doc::set_kind`] is given nothing to carry across —
            // otherwise a picture prints its own URL as its caption.
            let held = this.doc.blocks[ix]
                .text_at(Part::Body)
                .map_or(0, |text| text.text.len());
            this.doc.edit_at(Cursor::new(ix, Part::Body, 0), |text| {
                *text = Text::default()
            });
            this.doc.set_kind(ix, kind);
            // A caret goes where the block admits one, and otherwise carries on
            // in the block after it — a fresh one when it ends the document.
            let part = this.doc.blocks[ix].parts().first().copied();
            let at = match part {
                Some(part) => Cursor::new(ix, part, 0),
                None => {
                    if this.doc.blocks.len() <= ix + 1 {
                        this.doc
                            .blocks
                            .push(markdown::Block::new(BlockKind::Paragraph(Text::default())));
                    }
                    Cursor::new(ix + 1, Part::Body, 0)
                }
            };
            this.selection = Selection::at(at.clamp(&this.doc));
            vec![Delta::Spliced(Splice {
                removed: Selection::new(
                    Cursor::new(ix, Part::Body, 0),
                    Cursor::new(ix, Part::Body, held),
                ),
                caret: Cursor::new(ix, Part::Body, 0),
                blocks: 0,
            })]
        });
    }

    pub(super) fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(step) = self
            .history
            .undo(self.mode, &self.doc, self.selection, &self.anchors)
        {
            self.restore(step, cx);
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(step) = self
            .history
            .redo(self.mode, &self.doc, self.selection, &self.anchors)
        {
            self.restore(step, cx);
        }
    }

    /// Put a whole moment back — document, caret and anchors together.
    ///
    /// The anchors come from the snapshot rather than from mapping, because a
    /// step back is not an edit: there is no delta between here and a document
    /// two hundred keystrokes ago.
    pub(super) fn restore(&mut self, step: crate::history::Step, cx: &mut Context<Self>) {
        self.doc = step.doc;
        self.selection = step.selection.clamp(&self.doc);
        self.caret_moved();
        self.anchors = step.anchors;
        if step.mode != self.mode {
            self.mode = step.mode;
            self.dismiss_menus();
            cx.emit(EditorEvent::ModeChanged(step.mode));
        }
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Move the caret's block, children and all, and follow it.
    ///
    /// Public because a gutter handle and a menu row reach the same operation
    /// as the key does — one vocabulary, not three paths into [`Doc`].
    pub fn move_block(&mut self, ix: usize, delta: isize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let caret = this.cursor();
            let at = this.doc.subtree(ix);
            let Some(to) = this.doc.move_block(ix, delta) else {
                return vec![];
            };
            // The caret rides along, keeping its depth within the subtree
            // that moved and its offset within its own text.
            let block = to + caret.block.saturating_sub(ix);
            this.selection = Selection::at(Cursor { block, ..caret }.clamp(&this.doc));
            vec![Delta::Moved { at, to: Some(to) }]
        });
    }

    pub fn duplicate_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let span = this.doc.subtree(ix);
            let Some(copy) = this.doc.duplicate(ix) else {
                return vec![];
            };
            this.selection = Selection::at(Cursor::new(copy, Part::Body, 0).clamp(&this.doc));
            vec![Delta::Opened {
                at: copy,
                count: span.len(),
            }]
        });
    }

    pub fn remove_block(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            let at = this.doc.subtree(ix);
            this.doc.remove_block(ix);
            this.selection =
                Selection::at(Cursor::new(ix.saturating_sub(1), Part::Body, 0).clamp(&this.doc));
            vec![Delta::Moved { at, to: None }]
        });
    }

    /// Tag a fenced block with the language it holds, or `None` for plain.
    pub fn set_language(&mut self, ix: usize, language: Option<String>, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_language(ix, language);
            vec![]
        });
    }

    /// Turn the caret's block into `kind` — what the slash menu and the block
    /// menu both do.
    pub fn set_block(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_kind(ix, kind);
            this.selection = this.selection.clamp(&this.doc);
            vec![]
        });
    }

    /// Replace a fenced block's code, as one undo step — what a painted fence
    /// writes through [`markdown::Fence::rewrite`].
    pub fn set_code(&mut self, ix: usize, code: String, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.set_code(ix, code);
            this.selection = this.selection.clamp(&this.doc);
            vec![]
        });
    }

    /// Turn the block at `ix` into `kind` and put the caret after it — what a
    /// [`crate::SlashAction::Run`] row does with a block the caret cannot sit
    /// in.
    /// The block is replaced as given, not converted: nothing of what it held
    /// carries over.
    pub fn place_block(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>) {
        if !self.blocks() || ix >= self.doc.blocks.len() {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.blocks[ix].kind = kind;
            this.selection = this.selection.clamp(&this.doc);
            vec![]
        });
        self.step_past(ix, cx);
    }

    /// Put the caret at the start of the block after `ix`, making an empty
    /// paragraph when `ix` ends the document.
    pub(super) fn step_past(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.doc.blocks.len() <= ix + 1 {
            self.edit(EditKind::Structure, cx, |this| {
                this.doc
                    .blocks
                    .push(markdown::Block::new(BlockKind::Paragraph(Text::default())));
                vec![]
            });
        }
        let at = Cursor::new(ix + 1, Part::Body, 0).clamp(&self.doc);
        self.select(Selection::at(at), cx);
    }

    /// Focus back from inside the painted fence at `ix`, the caret after it.
    pub(super) fn leave_block(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        self.step_past(ix, cx);
    }

    /// Point every picture at `from` to `to`: an image block's URL, or in
    /// source mode each `](from)` and `](<from>)` in the text. One undo step,
    /// and the caret keeps its place in the text around it. Nothing pointing
    /// at `from` is no edit.
    pub fn relink(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        if self.blocks() {
            let found: Vec<(usize, BlockKind)> = self
                .doc
                .blocks
                .iter()
                .enumerate()
                .filter_map(|(ix, block)| match &block.kind {
                    BlockKind::Image { url, alt, width } if url == from => Some((
                        ix,
                        BlockKind::Image {
                            url: to.to_owned(),
                            alt: alt.clone(),
                            width: *width,
                        },
                    )),
                    _ => None,
                })
                .collect();
            if found.is_empty() {
                return;
            }
            self.edit(EditKind::Structure, cx, |this| {
                for (ix, kind) in found {
                    this.doc.set_kind(ix, kind);
                }
                this.selection = this.selection.clamp(&this.doc);
                vec![]
            });
            return;
        }
        let source = self.source_text();
        let mut hits: Vec<(Range<usize>, String)> = [
            (format!("]({from})"), format!("]({to})")),
            (format!("](<{from}>)"), format!("](<{to}>)")),
        ]
        .into_iter()
        .flat_map(|(old, new)| {
            source
                .match_indices(&old)
                .map(|(at, _)| (at..at + old.len(), new.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
        if hits.is_empty() {
            return;
        }
        // Last first, so each splice leaves the offsets before it standing.
        hits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let moved = |cursor: Cursor| {
            let offset = hits
                .iter()
                .fold(cursor.offset as isize, |at, (range, new)| {
                    if cursor.offset >= range.end {
                        at + new.len() as isize - range.len() as isize
                    } else if cursor.offset > range.start {
                        at - (cursor.offset - range.start) as isize
                    } else {
                        at
                    }
                });
            Cursor {
                offset: offset.max(0) as usize,
                ..cursor
            }
        };
        let selection = Selection::new(moved(self.selection.anchor), moved(self.selection.head));
        self.edit(EditKind::Structure, cx, |this| {
            let at = |offset| Cursor::new(0, Part::Code, offset);
            let splices = hits
                .iter()
                .map(|(range, new)| {
                    Delta::Spliced(this.doc.replace(
                        Selection::new(at(range.start), at(range.end)),
                        Text::plain(new.as_str()),
                    ))
                })
                .collect();
            this.selection = selection.clamp(&this.doc);
            splices
        });
    }

    /// Check or uncheck the task block at `ix`. Does nothing to a block that
    /// is not one.
    ///
    /// An edit like any other: one undo step, and the caret stays where it
    /// was, so toggling a box across the document does not move whatever is
    /// being typed.
    pub fn toggle_task(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        let checked = match self.doc.blocks.get(ix).map(|block| &block.kind) {
            Some(BlockKind::Task { checked, .. }) => !checked,
            _ => return,
        };
        self.edit(EditKind::Structure, cx, |this| {
            if let Some(BlockKind::Task { checked: at, .. }) =
                this.doc.blocks.get_mut(ix).map(|block| &mut block.kind)
            {
                *at = checked;
            }
            vec![]
        });
        // Every other edit is made at the caret and owes it a reveal. This one
        // is made where a press landed, and the caret can be pages away: the
        // reveal `edit` asked for would scroll the box being checked off the
        // screen.
        self.reveal = false;
    }

    /// Put an empty row in table `ix` before `row` ([`Part::Cell`]
    /// numbering) and the caret in its first cell.
    pub fn add_row(&mut self, ix: usize, row: usize, cx: &mut Context<Self>) {
        if !self.blocks() || !matches!(self.kind_at(ix), Some(BlockKind::Table { .. })) {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.insert_row(ix, row);
            let row = row.max(1);
            this.selection = Selection::at(Cursor::new(ix, Part::Cell { row, column: 0 }, 0));
            vec![Delta::Cells {
                block: ix,
                row: Some(row),
                column: None,
            }]
        });
    }

    /// Put an empty column in table `ix` before `column` and the caret in it,
    /// on the caret's row when the caret is in this table.
    pub fn add_column(&mut self, ix: usize, column: usize, cx: &mut Context<Self>) {
        if !self.blocks() || !matches!(self.kind_at(ix), Some(BlockKind::Table { .. })) {
            return;
        }
        let row = match self.cursor() {
            Cursor {
                block,
                part: Part::Cell { row, .. },
                ..
            } if block == ix => row,
            _ => self.doc.blocks[ix]
                .parts()
                .first()
                .map_or(0, |part| match part {
                    Part::Cell { row, .. } => *row,
                    _ => 0,
                }),
        };
        self.edit(EditKind::Structure, cx, |this| {
            this.doc.insert_column(ix, column);
            this.selection = Selection::at(Cursor::new(ix, Part::Cell { row, column }, 0));
            vec![Delta::Cells {
                block: ix,
                row: None,
                column: Some(column),
            }]
        });
    }

    /// Tab and Shift-Tab in a table: the next or previous cell, with Tab in
    /// the last one adding a row. `false` when the caret is not in a cell.
    pub(super) fn step_cell(&mut self, forward: bool, cx: &mut Context<Self>) -> bool {
        let at = self.cursor();
        let Part::Cell { row, .. } = at.part else {
            return false;
        };
        let parts = self.doc.blocks[at.block].parts();
        let Some(here) = parts.iter().position(|part| *part == at.part) else {
            return false;
        };
        let next = match forward {
            true => parts.get(here + 1),
            false => here.checked_sub(1).and_then(|ix| parts.get(ix)),
        };
        match next {
            Some(part) => {
                let end = Cursor::new(at.block, *part, usize::MAX).clamp(&self.doc);
                self.head_to(end, false);
                cx.notify();
            }
            None if forward => self.add_row(at.block, row + 1, cx),
            None => {}
        }
        true
    }

    /// Enter in a table: the same column one row down, adding a row below the
    /// last. `false` when the caret is not in a cell.
    pub(super) fn cell_down(&mut self, cx: &mut Context<Self>) -> bool {
        let at = self.cursor();
        let Part::Cell { row, column } = at.part else {
            return false;
        };
        let below = Part::Cell {
            row: row + 1,
            column,
        };
        if self.doc.blocks[at.block].text_at(below).is_none() {
            self.add_row(at.block, row + 1, cx);
            if column > 0 {
                self.head_to(Cursor::new(at.block, below, 0).clamp(&self.doc), false);
            }
            return true;
        }
        let end = Cursor::new(at.block, below, usize::MAX).clamp(&self.doc);
        self.head_to(end, false);
        cx.notify();
        true
    }

    /// Take body row `row` out of table `ix`. Does nothing to the header row.
    pub fn remove_row(&mut self, ix: usize, row: usize, cx: &mut Context<Self>) {
        self.remove_line(ix, Line::Row(row), cx);
    }

    /// Take `column` out of table `ix`. Does nothing to the last column.
    pub fn remove_column(&mut self, ix: usize, column: usize, cx: &mut Context<Self>) {
        self.remove_line(ix, Line::Column(column), cx);
    }

    fn remove_line(&mut self, ix: usize, line: Line, cx: &mut Context<Self>) {
        if !self.blocks() {
            return;
        }
        let Some(BlockKind::Table { header, rows, .. }) = self.kind_at(ix) else {
            return;
        };
        let width = rows
            .iter()
            .map(Vec::len)
            .chain([header.len()])
            .max()
            .unwrap_or(0);
        let removable = match line {
            Line::Row(row) => (1..=rows.len()).contains(&row),
            Line::Column(column) => width > 1 && column < width,
        };
        if !removable {
            return;
        }
        self.edit(EditKind::Structure, cx, |this| {
            match line {
                Line::Row(row) => this.doc.remove_row(ix, row),
                Line::Column(column) => this.doc.remove_column(ix, column),
            };
            this.selection = this.selection.clamp(&this.doc);
            let (row, column) = match line {
                Line::Row(row) => (Some(row), None),
                Line::Column(column) => (None, Some(column)),
            };
            vec![Delta::CellsRemoved {
                block: ix,
                row,
                column,
            }]
        });
    }

    fn kind_at(&self, ix: usize) -> Option<&BlockKind> {
        self.doc.blocks.get(ix).map(|block| &block.kind)
    }
}
