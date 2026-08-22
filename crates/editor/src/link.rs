//! The paste menu: a URL landed, and what it could be instead.
//!
//! Notion's shape — the link is already in the block by the time the menu
//! opens, so backing out is doing nothing and the richer form is the upgrade.
//! Which is also why `Dismiss` is a row rather than only a key: what it leaves
//! behind is a bare URL, and that is a link this model can still write down.

use markdown::Cursor;

/// What a row does.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Dismiss,
    Chip,
    Bookmark,
    Embed,
    Image,
}

impl Choice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dismiss => "Dismiss",
            Self::Chip => "Create chip",
            Self::Bookmark => "Create bookmark",
            Self::Embed => "Create embed",
            Self::Image => "Create image",
        }
    }
}

/// An open paste menu: the link that landed, and the row under the pointer.
pub struct Paste {
    /// The block the URL went into — what a bookmark replaces, and where the
    /// menu is anchored.
    pub at: Cursor,
    pub url: String,
    /// Whether the URL has a block to itself, which is what decides both the
    /// rows below and what a chip becomes: an element with a favicon there, a
    /// mark over shaped text anywhere else.
    pub alone: bool,
    /// What this spot can hold. A card is a block, so it is offered only where
    /// the URL has one; a chip fits either way. A picture is offered where a
    /// card is, and only for a URL whose name says it is one — a row that
    /// paints a broken box is worse than a row that is not there.
    pub rows: Vec<Choice>,
    pub active: usize,
}

impl Paste {
    pub fn open(at: Cursor, url: String, alone: bool) -> Self {
        let mut rows = vec![Choice::Dismiss, Choice::Chip];
        if alone {
            rows.extend([Choice::Bookmark, Choice::Embed]);
            if markdown::is_image(&url) {
                rows.push(Choice::Image);
            }
        }
        Self {
            at,
            url,
            alone,
            rows,
            active: 0,
        }
    }

    /// Walk the rows. Two rows do not wrap: past the end is the end.
    pub fn step(&mut self, delta: isize) {
        let last = self.rows.len() as isize - 1;
        self.active = (self.active as isize + delta).clamp(0, last) as usize;
    }

    pub fn choice(&self) -> Choice {
        self.rows[self.active]
    }
}
