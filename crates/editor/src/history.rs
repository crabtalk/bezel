//! Undo and redo, as coalesced snapshots.
//!
//! A whole [`Doc`] per step rather than a diff. That is what [`ui::input::TextField`]
//! settled on for a string, and the argument carries: a document held in memory
//! is small next to the machinery a transaction log needs, and a snapshot cannot
//! be wrong about what it restores. Steps, not keystrokes — a run of typing
//! coalesces into one, so the limit is deeper than it looks.
//!
//! Coalescing is by **adjacency rather than by a pause**, so there is no timing
//! threshold to invent: the next edit joins the last group when it is the same
//! kind and picks up where that one left off. Anything else — a motion, a
//! structural change, a click — starts a new group.

use ui::history::SnapshotHistory;

use markdown::{Cursor, Doc, Selection};

use crate::{anchor::Anchor, editor::Mode};

/// How many steps a document keeps.
///
/// Deeper than a text field's, because a document is the thing people actually
/// walk backwards through, and bounded because an unbounded history of a
/// growing document is a slow leak nothing reclaims.
pub const DEFAULT_UNDO_LIMIT: usize = 100;

/// What an edit did, so a run of the same kind can coalesce into one step
/// instead of giving the document back a character at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
    /// Anything structural — a split, an indent, a block turned into another.
    /// Never coalesces: these are the steps a reader wants to land on.
    Structure,
}

#[derive(Clone)]
struct Snapshot {
    doc: Doc,
    /// Which form the document was being edited in, so a step back across a
    /// switch to the source lands in the form it was taken in.
    mode: Mode,
    selection: Selection,
    /// Carried with the document because an undo replaces it wholesale: there
    /// is no delta to map an anchor through, so the anchors of that moment have
    /// to be the ones that come back.
    anchors: Vec<Anchor>,
}

/// A document's undo and redo stacks, shared by every editor on it.
pub struct History {
    snapshots: SnapshotHistory<Snapshot>,
}

impl Default for History {
    fn default() -> Self {
        Self::with_limit(DEFAULT_UNDO_LIMIT)
    }
}

impl History {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            snapshots: SnapshotHistory::new(limit),
        }
    }

    /// Drop every step, both ways.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Whether there is a step to take back.
    pub fn can_undo(&self) -> bool {
        self.snapshots.can_undo()
    }

    /// Record the state *before* an edit. `joins` leaves it to the step the
    /// last edit opened — see [`Run::joins`]. Either way the redo stack goes.
    pub fn record(
        &mut self,
        joins: bool,
        mode: Mode,
        doc: &Doc,
        selection: Selection,
        anchors: &[Anchor],
    ) {
        self.snapshots.record((!joins).then(|| Snapshot {
            doc: doc.clone(),
            mode,
            selection,
            anchors: anchors.to_vec(),
        }));
    }

    /// Step back, handing the caller the state to restore. Pushes what it was
    /// given onto the redo stack.
    pub fn undo(
        &mut self,
        mode: Mode,
        doc: &Doc,
        selection: Selection,
        anchors: &[Anchor],
    ) -> Option<Step> {
        let previous = self.snapshots.undo(|| Snapshot {
            doc: doc.clone(),
            mode,
            selection,
            anchors: anchors.to_vec(),
        })?;
        Some(previous.into())
    }

    pub fn redo(
        &mut self,
        mode: Mode,
        doc: &Doc,
        selection: Selection,
        anchors: &[Anchor],
    ) -> Option<Step> {
        let next = self.snapshots.redo(|| Snapshot {
            doc: doc.clone(),
            mode,
            selection,
            anchors: anchors.to_vec(),
        })?;
        Some(next.into())
    }
}

/// Where one editor's run of edits left off: the kind of the last edit and
/// where it *left* the caret, which decides whether the next edit joins that
/// step or opens one of its own.
#[derive(Default)]
pub struct Run {
    last: Option<(EditKind, Cursor)>,
}

impl Run {
    /// Whether an edit of `kind` from `selection` continues the run — same
    /// kind, same text, and picking up exactly where the last one stopped.
    /// Structural edits never do.
    pub fn joins(&self, kind: EditKind, selection: Selection) -> bool {
        kind != EditKind::Structure
            && self.last == Some((kind, selection.head))
            && selection.is_collapsed()
    }

    /// Close the edit, noting where it left the caret. The next edit joins this
    /// run only if it starts from exactly here.
    pub fn landed(&mut self, kind: EditKind, selection: Selection) {
        self.last = (kind != EditKind::Structure).then_some((kind, selection.head));
    }

    /// Anything that is not an edit ends the run — a motion, a click, a focus
    /// change. Without this, typing a word, clicking elsewhere and typing again
    /// would undo as one step across two places.
    pub fn interrupt(&mut self) {
        self.last = None;
    }
}

/// The state a step restores, which is a whole moment rather than a diff.
pub struct Step {
    pub doc: Doc,
    pub mode: Mode,
    pub selection: Selection,
    pub anchors: Vec<Anchor>,
}

impl From<Snapshot> for Step {
    fn from(snapshot: Snapshot) -> Self {
        Self {
            doc: snapshot.doc,
            mode: snapshot.mode,
            selection: snapshot.selection,
            anchors: snapshot.anchors,
        }
    }
}
