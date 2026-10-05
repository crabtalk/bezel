//! The paste menu: a URL landed, and what it could be instead.
//!
//! Notion's shape — the link is already in the block by the time the menu
//! opens, so backing out is doing nothing and the richer form is the upgrade.
//! Which is also why `Dismiss` is a row rather than only a key: what it leaves
//! behind is a bare URL, and that is a link this model can still write down.

use markdown::Cursor;
use ui::menu::{self, Item};

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
    /// What this spot can hold. A card is a block, so it is offered only where
    /// the URL has one; a chip fits either way. A picture is offered where a
    /// card is, and only for a URL whose name says it is one — a row that
    /// paints a broken box is worse than a row that is not there.
    pub rows: Vec<Choice>,
    pub cursor: menu::Cursor,
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
        let mut paste = Self {
            at,
            url,
            rows,
            cursor: menu::Cursor::default(),
        };
        paste.step(1);
        paste
    }

    /// The rows as [`ui::menu::card`] paints them.
    pub fn menu(&self) -> Vec<Item> {
        self.rows
            .iter()
            .map(|choice| Item::action(choice.label()))
            .collect()
    }

    pub fn step(&mut self, delta: isize) {
        let menu = self.menu();
        self.cursor.step(&menu, delta);
    }

    /// The row at `path`, or the live one for `None`.
    pub fn choice(&self, path: Option<&[usize]>) -> Option<Choice> {
        let row = match path {
            Some(path) => *path.first()?,
            None => self.cursor.row()?,
        };
        self.rows.get(row).copied()
    }
}
