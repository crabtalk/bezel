//! Source mode and the switch between it and blocks.

use super::*;

impl Editor {
    /// The document as markdown — normalized, because that is the form that
    /// survives being read back. In [`Mode::Source`] it is the text being
    /// edited, exactly as it stands.
    pub fn source(&self) -> String {
        match self.mode {
            Mode::Blocks => {
                let mut doc = self.doc.clone();
                doc.normalize_with(&self.marks);
                markdown::serialize_with(&doc, &self.marks)
            }
            Mode::Source => self.source_text().to_string(),
        }
    }

    /// Which form the document is being edited in.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// What a toolbar needs to light itself. See [`Formatting`].
    pub fn formatting(&self) -> Formatting {
        let at = self.cursor();
        let marks = if self.blocks() {
            let mut marks = self.doc.marks(self.selection);
            // A stored mark is one cmd-B has already taken and nothing has
            // spent yet, so the button that took it stays lit.
            for mark in &self.stored {
                if !marks.contains(mark) {
                    marks.push(mark.clone());
                }
            }
            marks
        } else {
            Vec::new()
        };
        Formatting {
            mode: self.mode,
            marks,
            block: self
                .doc
                .blocks
                .get(at.block)
                .and_then(|block| crate::slash::label(&block.kind)),
            fenceable: self.blocks() && fenceable(&self.doc, self.selection),
        }
    }

    /// Switch between the document and its markdown, carrying the caret across.
    ///
    /// One undo step, and one the history knows the mode of: stepping back
    /// over a switch puts the document back in the form it was edited in.
    ///
    /// A comment anchor does not follow an edit made to the source — there are
    /// no blocks there to anchor to — and is clamped back onto the document on
    /// the way out.
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if mode == self.mode {
            return;
        }
        self.history.record(
            EditKind::Structure,
            self.mode,
            &self.doc,
            self.selection,
            &self.anchors,
        );
        self.dismiss_menus();
        self.switch(mode);
        self.history.landed(EditKind::Structure, self.selection);
        self.reveal = true;
        self.caret_moved();
        cx.emit(EditorEvent::ModeChanged(mode));
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Turn the document into the other form, caret and all. The half of
    /// [`Self::set_mode`] that [`Self::with_mode`] needs without a window.
    pub(super) fn switch(&mut self, mode: Mode) {
        match mode {
            Mode::Source => {
                let (source, offset) =
                    markdown::serialize_at(&self.doc, self.cursor(), &self.marks);
                self.doc = source_doc(&source);
                self.selection = Selection::at(Cursor::new(0, Part::Code, offset));
            }
            Mode::Blocks => {
                let (doc, at) =
                    markdown::parse_at(self.source_text(), self.cursor().offset, &self.marks);
                self.doc = doc;
                ensure_block(&mut self.doc);
                self.selection = Selection::at(at.clamp(&self.doc));
                for anchor in &mut self.anchors {
                    anchor.range = anchor.range.clamp(&self.doc);
                }
            }
        }
        self.mode = mode;
    }

    /// [`Mode::Source`] if the document is in blocks, and back again — what a
    /// toggle in the app's own chrome calls.
    pub fn toggle_source(&mut self, cx: &mut Context<Self>) {
        self.set_mode(
            match self.mode {
                Mode::Blocks => Mode::Source,
                Mode::Source => Mode::Blocks,
            },
            cx,
        );
    }

    /// Whether the document is the thing being edited, rather than its source.
    ///
    /// Every operation that acts on *blocks* asks this: in source mode there is
    /// one block, it is a fence holding a string, and turning it into a heading
    /// or dragging it somewhere would edit the markup rather than the document
    /// the markup spells.
    pub(super) fn blocks(&self) -> bool {
        self.mode == Mode::Blocks
    }

    /// The text of the fence the source is held in — what is being edited in
    /// [`Mode::Source`]. Only meaningful there; in [`Mode::Blocks`] the
    /// document is the truth and this is whatever block zero happens to be.
    pub(super) fn source_text(&self) -> &str {
        self.doc
            .blocks
            .first()
            .and_then(|block| block.text_at(Part::Code))
            .map_or("", |text| text.text.as_str())
    }

    /// Put the source back in the one fence it is edited as.
    ///
    /// Backspace at the start of an empty fence is a merge, and a merge with
    /// nothing above it leaves a paragraph — a block the source view does not
    /// paint and the caret would be stranded in. The text survives either way,
    /// so this is a change of container and never of content.
    pub(super) fn ensure_source(&mut self) {
        if self.doc.blocks.len() == 1 && matches!(self.doc.blocks[0].kind, BlockKind::Code { .. }) {
            return;
        }
        let source = self
            .doc
            .blocks
            .iter()
            .filter_map(|block| block.text_at(*block.parts().first()?))
            .map(|text| text.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        let offset = self.selection.head.offset.min(source.len());
        self.doc = source_doc(&source);
        self.selection = Selection::at(Cursor::new(0, Part::Code, offset));
    }

    /// Shut everything floating. A switch of mode is a new document as far as
    /// a menu anchored to a block is concerned.
    pub(super) fn dismiss_menus(&mut self) {
        self.slash = None;
        self.pasted = None;
        self.url_prompt = None;
        self.hovered = None;
        self.lifted = None;
        self.dropping = None;
    }
}
