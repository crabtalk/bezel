//! The document every editor on it shares.
//!
//! An [`Editor`](crate::Editor) is one view of a [`Document`]: its caret, its
//! menus, its scroll and its layout are its own, and the text, the form it is
//! edited in, the comment anchors and the undo stack are the document's. Two
//! editors on one document show the same text as it is typed.
//!
//! Each editor holds a snapshot of the document and edits that, then hands
//! the result back; the document tells every other editor on it, which takes
//! the new snapshot and carries its caret through what changed.

use gpui::{Context, EntityId, EventEmitter};
use markdown::{AppExt as _, Block, BlockKind, Cursor, Doc, Part, Selection, Synced, Text};
use std::{cell::RefCell, rc::Rc};

use crate::{
    anchor::{Anchor, Delta, SourceAnchor, SourceRanges},
    editor::{
        Mode,
        mode::{source_of, switch},
    },
    history::History,
};

/// A document, and a count of the changes made to it.
///
/// Reads go through `Deref`. [`Tracked::edit`] and [`Tracked::set`] are the
/// only ways to change it, and both move [`Tracked::revision`], so something
/// derived from a revision is current while the revision is. Cloning is
/// cheap: the document is shared until one of the clones is edited.
#[derive(Clone)]
pub(crate) struct Tracked {
    doc: Rc<Doc>,
    revision: u64,
}

impl Tracked {
    fn new(doc: Rc<Doc>) -> Self {
        Self { doc, revision: 0 }
    }

    pub(crate) fn edit(&mut self) -> &mut Doc {
        self.revision += 1;
        Rc::make_mut(&mut self.doc)
    }

    pub(crate) fn set(&mut self, doc: Doc) {
        *self.edit() = doc;
    }

    /// [`Self::set`] to a document held elsewhere, shared until edited.
    pub(crate) fn share(&mut self, doc: Rc<Doc>) {
        self.revision += 1;
        self.doc = doc;
    }

    pub(crate) fn shared(&self) -> Rc<Doc> {
        self.doc.clone()
    }

    /// Give an empty document a block to hold a caret, and say whether one
    /// was needed.
    ///
    /// A [`Doc`] with no blocks is a legitimate document — it is what
    /// `parse("")` returns — but it is not something that can be edited:
    /// nothing paints, so there is no caret and no placeholder, and neither a
    /// click nor a hit test has a target to find.
    pub(crate) fn ensure_block(&mut self) -> bool {
        if !self.doc.blocks.is_empty() {
            return false;
        }
        self.edit()
            .blocks
            .push(Block::new(BlockKind::Paragraph(Text::default())));
        true
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}

impl std::ops::Deref for Tracked {
    type Target = Doc;

    fn deref(&self) -> &Doc {
        &self.doc
    }
}

/// A markdown document as its editors share it.
pub struct Document {
    pub(crate) doc: Tracked,
    /// Which form the document is in. In [`Mode::Source`] `doc` is one fenced
    /// block holding the markdown.
    pub(crate) mode: Mode,
    /// The document as markdown and as blocks. In [`Mode::Blocks`] it is kept
    /// up with `doc` as each change is handed back; in [`Mode::Source`] it
    /// stands where the last switch left it, and catches up with the source at
    /// the next. Every editor on the document holds it too.
    pub(crate) synced: Rc<RefCell<Synced>>,
    /// The dialect this document is read and written in — the app's own marks,
    /// taken once at construction. One document, one spelling: a document that
    /// changed dialect between a read and a write would rewrite itself.
    pub(crate) marks: markdown::Marks,
    /// Comment ranges, mapped through every edit and snapshotted with the
    /// document. Here rather than in the app because an undo restores a whole
    /// document and leaves no delta an app could map its own copy through.
    pub(crate) anchors: Vec<Anchor>,
    pub(crate) history: History,
}

/// A document changed under its editors. Every editor on it but the one that
/// changed it takes the new snapshot.
pub struct DocumentEvent {
    /// The editor the change came from; `None` for a change made on the
    /// document itself.
    pub(crate) by: Option<EntityId>,
    pub(crate) change: Change,
}

impl DocumentEvent {
    /// Whether the text changed: not an IME composition still under way, and
    /// not only the anchors.
    pub fn edited(&self) -> bool {
        matches!(self.change, Change::Edited(_) | Change::Replaced)
    }
}

/// What a [`DocumentEvent`] did to positions in the document.
pub(crate) enum Change {
    /// An edit, through these steps — what a caret elsewhere is carried
    /// through.
    Edited(Rc<[Delta]>),
    /// The document was put back whole — an undo, a switch of form — and
    /// leaves no step to carry a caret through.
    Replaced,
    /// An IME composition moved the text; nothing is committed yet.
    Composing,
    /// [`Document::reload`] read the text again from what it was saved as.
    Reloaded,
    /// Only the anchors moved.
    Anchors,
}

impl EventEmitter<DocumentEvent> for Document {}

impl Document {
    pub fn new(source: &str, cx: &mut Context<Self>) -> Self {
        let marks = cx.marks();
        let synced = Synced::new(source, &marks);
        let mut doc = Tracked::new(synced.doc().clone());
        doc.ensure_block();
        Self {
            doc,
            mode: Mode::default(),
            synced: Rc::new(RefCell::new(synced)),
            marks,
            anchors: Vec::new(),
            history: History::default(),
        }
    }

    /// How many undo steps to keep; [`crate::DEFAULT_UNDO_LIMIT`] until set. A
    /// run of typing coalesces into one step.
    pub fn with_undo_limit(mut self, limit: usize) -> Self {
        self.history = History::with_limit(limit);
        self
    }

