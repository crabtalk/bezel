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
use markdown::{Annotation, AppExt as _, Cursor, Doc, Part, Selection};
use std::{ops::Range, rc::Rc};

use crate::{
    anchor::{Anchor, AnchorId, Delta, SourceRanges},
    editor::{
        Mode, ensure_block,
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
    fn new(doc: Doc) -> Self {
        Self {
            doc: Rc::new(doc),
            revision: 0,
        }
    }

    pub(crate) fn edit(&mut self) -> &mut Doc {
        self.revision += 1;
        Rc::make_mut(&mut self.doc)
    }

    pub(crate) fn set(&mut self, doc: Doc) {
        *self.edit() = doc;
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
        let mut doc = markdown::parse_with(source, &marks);
        ensure_block(&mut doc);
        Self {
            doc: Tracked::new(doc),
            mode: Mode::default(),
            marks,
            anchors: Vec::new(),
            history: History::default(),
        }
    }

    /// How many undo steps to keep. App-wide configuration would be a gpui
    /// global alongside [`crate::init`], not a `Theme` field — the theme is
    /// rebuilt on every light/dark switch, which would quietly reset anything
    /// behavioural parked in it.
    pub fn with_undo_limit(mut self, limit: usize) -> Self {
        self.history = History::with_limit(limit);
        self
    }

    /// Read and write this document with marks of its own, rather than the ones
    /// [`markdown::AppExt::set_marks`] installed. For an app whose documents do
    /// not all speak the same dialect.
    pub fn with_marks(mut self, marks: markdown::Marks) -> Self {
        let source = crate::editor::mode::source(&self.doc, self.mode, &self.marks);
        self.marks = marks;
        let mut doc = markdown::parse_with(&source, &self.marks);
        ensure_block(&mut doc);
        self.doc.set(doc);
        self
    }

    /// Open in [`Mode::Source`] rather than on the document — an app whose
    /// document is a markdown file first. Nothing is recorded: this is where
    /// the document starts, not a switch to step back over.
    pub fn with_mode(mut self, mode: Mode) -> Self {
        if mode != self.mode {
            switch(
                &mut self.doc,
                &mut self.anchors,
                &self.marks,
                mode,
                Default::default(),
            );
            self.mode = mode;
        }
        self
    }

    /// The document as markdown — see [`crate::Editor::source`].
    pub fn source(&self) -> String {
        crate::editor::mode::source(&self.doc, self.mode, &self.marks)
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
        let mut doc = markdown::parse_with(source, &self.marks);
        ensure_block(&mut doc);
        self.doc.set(doc);
        self.anchors.clear();
        if self.mode == Mode::Source {
            switch(
                &mut self.doc,
                &mut self.anchors,
                &self.marks,
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
            Mode::Blocks => markdown::serialize_at_many(&self.doc, &ends, &self.marks),
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
        anchors: Vec<(AnchorId, Range<usize>, Annotation)>,
        cx: &mut Context<Self>,
    ) {
        let offsets: Vec<usize> = anchors
            .iter()
            .flat_map(|(_, range, _)| [range.start, range.end])
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
            .map(|(ix, (id, _, state))| {
                let range = match (ends[2 * ix], ends[2 * ix + 1]) {
                    (Some(start), Some(end)) => Selection::new(start, end),
                    (surviving, other) => Selection::at(surviving.or(other).unwrap_or_default()),
                };
                Anchor {
                    id,
                    range: range.clamp(&self.doc),
                    state,
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
