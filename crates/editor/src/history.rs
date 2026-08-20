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

use std::collections::VecDeque;

use markdown::{Cursor, Doc, Selection};

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
    selection: Selection,
}

pub struct History {
    /// Points to return to, oldest first.
    undo: VecDeque<Snapshot>,
    /// Undone points, newest last. Cleared by any fresh edit — the usual model,
    /// and the only one where redo cannot resurrect a branch the document has
    /// already diverged from.
    redo: Vec<Snapshot>,
    limit: usize,
    /// The kind of the last edit and where it *left* the caret, which is what
    /// decides whether the next edit joins that group or starts a new one.
    last: Option<(EditKind, Cursor)>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit: DEFAULT_UNDO_LIMIT,
            last: None,
        }
    }
}

impl History {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }

    /// Record the state *before* an edit of `kind`; [`History::landed`] closes
    /// it afterwards. A run of insertions leaves one step, so undo gives back
    /// the word rather than the letter.
    pub fn record(&mut self, kind: EditKind, doc: &Doc, selection: Selection) {
        self.redo.clear();
        if self.joins(kind, selection) {
            return;
        }
        self.undo.push_back(Snapshot {
            doc: doc.clone(),
            selection,
        });
        while self.undo.len() > self.limit {
            self.undo.pop_front();
        }
        self.last = None;
    }

    /// Close the edit, noting where it left the caret. The next edit joins this
    /// group only if it starts from exactly here.
    pub fn landed(&mut self, kind: EditKind, selection: Selection) {
        self.last = (kind != EditKind::Structure).then_some((kind, selection.head));
    }

    /// Whether this edit continues the group the last one opened — same kind,
    /// same text, and picking up exactly where that one stopped.
    fn joins(&self, kind: EditKind, selection: Selection) -> bool {
        if kind == EditKind::Structure || self.undo.is_empty() {
            return false;
        }
        self.last == Some((kind, selection.head)) && selection.is_collapsed()
    }

    /// Anything that is not an edit ends the group — a motion, a click, a
    /// focus change. Without this, typing a word, clicking elsewhere and typing
    /// again would undo as one step across two places.
    pub fn interrupt(&mut self) {
        self.last = None;
    }

    /// Step back, handing the caller the document to restore. Pushes what it
    /// was given onto the redo stack.
    pub fn undo(&mut self, doc: &Doc, selection: Selection) -> Option<(Doc, Selection)> {
        let previous = self.undo.pop_back()?;
        self.redo.push(Snapshot {
            doc: doc.clone(),
            selection,
        });
        self.last = None;
        Some((previous.doc, previous.selection))
    }

    pub fn redo(&mut self, doc: &Doc, selection: Selection) -> Option<(Doc, Selection)> {
        let next = self.redo.pop()?;
        self.undo.push_back(Snapshot {
            doc: doc.clone(),
            selection,
        });
        self.last = None;
        Some((next.doc, next.selection))
    }
}
