//! The slash menu: `/` at an empty block, then the block vocabulary.
//!
//! [`markdown::BlockKind`] says it is closed by design because *this* is what
//! it is for — the menu is the enum, and a consumer that needs another block
//! widens the enum rather than registering something here.
//!
//! The editor keeps focus while the menu is open and the query is the text
//! typed after the `/`, which is how Notion does it and why there is no second
//! field to hand focus to.

use gpui::{ScrollHandle, SharedString};
use ui::{popover::Filter, scroll::TransientState};

use markdown::{Align, BlockKind, Cursor, Text};

/// Every block the menu offers, and what each makes.
///
/// A bookmark is deliberately absent: it needs a URL, and a card with none is a
/// blank the reader cannot fill. A pasted URL is where a bookmark comes from.
/// An image is here because it *can* wait — with no URL it paints the row that
/// asks for one.
pub fn items() -> Vec<(SharedString, BlockKind)> {
    let text = Text::default;
    vec![
        ("Text".into(), BlockKind::Paragraph(text())),
        (
            "Heading 1".into(),
            BlockKind::Heading {
                level: 1,
                text: text(),
            },
        ),
        (
            "Heading 2".into(),
            BlockKind::Heading {
                level: 2,
                text: text(),
            },
        ),
        (
            "Heading 3".into(),
            BlockKind::Heading {
                level: 3,
                text: text(),
            },
        ),
        ("Bullet".into(), BlockKind::Bullet(text())),
        (
            "Numbered".into(),
            BlockKind::Ordered {
                number: 1,
                text: text(),
            },
        ),
        (
            "Task".into(),
            BlockKind::Task {
                checked: false,
                text: text(),
            },
        ),
        ("Quote".into(), BlockKind::Quote(text())),
        (
            "Code".into(),
            BlockKind::Code {
                language: None,
                code: text(),
            },
        ),
        (
            "Table".into(),
            BlockKind::Table {
                align: vec![Align::Left; 2],
                header: vec![text(), text()],
                rows: vec![vec![text(), text()]],
            },
        ),
        (
            "Image".into(),
            BlockKind::Image {
                url: String::new(),
                alt: text(),
            },
        ),
        ("Divider".into(), BlockKind::Rule),
    ]
}

/// An open menu: where the `/` sits, and the ranked list under it.
pub struct Slash {
    /// The `/` itself. Everything between it and the caret is the query, and
    /// backspacing onto it closes the menu.
    pub at: Cursor,
    pub filter: Filter,
    /// The list's own scroll, so a walk down the rows can bring one below the
    /// fold into view. Made with the menu, so every open starts at the top.
    pub scroll: ScrollHandle,
    pub bar: TransientState,
}

impl Slash {
    pub fn open(at: Cursor) -> Self {
        Self {
            at,
            filter: Filter::new(items().into_iter().map(|(label, _)| label).collect()),
            scroll: ScrollHandle::new(),
            bar: TransientState::new(),
        }
    }

    pub fn refilter(&mut self, query: &str) {
        self.filter.refilter(query);
        self.scroll.scroll_to_item(0);
    }

    /// Walk the rows, keeping the active one on screen.
    pub fn step(&mut self, delta: isize) {
        self.filter.step(delta);
        if let Some(row) = self.filter.active() {
            self.scroll.scroll_to_item(row);
        }
    }

    /// The block confirming right now would make.
    pub fn choice(&self) -> Option<BlockKind> {
        let ix = self.filter.active_item()?;
        items().into_iter().nth(ix).map(|(_, kind)| kind)
    }

    /// What has been typed since the `/`, or `None` when the caret has left
    /// the run entirely — which is what closes the menu.
    pub fn query(&self, caret: Cursor, text: &str) -> Option<String> {
        if caret.block != self.at.block || caret.part != self.at.part {
            return None;
        }
        let start = self.at.offset + 1;
        if caret.offset < start {
            return None;
        }
        let query = text.get(start..caret.offset)?;
        // A space ends it: `/ ` is a stray slash, not a command.
        (!query.contains(char::is_whitespace)).then(|| query.to_string())
    }
}
