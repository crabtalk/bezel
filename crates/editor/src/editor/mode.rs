//! Source mode and the switch between it and blocks.

use super::*;

impl Editor {
    /// The document as markdown. In [`Mode::Blocks`] it is the text the
    /// document was read from, with every block edited since written again
    /// in normalized form. In [`Mode::Source`] it is the text being edited,
    /// exactly as it stands.
    pub fn source(&self) -> String {
        source(&self.doc, self.mode, &self.synced)
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
            quote: self.doc.blocks.get(at.block).and_then(|block| block.quote),
            fenceable: self.blocks() && fenceable(&self.doc, self.selection),
        }
    }

    /// Switch between the document and its markdown, carrying the caret across.
    ///
    /// One undo step, and one the history knows the mode of: stepping back
    /// over a switch puts the document back in the form it was edited in.
    ///
    /// Anchors cross with it, into the coordinates of the form switched to. One
    /// with an end that cannot be placed there comes out detached.
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if mode == self.mode {
            return;
        }
        let (was, selection) = (self.mode, self.selection);
        self.document.update(cx, |document, _| {
            document
                .history
                .record(false, was, &self.doc, selection, &self.anchors)
        });
        self.dismiss_menus();
        self.switch(mode);
        self.run.landed(EditKind::Structure, self.selection);
        self.reveal = Some(Reveal::Nearest);
        self.caret_moved();
        self.publish(Change::Replaced, cx);
        cx.emit(EditorEvent::ModeChanged(mode));
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Turn the document into the other form, caret and all. The half of
    /// [`Self::set_mode`] that needs no window.
    pub(super) fn switch(&mut self, mode: Mode) {
        let caret = self.cursor();
        let caret = switch(
            &mut self.doc,
            &mut self.synced.borrow_mut(),
            &mut self.anchors,
            mode,
            caret,
        );
        self.selection = Selection::at(caret);
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

    /// Bring the source view's text up to the document. A first build
    /// highlights in place; an edit after it is highlighted on the background
    /// executor, and the view shows the spans from before it until then.
    /// A highlighter installed since the last highlight is run the same way.
    pub(super) fn refresh_source_view(&mut self, cx: &mut Context<Self>) {
        let revision = self.doc.revision();
        let job = match &mut self.source_view {
            Some((built, view)) => {
                let edited = match *built == revision {
                    true => None,
                    false => {
                        *built = revision;
                        view.edit(source_of(&self.doc), cx)
                    }
                };
                edited.or_else(|| view.rehighlight(cx))
            }
            None => {
                let view = markdown::SourceText::new(source_of(&self.doc), cx);
                self.source_view = Some((revision, view));
                None
            }
        };
        let Some(job) = job else { return };
        self.source_spans = Some(cx.spawn(async move |editor, cx| {
            let spans = cx
                .background_executor()
                .spawn(async move { job.run() })
                .await;
            editor
                .update(cx, |editor, cx| {
                    if let Some((_, view)) = &mut editor.source_view
                        && view.set_spans(spans)
                    {
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    /// Drop the source view and any highlighting under way for it.
    pub(super) fn drop_source_view(&mut self) {
        self.source_view = None;
        self.source_spans = None;
    }

    /// The source view's text. Current only after [`Self::refresh_source_view`].
    pub(super) fn source_view(&self) -> &markdown::SourceText {
        &self
            .source_view
            .as_ref()
            .expect("render refreshes the source view before reading it")
            .1
    }

    /// Lines in the source, counting the empty one after a trailing newline.
    /// Only meaningful in [`Mode::Source`], as [`Self::source_text`] is.
    pub fn source_lines(&self) -> usize {
        match &self.source_view {
            Some((built, view)) if *built == self.doc.revision() => view.lines(),
            _ => {
                self.source_text()
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1
            }
        }
    }

    /// The text of the fence the source is held in — what is being edited in
    /// [`Mode::Source`]. Only meaningful there; in [`Mode::Blocks`] the
    /// document is the truth and this is whatever block zero happens to be.
    pub(super) fn source_text(&self) -> &str {
        source_of(&self.doc)
    }

    /// Put the source back in the one fence it is edited as.
    ///
    /// Backspace at the start of an empty fence is a merge, and a merge with
    /// nothing above it leaves a paragraph — a block the source view does not
    /// paint and the caret would be stranded in. The text survives either way,
    /// so this is a change of container and never of content.
    ///
    /// Whether it had to.
    pub(super) fn ensure_source(&mut self) -> bool {
        if self.doc.blocks.len() == 1 && matches!(self.doc.blocks[0].kind, BlockKind::Code { .. }) {
            return false;
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
        self.doc.set(source_doc(&source));
        self.selection = Selection::at(Cursor::new(0, Part::Code, offset));
        for anchor in &mut self.anchors {
            anchor.range = anchor.range.clamp(&self.doc);
        }
        true
    }

    /// Shut everything floating. A switch of mode is a new document as far as
    /// a menu anchored to a block is concerned.
    pub(super) fn dismiss_menus(&mut self) {
        self.slash = None;
        self.pasted = None;
        self.url_prompt = None;
        self.hovered = None;
        self.lifted = None;
        self.table_drag = None;
        self.dropping = None;
    }
}

/// See [`Editor::source_text`].
pub(crate) fn source_of(doc: &Doc) -> &str {
    doc.blocks
        .first()
        .and_then(|block| block.text_at(Part::Code))
        .map_or("", |text| text.text.as_str())
}

/// `doc` as markdown — see [`Editor::source`].
pub(crate) fn source(doc: &Tracked, mode: Mode, synced: &RefCell<Synced>) -> String {
    match mode {
        Mode::Blocks => {
            let mut synced = synced.borrow_mut();
            synced.set_doc(doc.shared());
            synced.text().to_owned()
        }
        Mode::Source => source_of(doc).to_string(),
    }
}

/// Turn `doc` into `mode`, carrying `anchors` into the coordinates of the
/// form switched to, and hand back where `caret` lands. An anchor with an end
/// that cannot be placed there comes out detached.
///
/// `synced` is brought up to `doc` first. The markdown it holds is then the
/// source, and its blocks the document, so neither is converted whole.
pub(crate) fn switch(
    doc: &mut Tracked,
    synced: &mut Synced,
    anchors: &mut [Anchor],
    mode: Mode,
    caret: Cursor,
) -> Cursor {
    let ends: Vec<(Cursor, Cursor)> = anchors
        .iter()
        .map(|anchor| anchor.range.ordered())
        .collect();
    match mode {
        Mode::Source => {
            synced.set_doc(doc.shared());
            let mut at = vec![caret];
            at.extend(ends.iter().flat_map(|(start, end)| [*start, *end]));
            let offsets = synced.offsets(&at);
            let source = synced.text();
            doc.set(source_doc(source));
            let place = |ix: usize| offsets[ix].map(|offset| Cursor::new(0, Part::Code, offset));
            place_anchors(anchors, |ix| place(ix + 1));
            place(0).unwrap_or(Cursor::new(0, Part::Code, source.len()))
        }
        Mode::Blocks => {
            let mut offsets = vec![caret.offset];
            offsets.extend(
                ends.iter()
                    .flat_map(|(start, end)| [start.offset, end.offset]),
            );
            synced.set_text(source_of(doc));
            let at = synced.cursors(&offsets);
            doc.share(synced.doc().clone());
            doc.ensure_block();
            let place = |ix: usize| at[ix].map(|at: Cursor| at.clamp(doc));
            let caret = place(0).unwrap_or_else(|| Cursor::default().clamp(doc));
            let ends: Vec<_> = (1..at.len()).map(place).collect();
            place_anchors(anchors, |ix| ends[ix]);
            caret
        }
    }
}

/// Put each anchor at the ends `place` gives for it, indexed start, end,
/// start, end. An anchor with an end `place` cannot give collapses onto the
/// other, detached.
fn place_anchors(anchors: &mut [Anchor], place: impl Fn(usize) -> Option<Cursor>) {
    for (ix, anchor) in anchors.iter_mut().enumerate() {
        anchor.range = match (place(2 * ix), place(2 * ix + 1)) {
            (Some(start), Some(end)) => Selection::new(start, end),
            (surviving, other) => Selection::at(surviving.or(other).unwrap_or_default()),
        };
    }
}
