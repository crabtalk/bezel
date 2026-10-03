//! The slash menu: `/` at an empty block, then the rows the app installed with
//! [`crate::AppExt::set_slash_items`], or [`defaults`] until it does.
//!
//! The editor keeps focus while the menu is open and the query is the text
//! typed after the `/`, which is how Notion does it and why there is no second
//! field to hand focus to.

use std::rc::Rc;

use gpui::{App, Global, SharedString, WeakEntity, Window};
use ui::{
    icons::{Icon, glyph},
    menu::{self, Item},
    popover::filter_indices,
};

use markdown::{Align, BlockKind, Cursor, QuoteKind, Text};

/// Every block a block can be turned into, and what each is called.
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

/// One row of the slash menu.
#[derive(Clone)]
pub struct SlashRow {
    pub label: SharedString,
    pub icon: Option<Icon>,
    pub action: SlashAction,
}

/// An item of the slash menu.
#[derive(Clone)]
pub enum SlashItem {
    Row(SlashRow),
    /// Rows behind a submenu. A query matches them as `Group (row)`; several
    /// matches stay behind the submenu, and a lone one stands as its own row.
    Group {
        label: SharedString,
        icon: Option<Icon>,
        rows: Vec<SlashRow>,
    },
}

/// The editor's blocks as slash items, each under its glyph, the quotes and
/// callouts behind one `Quote` group.
pub fn defaults() -> Vec<SlashItem> {
    let mut defaults = Vec::new();
    for (label, kind) in items() {
        let row = SlashRow {
            label,
            icon: Some(glyph_of(&kind)),
            action: SlashAction::Block(kind.clone()),
        };
        if !matches!(kind, BlockKind::Quote { .. }) {
            defaults.push(SlashItem::Row(row));
            continue;
        }
        let row = match row.label.strip_prefix("Quote (") {
            Some(rest) => SlashRow {
                label: rest.trim_end_matches(')').to_owned().into(),
                ..row
            },
            None => row,
        };
        match defaults.last_mut() {
            Some(SlashItem::Group { rows, .. }) => rows.push(row),
            _ => defaults.push(SlashItem::Group {
                label: "Quote".into(),
                icon: Some(glyph::TextQuote.into()),
                rows: vec![row],
            }),
        }
    }
    defaults
}

/// The glyph a block's slash row carries.
fn glyph_of(kind: &BlockKind) -> Icon {
    match kind {
        BlockKind::Paragraph(_) => glyph::Pilcrow.into(),
        BlockKind::Heading { level: 1, .. } => glyph::Heading1.into(),
        BlockKind::Heading { level: 2, .. } => glyph::Heading2.into(),
        BlockKind::Heading { .. } => glyph::Heading3.into(),
        BlockKind::Bullet(_) => glyph::List.into(),
        BlockKind::Ordered { .. } => glyph::ListOrdered.into(),
        BlockKind::Task { .. } => glyph::ListTodo.into(),
        BlockKind::Quote { kind, .. } => match kind {
            None => glyph::TextQuote.into(),
            Some(QuoteKind::Note) => glyph::Info.into(),
            Some(QuoteKind::Tip) => glyph::Lightbulb.into(),
            Some(QuoteKind::Important) => glyph::MessageSquareWarning.into(),
            Some(QuoteKind::Warning) => glyph::TriangleAlert.into(),
            Some(QuoteKind::Caution) => glyph::OctagonAlert.into(),
        },
        BlockKind::Code { .. } => glyph::SquareCode.into(),
        BlockKind::Table { .. } => glyph::Table.into(),
        BlockKind::Image { .. } => glyph::Image.into(),
        _ => glyph::SeparatorHorizontal.into(),
    }
}

/// What the app installed.
pub(crate) struct Installed(pub Vec<SlashItem>);

impl Global for Installed {}

