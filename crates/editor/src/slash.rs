//! The slash menu: `/` at an empty block, then the block vocabulary.
//!
//! [`markdown::BlockKind`] says it is closed by design because *this* is what
//! it is for — the menu is the enum, and a consumer that needs another block
//! widens the enum rather than registering something here.
//!
//! The editor keeps focus while the menu is open and the query is the text
//! typed after the `/`, which is how Notion does it and why there is no second
//! field to hand focus to.

use gpui::SharedString;
use ui::{
    menu::{self, Item},
    popover::filter_indices,
};

use markdown::{Align, BlockKind, Cursor, QuoteKind, Text};

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
        (
            "Quote".into(),
            BlockKind::Quote {
                kind: None,
                text: text(),
            },
        ),
        (
            "Quote (Note)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Note),
                text: text(),
            },
        ),
        (
            "Quote (Tip)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Tip),
                text: text(),
            },
        ),
        (
            "Quote (Important)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Important),
                text: text(),
            },
        ),
        (
            "Quote (Warning)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Warning),
                text: text(),
            },
        ),
        (
            "Quote (Caution)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Caution),
                text: text(),
            },
        ),
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
                width: None,
            },
        ),
        ("Divider".into(), BlockKind::Rule),
    ]
}

/// What [`items`] calls this block, and `None` for one the menu does not offer
/// — a bookmark, which needs a URL nobody can type into a menu row.
///
/// Matched on the kind alone: a row spells a heading's level and nothing else,
/// so a numbered list at 7, a fence tagged `rs` and a table of any size are all
/// the row they came from.
pub fn label(kind: &BlockKind) -> Option<SharedString> {
    items()
        .into_iter()
        .find(|(_, row)| same(row, kind))
        .map(|(label, _)| label)
}

fn same(row: &BlockKind, kind: &BlockKind) -> bool {
    match (row, kind) {
        (BlockKind::Heading { level: a, .. }, BlockKind::Heading { level: b, .. }) => a == b,
        (BlockKind::Quote { kind: a, .. }, BlockKind::Quote { kind: b, .. }) => a == b,
        (row, kind) => std::mem::discriminant(row) == std::mem::discriminant(kind),
    }
}

/// A row of the open menu: one block, or a group of them behind a submenu.
/// Indices are into [`items`].
enum Row {
    Block(usize),
    Group(SharedString, Vec<usize>),
}

/// An open menu: where the `/` sits, and the rows under it.
pub struct Slash {
    /// The `/` itself. Everything between it and the caret is the query, and
    /// backspacing onto it closes the menu.
    pub at: Cursor,
    rows: Vec<Row>,
    pub cursor: menu::Cursor,
}

impl Slash {
    pub fn open(at: Cursor) -> Self {
        let mut slash = Self {
            at,
            rows: Vec::new(),
            cursor: menu::Cursor::default(),
        };
        slash.refilter("");
        slash
    }

    /// With no query the quotes sit behind one row; a query ranks every block
    /// flat.
    pub fn refilter(&mut self, query: &str) {
        let items = items();
        self.rows = if query.is_empty() {
            let mut rows = Vec::new();
            let mut quotes = Vec::new();
            for (ix, (_, kind)) in items.iter().enumerate() {
                if !matches!(kind, BlockKind::Quote { .. }) {
                    rows.push(Row::Block(ix));
                    continue;
                }
                if quotes.is_empty() {
                    rows.push(Row::Group("Quote".into(), Vec::new()));
                }
                quotes.push(ix);
            }
            if let Some(Row::Group(_, group)) =
                rows.iter_mut().find(|row| matches!(row, Row::Group(..)))
            {
                *group = quotes;
            }
            rows
        } else {
            let labels: Vec<SharedString> = items.into_iter().map(|(label, _)| label).collect();
            filter_indices(query, &labels)
                .into_iter()
                .map(Row::Block)
                .collect()
        };
        self.cursor.clear();
        self.cursor.step(&self.menu(), 1);
    }

    /// The rows as [`ui::menu::card`] paints them.
    pub fn menu(&self) -> Vec<Item> {
        let items = items();
        self.rows
            .iter()
            .map(|row| match row {
                Row::Block(ix) => Item::action(items[*ix].0.clone()),
                Row::Group(label, group) => Item::submenu(
                    label.clone(),
                    group
                        .iter()
                        .map(|ix| {
                            let label = &items[*ix].0;
                            let short = label
                                .strip_prefix("Quote (")
                                .and_then(|rest| rest.strip_suffix(')'))
                                .unwrap_or(label);
                            Item::action(short.to_string())
                        })
                        .collect(),
                ),
            })
            .collect()
    }

    /// Walk the rows of the innermost open panel.
    pub fn step(&mut self, delta: isize) {
        let menu = self.menu();
        self.cursor.step(&menu, delta);
    }

    /// Open the group under the live row. `false` when it is not one.
    pub fn descend(&mut self) -> bool {
        let menu = self.menu();
        self.cursor.descend(&menu)
    }

    /// Whether the live row is a group, which Enter opens rather than picks.
    pub fn on_group(&self) -> bool {
        self.cursor.path().is_some_and(|path| {
            matches!(
                (self.rows.get(path[0]), path.len()),
                (Some(Row::Group(..)), 1)
            )
        })
    }

    /// The block confirming right now would make.
    pub fn choice(&self) -> Option<BlockKind> {
        self.kind_at(&self.cursor.path()?)
    }

    /// The block the row at `path` makes, and `None` for a group.
    pub fn kind_at(&self, path: &[usize]) -> Option<BlockKind> {
        let ix = match (self.rows.get(*path.first()?)?, path.get(1)) {
            (Row::Block(ix), None) => *ix,
            (Row::Group(_, group), Some(row)) => *group.get(*row)?,
            _ => return None,
        };
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
