//! The mention menu: entries an app offers, linked inline as chips.
//!
//! The app installs a [`MentionSource`] per trigger character with
//! [`crate::AppExt::set_mention_source`]; with none installed no character
//! opens a menu. The editor keeps focus while it is open and the query is the
//! text typed after the trigger, the way the slash menu works. Picking a row
//! replaces the trigger and query with a [`markdown::Mark::Mention`] chip
//! linking its URL.

use gpui::{App, Global, SharedString};
use icons::Icon;
use ui::menu::{self, Item};

use markdown::Cursor;

/// One row of the mention menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub icon: Option<Icon>,
    pub label: SharedString,
    pub description: Option<SharedString>,
    /// What the chip links.
    pub url: String,
}

/// The rows for what has been typed after the trigger, best first. Asked at
/// paint.
pub type MentionSource = fn(query: &str, cx: &App) -> Vec<Mention>;

/// One source per trigger character.
#[derive(Default)]
pub(crate) struct Installed(pub Vec<(char, MentionSource)>);

impl Global for Installed {}

impl Installed {
    pub(crate) fn source(cx: &App, trigger: &str) -> Option<(char, MentionSource)> {
        let mut chars = trigger.chars();
        let trigger = chars.next().filter(|_| chars.next().is_none())?;
        cx.try_global::<Self>()?
            .0
            .iter()
            .find(|(installed, _)| *installed == trigger)
            .copied()
    }
}

/// An open menu: where the trigger sits, what was typed after it, and the rows
/// the source last gave for that.
pub struct MentionMenu {
    /// The trigger itself. Backspacing onto it closes the menu.
    pub at: Cursor,
    pub trigger: char,
    source: MentionSource,
    query: String,
    /// The query `rows` answer, `None` until they are read.
    asked: Option<String>,
    rows: Vec<Mention>,
    pub cursor: menu::Cursor,
}

impl MentionMenu {
    pub fn open(at: Cursor, (trigger, source): (char, MentionSource)) -> Self {
        Self {
            at,
            trigger,
            source,
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
        self.rows = (self.source)(&self.query, cx);
        self.asked = Some(self.query.clone());
        self.cursor.clear();
        self.cursor.step(&self.menu(), 1);
    }

    /// The rows as [`ui::menu::card`] paints them.
    pub fn menu(&self) -> Vec<Item> {
        self.rows
            .iter()
            .map(|row| {
                let mut item = Item::action(row.label.clone());
                if let Some(icon) = &row.icon {
                    item = item.with_icon(icon.clone());
                }
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