    /// Read and write this document with marks of its own, rather than the ones
    /// [`markdown::AppExt::set_marks`] installed. For an app whose documents do
    /// not all speak the same dialect.
    pub fn with_marks(mut self, marks: markdown::Marks) -> Self {
        let source = self.source();
        self.marks = marks;
        let synced = Synced::new(source, &self.marks);
        self.doc.share(synced.doc().clone());
        self.doc.ensure_block();
        *self.synced.borrow_mut() = synced;
        self
    }

    /// Open in [`Mode::Source`] rather than on the document — an app whose
    /// document is a markdown file first. Nothing is recorded: this is where
    /// the document starts, not a switch to step back over.
    pub fn with_mode(mut self, mode: Mode) -> Self {
        if mode != self.mode {
            switch(
                &mut self.doc,
                &mut self.synced.borrow_mut(),
                &mut self.anchors,
                mode,
                Default::default(),
            );
            self.mode = mode;
        }
        self
    }

    /// The document as markdown — see [`crate::Editor::source`].
    pub fn source(&self) -> String {
        crate::editor::mode::source(&self.doc, self.mode, &self.synced)
    }

    /// Which form the document is being edited in.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// Read the document again from `source`, in the form it is in. The undo
    /// stack and the anchors go with the old text.
    pub fn reload(&mut self, source: &str, cx: &mut Context<Self>) {
        let synced = Synced::new(source, &self.marks);
        self.doc.share(synced.doc().clone());
        self.doc.ensure_block();
        *self.synced.borrow_mut() = synced;
        self.anchors.clear();
        if self.mode == Mode::Source {
            switch(
                &mut self.doc,
                &mut self.synced.borrow_mut(),
                &mut self.anchors,
                Mode::Source,
                Cursor::default(),
            );
        }
        self.history.clear();
        cx.emit(DocumentEvent {
            by: None,
            change: Change::Reloaded,
        });
        cx.notify();
    }

    /// The comment ranges, mapped up to date with the document.
    ///
    /// A range that reads [`Anchor::detached`] lost the words it pointed at.
    /// It is kept rather than dropped, because whether that means "outdated" or
    /// "resolved" is the app's question.
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// Hand over the whole list — the app keeps the threads, this keeps their
    /// ranges. One entry point rather than add/remove/update, since the app is
    /// already holding the list that decides all three.
    ///
    /// Ranges are positions in [`Self::doc`] as it stands: the source fence in
    /// [`Mode::Source`], the blocks otherwise.
    pub fn set_anchors(&mut self, anchors: Vec<Anchor>, cx: &mut Context<Self>) {
        self.anchors = anchors;
        cx.emit(DocumentEvent {
            by: None,
            change: Change::Anchors,
        });
        cx.notify();
    }

    /// The markdown to save, and where each anchor's ends land in it, in byte
    /// offsets, in the order of [`Self::anchors`]. A range with an end the
    /// markdown has no place for comes out `None`.
    ///
    /// The string is [`Self::source`] but for whitespace a placed end keeps
    /// from being trailing; the ranges index this string, so it is the one to
    /// write beside them.
    pub fn source_with_anchors(&self) -> (String, SourceRanges) {
        let ends: Vec<_> = self
            .anchors
            .iter()
            .flat_map(|anchor| {
                let (start, end) = anchor.range.ordered();
                [start, end]
            })
            .collect();
        let (source, offsets) = match self.mode {
            Mode::Blocks => {
                let mut synced = self.synced.borrow_mut();
                synced.set_doc(self.doc.shared());
                let offsets = synced.offsets(&ends);
                (synced.text().to_owned(), offsets)
            }
            Mode::Source => {
                let source = source_of(&self.doc).to_string();
                let offsets = ends
                    .iter()
                    .map(|at| (at.offset <= source.len()).then_some(at.offset))
                    .collect();
                (source, offsets)
            }
        };
        let ranges = self
            .anchors
            .iter()
            .enumerate()
            .map(|(ix, anchor)| {
                let range = offsets[2 * ix].zip(offsets[2 * ix + 1]);
                (anchor.id, range.map(|(start, end)| start..end))
            })
            .collect();
        (source, ranges)
    }

    /// [`Self::set_anchors`] with ranges given in byte offsets into `source`,
    /// the markdown the document was read from. A range with an end the
    /// document has no place for comes in detached.
    pub fn set_source_anchors(
        &mut self,
        source: &str,
        anchors: Vec<SourceAnchor>,
        cx: &mut Context<Self>,
    ) {
        let offsets: Vec<usize> = anchors
            .iter()
            .flat_map(|anchor| [anchor.range.start, anchor.range.end])
            .collect();
        let ends: Vec<Option<Cursor>> = match self.mode {
            Mode::Blocks => markdown::parse_at_many(source, &offsets, &self.marks).1,
            Mode::Source => offsets
                .iter()
                .map(|offset| {
                    (*offset <= source.len()).then(|| Cursor::new(0, Part::Code, *offset))
                })
                .collect(),
        };
        self.anchors = anchors
            .into_iter()
            .enumerate()
            .map(|(ix, anchor)| {
                let range = match (ends[2 * ix], ends[2 * ix + 1]) {
                    (Some(start), Some(end)) => Selection::new(start, end),
                    (surviving, other) => Selection::at(surviving.or(other).unwrap_or_default()),
                };
                Anchor {
                    id: anchor.id,
                    range: range.clamp(&self.doc),
                    state: anchor.state,
                    actionable: anchor.actionable,
                }
            })
            .collect();
        cx.emit(DocumentEvent {
            by: None,
            change: Change::Anchors,
        });
        cx.notify();
    }
}