/// The items the app installed, or [`defaults`].
pub(crate) fn installed(cx: &App) -> Vec<SlashItem> {
    cx.try_global::<Installed>()
        .map_or_else(defaults, |Installed(items)| items.clone())
}

/// One row the open menu can offer: what a query matches, what a submenu
/// shows, the group it sits in and that group's glyph, and what picking it
/// does.
#[derive(Clone)]
struct Entry {
    label: SharedString,
    short: SharedString,
    group: Option<(SharedString, Option<Icon>)>,
    icon: Option<Icon>,
    action: SlashAction,
}

/// The installed items, flattened.
fn entries(items: Vec<SlashItem>) -> Vec<Entry> {
    let mut entries = Vec::new();
    for item in items {
        match item {
            SlashItem::Row(row) => entries.push(Entry {
                label: row.label.clone(),
                short: row.label,
                group: None,
                icon: row.icon,
                action: row.action,
            }),
            SlashItem::Group { label, icon, rows } => {
                entries.extend(rows.into_iter().map(|row| Entry {
                    label: match row.label == label {
                        true => label.clone(),
                        false => format!("{label} ({})", row.label).into(),
                    },
                    short: row.label,
                    group: Some((label.clone(), icon.clone())),
                    icon: row.icon,
                    action: row.action,
                }))
            }
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
    Group(SharedString, Option<Icon>, Vec<usize>),
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
    pub fn open(at: Cursor, items: Vec<SlashItem>) -> Self {
        let mut slash = Self {
            at,
            rows: Vec::new(),
            entries: entries(items),
            cursor: menu::Cursor::default(),
        };
        slash.refilter("");
        slash
    }

    /// A group sits behind one row where its first entry is. A query ranks
    /// the entries, and a group's row stands where its best match does,
    /// holding the matches in rank order — or, holding one, is that row.
    pub fn refilter(&mut self, query: &str) {
        let order: Vec<usize> = if query.is_empty() {
            (0..self.entries.len()).collect()
        } else {
            let labels: Vec<SharedString> = self
                .entries
                .iter()
                .map(|entry| entry.label.clone())
                .collect();
            filter_indices(query, &labels)
        };
        let mut rows: Vec<Row> = Vec::new();
        for ix in order {
            let Some((group, icon)) = &self.entries[ix].group else {
                rows.push(Row::Block(ix));
                continue;
            };
            match rows
                .iter_mut()
                .find(|row| matches!(row, Row::Group(label, ..) if label == group))
            {
                Some(Row::Group(_, _, held)) => held.push(ix),
                _ => rows.push(Row::Group(group.clone(), icon.clone(), vec![ix])),
            }
        }
        self.rows = rows
            .into_iter()
            .map(|row| match row {
                Row::Group(_, _, held) if !query.is_empty() && held.len() == 1 => {
                    Row::Block(held[0])
                }
                row => row,
            })
            .collect();
        self.cursor.clear();
        self.cursor.step(&self.menu(), 1);
    }

    /// The rows as [`ui::menu::card`] paints them.
    pub fn menu(&self) -> Vec<Item> {
        self.rows
            .iter()
            .map(|row| match row {
                Row::Block(ix) => self.item(*ix, &self.entries[*ix].label),
                Row::Group(label, icon, group) => {
                    let submenu = Item::submenu(
                        label.clone(),
                        group
                            .iter()
                            .map(|ix| self.item(*ix, &self.entries[*ix].short))
                            .collect(),
                    );
                    match icon.clone() {
                        Some(icon) => submenu.with_icon(icon),
                        None => submenu,
                    }
                }
            })
            .collect()
    }

    fn item(&self, ix: usize, label: &SharedString) -> Item {
        let item = Item::action(label.clone());
        match self.entries[ix].icon.clone() {
            Some(icon) => item.with_icon(icon),
            None => item,
        }
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
            (Row::Group(_, _, group), Some(row)) => *group.get(*row)?,
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
