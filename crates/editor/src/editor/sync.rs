//! Keeping this editor and the other editors on its document in step.

use super::*;

impl Editor {
    /// Hand this editor's snapshot back to the document, telling every other
    /// editor on it what `change` did.
    pub(super) fn publish(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.blocks() && !matches!(change, Change::Composing) {
            self.synced.borrow_mut().set_doc(self.doc.shared());
        }
        let (doc, mode, anchors, by) = (
            self.doc.clone(),
            self.mode,
            self.anchors.clone(),
            Some(cx.entity_id()),
        );
        self.document.update(cx, |document, cx| {
            document.doc = doc;
            document.mode = mode;
            document.anchors = anchors;
            cx.emit(DocumentEvent { by, change });
        });
    }

    /// Take the document's snapshot after another editor changed it, carrying
    /// the caret through `change`.
    pub(super) fn take_document(
        &mut self,
        document: &Entity<Document>,
        change: &Change,
        cx: &mut Context<Self>,
    ) {
        let was = self.mode;
        {
            let document = document.read(cx);
            self.doc = document.doc.clone();
            self.mode = document.mode;
            self.anchors = document.anchors.clone();
        }
        let carry = |at: Cursor| match change {
            Change::Edited(deltas) => deltas
                .iter()
                .try_fold(at, |at, delta| delta.cursor(at))
                .unwrap_or_default(),
            Change::Replaced | Change::Composing | Change::Reloaded | Change::Anchors => at,
        };
        if !matches!(change, Change::Anchors) {
            let selection = Selection {
                anchor: carry(self.selection.anchor),
                head: carry(self.selection.head),
                affinity: self.selection.affinity,
            };
            self.selection = selection.clamp(&self.doc);
            self.marked = None;
            self.stored.clear();
            self.goal = None;
            self.run.interrupt();
            self.dismiss_menus();
        }
        if !self.blocks() {
            self.drop_source_view();
        }
        if self.mode != was {
            cx.emit(EditorEvent::ModeChanged(self.mode));
        }
        cx.notify();
    }
}
