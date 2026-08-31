//! Positions that outlive the edits under them.
//!
//! A comment is anchored to a range of text, and that range has to move when
//! the text around it does. What the comment *says* — who wrote it, its
//! replies, whether it is settled — is the app's, the way a link preview is.
//! What is here is the range, and what keeps it over the same words.
//!
//! The anchors live in the editor rather than in the app because
//! [`crate::History`] restores whole-document snapshots: an undo has no delta to
//! map an anchor through, so the store has to sit where the history can carry
//! it back.
//!
//! Mapping follows the **left-sticky** rule the marks already follow
//! (`markdown::Text::insert`): text typed at the end of a range joins it, text
//! typed at the start does not. Both ends take the same arithmetic, so there is
//! no bias to pick per end and no second rule to keep in step with the first.

use std::ops::Range;

use markdown::{Annotation, Cursor, Selection, Splice};

/// The app's key for a thread. Opaque — nothing here looks inside one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommentId(pub u64);

/// A comment's range in the document, and which wash it paints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub id: CommentId,
    pub range: Selection,
    pub state: Annotation,
}

impl Anchor {
    pub fn new(id: CommentId, range: Selection) -> Self {
        Self {
            id,
            range,
            state: Annotation::default(),
        }
    }

    /// Whether the words this pointed at are gone.
    ///
    /// Detached rather than dropped: whether that reads as "outdated" or as
    /// "resolved" is the app's call, and deleting it here would take the choice
    /// away.
    pub fn detached(&self) -> bool {
        self.range.is_collapsed()
    }

    pub(crate) fn map(&mut self, delta: &Delta) {
        let (start, end) = self.range.ordered();
        self.range = match (delta.cursor(start), delta.cursor(end)) {
            (Some(start), Some(end)) => Selection::new(start.min(end), start.max(end)),
            (surviving, other) => Selection::at(surviving.or(other).unwrap_or_default()),
        };
    }
}

/// What one step of a mutation did to the positions in a document.
///
/// Every path through [`crate::Editor::edit`] hands back a list of these, empty
/// for an edit that moved nothing — so a block operation added later does not
/// compile until it says what it did. Silent is the one thing it must not be:
/// an anchor mapped through the wrong shift points at the wrong words and
/// nothing complains.
pub(crate) enum Delta {
    /// Text went in, out, or both.
    Spliced(Splice),
    /// A run of blocks left `at`, and arrived at `to` unless it went away.
    Moved { at: Range<usize>, to: Option<usize> },
    /// Blocks appeared at `at`, pushing everything from there down.
    Opened { at: usize, count: usize },
}

impl Delta {
    /// Where `at` ends up, and `None` when the block under it went away.
    fn cursor(&self, at: Cursor) -> Option<Cursor> {
        match self {
            Self::Spliced(splice) => {
                let (start, end) = splice.removed.ordered();
                if at < start {
                    return Some(at);
                }
                // Inside what went, the seam is the only place left to be.
                if at <= end {
                    return Some(splice.caret);
                }
                // Past it in the same text, the seam moved by what replaced it;
                // further down the document, only the block count moved.
                if at.block == end.block && at.part == end.part {
                    return Some(Cursor {
                        offset: splice.caret.offset + (at.offset - end.offset),
                        ..splice.caret
                    });
                }
                Some(Cursor {
                    block: at.block.checked_add_signed(splice.blocks)?,
                    ..at
                })
            }
            Self::Moved { at: span, to } => {
                if span.contains(&at.block) {
                    return Some(Cursor {
                        block: (*to)? + (at.block - span.start),
                        ..at
                    });
                }
                // Drained before it was put back, and `to` is already counted
                // against the hole that left — so this is too.
                let pulled = if at.block >= span.end {
                    at.block - span.len()
                } else {
                    at.block
                };
                let block = match to {
                    Some(to) if pulled >= *to => pulled + span.len(),
                    _ => pulled,
                };
                Some(Cursor { block, ..at })
            }
            Self::Opened { at: opened, count } => Some(Cursor {
                block: if at.block >= *opened {
                    at.block + count
                } else {
                    at.block
                },
                ..at
            }),
        }
    }
}
