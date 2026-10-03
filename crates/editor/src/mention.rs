//! The `@` menu: entries an app offers, linked inline as chips.
//!
//! The editor keeps focus while it is open and the query is the text typed
//! after the `@`, the way the slash menu works. Rows come from the
//! [`MentionSource`] the app installs with
//! [`crate::AppExt::set_mention_source`]; picking one replaces `@query` with a
//! [`markdown::Mark::Mention`] chip linking its URL.

use gpui::{App, Global, SharedString};
use ui::menu::{self, Item};

use markdown::Cursor;

/// One row of the `@` menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub label: SharedString,
    pub description: Option<SharedString>,
    /// What the chip links.
    pub url: String,
}

/// The rows for what has been typed after the `@`, best first. Asked at paint.
pub type MentionSource = fn(query: &str, cx: &App) -> Vec<Mention>;

pub(crate) struct Installed(pub MentionSource);

impl Global for Installed {}

/// An open menu: where the `@` sits, what was typed after it, and the rows
/// the source last gave for that.
pub struct MentionMenu {
    /// The `@` itself. Backspacing onto it closes the menu.
    pub at: Cursor,
    query: String,
    /// The query `rows` answer, `None` until they are read.
    asked: Option<String>,
    rows: Vec<Mention>,
    pub cursor: menu::Cursor,
}

impl MentionMenu {
    pub fn open(at: Cursor) -> Self {
        Self {
            at,
            query: String::new(),
            asked: None,
            rows: Vec::new(),
            cursor: menu::Cursor::default(),
        }
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
    }

    /// Ask the source again when the query moved since it last answered.
    pub fn refresh(&mut self, cx: &App) {
        if self.asked.as_ref() == Some(&self.query) {
            return;
        }
        self.rows = cx
            .try_global::<Installed>()
            .map(|Installed(source)| source(&self.query, cx))
            .unwrap_or_default();
        self.asked = Some(self.query.clone());
        self.cursor.clear();
        self.cursor.step(&self.menu(), 1);
    }

    /// The rows as [`ui::menu::card`] paints them.
    pub fn menu(&self) -> Vec<Item> {
        self.rows
            .iter()
            .map(|row| {
                let item = Item::action(row.label.clone());
                match &row.description {
                    Some(description) => item.with_description(description.clone()),
                    None => item,
                }
            })
            .collect()
    }

    pub fn step(&mut self, delta: isize) {
        let menu = self.menu();
        self.cursor.step(&menu, delta);
    }

    /// The row at `ix`, or the live one.
    pub fn row(&self, ix: Option<usize>) -> Option<&Mention> {
        self.rows.get(ix.or(self.cursor.row())?)
    }
}
