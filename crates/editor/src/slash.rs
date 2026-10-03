//! The slash menu: `/` at an empty block, then the block vocabulary and the
//! rows an app adds with [`crate::AppExt::set_slash_items`].
//!
//! The editor keeps focus while the menu is open and the query is the text
//! typed after the `/`, which is how Notion does it and why there is no second
//! field to hand focus to.

use std::rc::Rc;

use gpui::{App, Global, SharedString, WeakEntity, Window};
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

/// What a [`SlashAction::Run`] row calls.
pub type SlashRun = Rc<dyn Fn(SlashAt, &mut Window, &mut App)>;

/// What picking a slash row does.
#[derive(Clone)]
pub enum SlashAction {
    /// Turn the block the `/` was typed in into this one.
    Block(BlockKind),
    /// Hand the block to the app. The `/query` is gone from it by then; what
    /// the block becomes, and where the caret goes, is the app's.
    Run(SlashRun),
}

/// The block a [`SlashAction::Run`] row was picked in.
#[derive(Clone)]
pub struct SlashAt {
    pub editor: WeakEntity<crate::Editor>,
    pub block: usize,
}

/// A row an app adds to the slash menu, after the editor's own.
#[derive(Clone)]
pub enum SlashItem {
    Row {
        label: SharedString,
        action: SlashAction,
    },
    /// Rows behind a submenu. A query ranks them flat, each called
    /// `Group (row)`.
    Group {
        label: SharedString,
        rows: Vec<(SharedString, SlashAction)>,
    },
}

/// What the app installed.
pub(crate) struct AppItems(pub Vec<SlashItem>);

impl Global for AppItems {}

/// The rows the app installed, or none.
pub(crate) fn app_items(cx: &App) -> Vec<SlashItem> {
    cx.try_global::<AppItems>()
        .map(|AppItems(items)| items.clone())
        .unwrap_or_default()
}

/// One row the open menu can offer: what a query matches, what a submenu
/// shows, the group it sits in, and what picking it does.
#[derive(Clone)]
struct Entry {
    label: SharedString,
    short: SharedString,
    group: Option<SharedString>,
    action: SlashAction,
}

/// [`items`] and then the app's, flattened.
fn entries(app: Vec<SlashItem>) -> Vec<Entry> {
    let grouped = |group: &str, short: SharedString, action| Entry {
        label: format!("{group} ({short})").into(),
        short,
        group: Some(SharedString::from(group.to_owned())),
        action,
    };
    let mut entries: Vec<Entry> = items()
        .into_iter()
        .map(|(label, kind)| {
            let quoted = label
                .strip_prefix("Quote (")
                .and_then(|rest| rest.strip_suffix(')'))
                .map(|short| SharedString::from(short.to_owned()));
            match quoted {
                Some(short) => grouped("Quote", short, SlashAction::Block(kind)),
                None if matches!(kind, BlockKind::Quote { .. }) => Entry {
                    label: label.clone(),
                    short: label,
                    group: Some("Quote".into()),
                    action: SlashAction::Block(kind),
                },
                None => Entry {
                    label: label.clone(),
                    short: label,
                    group: None,
                    action: SlashAction::Block(kind),
                },
            }
        })
        .collect();
    for item in app {
        match item {
            SlashItem::Row { label, action } => entries.push(Entry {
                label: label.clone(),
                short: label,
                group: None,
                action,
            }),
            SlashItem::Group { label, rows } => entries.extend(
                rows.into_iter()
                    .map(|(short, action)| grouped(&label, short, action)),
            ),
        }
    }
    entries
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

/// A row of the open menu: one entry, or a group of them behind a submenu.
/// Indices are into the menu's entries.
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
    /// Read once at open.
    entries: Vec<Entry>,
    pub cursor: menu::Cursor,
}

impl Slash {
    pub fn open(at: Cursor, app_items: Vec<SlashItem>) -> Self {
        let mut slash = Self {
            at,
            rows: Vec::new(),
            entries: entries(app_items),
            cursor: menu::Cursor::default(),
        };
        slash.refilter("");
        slash
    }

    /// With no query a group sits behind one row where its first entry is; a
    /// query ranks every entry flat.
    pub fn refilter(&mut self, query: &str) {
        self.rows = if query.is_empty() {
            let mut rows: Vec<Row> = Vec::new();
            for (ix, entry) in self.entries.iter().enumerate() {
                let Some(group) = &entry.group else {
                    rows.push(Row::Block(ix));
                    continue;
                };
                match rows
                    .iter_mut()
                    .find(|row| matches!(row, Row::Group(label, _) if label == group))
                {
                    Some(Row::Group(_, held)) => held.push(ix),
                    _ => rows.push(Row::Group(group.clone(), vec![ix])),
                }
            }
            rows
        } else {
            let labels: Vec<SharedString> = self
                .entries
                .iter()
                .map(|entry| entry.label.clone())
                .collect();
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
        self.rows
            .iter()
            .map(|row| match row {
                Row::Block(ix) => Item::action(self.entries[*ix].label.clone()),
                Row::Group(label, group) => Item::submenu(
                    label.clone(),
                    group
                        .iter()
                        .map(|ix| Item::action(self.entries[*ix].short.clone()))
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

    /// What the row at `path` does, and `None` for a group.
    pub fn action_at(&self, path: &[usize]) -> Option<SlashAction> {
        let ix = match (self.rows.get(*path.first()?)?, path.get(1)) {
            (Row::Block(ix), None) => *ix,
            (Row::Group(_, group), Some(row)) => *group.get(*row)?,
            _ => return None,
        };
        self.entries.get(ix).map(|entry| entry.action.clone())
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
