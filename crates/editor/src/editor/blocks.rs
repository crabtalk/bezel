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
}
