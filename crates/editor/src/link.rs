//! The paste menu: a URL landed, and what it could be instead.
//!
//! Notion's shape — the link is already in the block by the time the menu
//! opens, so backing out is doing nothing and the card is the upgrade. Which
//! is also why `Dismiss` is a row rather than only a key: what it leaves
//! behind is a bare URL, and that is a link this model can still write down.

use markdown::Cursor;

/// What a row does.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Dismiss,
    Bookmark,
}

impl Choice {
    pub const ROWS: [Self; 2] = [Self::Dismiss, Self::Bookmark];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dismiss => "Dismiss",
            Self::Bookmark => "Create bookmark",
        }
    }
}

/// An open paste menu: the link that landed, and the row under the pointer.
pub struct Paste {
    /// The block the URL went into — what a bookmark replaces, and what the
    /// menu is anchored under.
    pub at: Cursor,
    pub url: String,
    pub active: usize,
}

impl Paste {
    pub fn open(at: Cursor, url: String) -> Self {
        Self { at, url, active: 0 }
    }

    /// Walk the rows. Two rows do not wrap: past the end is the end.
    pub fn step(&mut self, delta: isize) {
        let last = Choice::ROWS.len() as isize - 1;
        self.active = (self.active as isize + delta).clamp(0, last) as usize;
    }

    pub fn choice(&self) -> Choice {
        Choice::ROWS[self.active]
    }
}
