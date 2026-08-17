//! Tree view — nested rows with disclosure, indent guides and arrow keys.
//!
//! bezel cannot walk your tree. It has no idea what a node is, and a trait or a
//! callback to find out would be a data model this library does not want to
//! own — so the app flattens its own tree into the rows that are visible right
//! now, which it has to do anyway to render them.
//!
//! That is not a compromise, because **a depth-annotated flat list is a
//! complete navigation model**. Everything a tree does falls out of [`Row`]
//! with no parent pointers and no tree walk: down and up are neighbouring
//! indices, a first child is simply the next row, and a parent is the nearest
//! row above with a smaller depth. [`step`] is that, and nothing else.
//!
//! ```ignore
//! bezel_ui::tree::init(cx);   // once, at startup
//!
//! // Each frame: flatten what is open, paint it, and let `step` answer the keys.
//! let rows = self.flatten();                       // Vec<(Row, label)>
//! tree().children(rows.iter().enumerate().map(|(index, (row, label))| {
//!     tree_row(&theme, row, self.selected == Some(index), self.cursor == index)
//!         .id(("row", index))
//!         .child(label.clone())
//! }))
//! ```
//!
//! Expansion stays with the app because it *is* app data — a file tree's open
//! folders often outlive the window — so [`step`] reports an intent, and the app
//! applies it to the set it owns.

use gpui::{App, KeyBinding, actions, div, prelude::*, px};

use bezel_theme::{Theme, hairline};

use crate::widgets;

/// One visible row: how deep it sits, and whether it is a branch.
///
/// `expanded` is `None` for a leaf — which is a different thing from a closed
/// branch, and the difference is what stops `right` pretending a file can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub depth: usize,
    pub expanded: Option<bool>,
}

impl Row {
    pub fn leaf(depth: usize) -> Self {
        Self {
            depth,
            expanded: None,
        }
    }

    pub fn branch(depth: usize, expanded: bool) -> Self {
        Self {
            depth,
            expanded: Some(expanded),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// What a keypress meant. An intent rather than a mutation: only the app can
/// expand a row, because only the app knows what is under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    To(usize),
    Expand(usize),
    Collapse(usize),
}

/// The row `index` hangs under: the nearest row above it with a smaller depth.
///
/// Nearest, not previous — the row directly above is usually a sibling, and
/// often the last leaf of a sibling's whole subtree. Walking up until the depth
/// actually drops is what skips all of that.
pub fn parent_of(rows: &[Row], index: usize) -> Option<usize> {
    let depth = rows.get(index)?.depth;
    rows[..index]
        .iter()
        .rposition(|candidate| candidate.depth < depth)
}

/// What an arrow key means at `cursor`, or `None` when it means nothing.
///
/// Neither end wraps. A menu wraps because it is a ring of choices; a tree is a
/// document, and arriving back at the top because you pressed down once too
/// often loses your place in it.
pub fn step(rows: &[Row], cursor: usize, direction: Direction) -> Option<Move> {
    let row = rows.get(cursor)?;
    match direction {
        Direction::Up => cursor.checked_sub(1).map(Move::To),
        Direction::Down => (cursor + 1 < rows.len()).then_some(Move::To(cursor + 1)),
        // A closed branch opens; an open one steps into it — and its first child
        // is just the next row, because the list is already in visible order.
        Direction::Right => match row.expanded {
            Some(false) => Some(Move::Expand(cursor)),
            Some(true) => (cursor + 1 < rows.len()).then_some(Move::To(cursor + 1)),
            None => None,
        },
        // The mirror: an open branch closes, everything else goes up a level.
        Direction::Left => match row.expanded {
            Some(true) => Some(Move::Collapse(cursor)),
            _ => parent_of(rows, cursor).map(Move::To),
        },
    }
}

actions!(bezel_tree, [SelectPrevious, SelectNext, Collapse, Expand]);

/// The key context a tree claims.
pub const KEY_CONTEXT: &str = "Tree";

/// Bind the arrows. Call once, alongside [`crate::input::init`].
///
/// The actions are public and the handlers are the app's — like [`crate::focus`]
/// and unlike the menubar, a tree cannot handle them itself, because applying a
/// [`Move`] means touching the app's own expansion set. What bezel does here is
/// name the four chords everyone already agrees on, once.
pub fn init(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", SelectPrevious, ctx),
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("left", Collapse, ctx),
        KeyBinding::new("right", Expand, ctx),
    ]);
}

/// How far one level of nesting indents.
pub const INDENT: f32 = 14.0;
/// Width of the chevron column, kept by leaves as well so their labels line up
/// with their siblings' rather than sliding under them.
const CHEVRON: f32 = 16.0;

/// The container. Rows go in it; scrolling is the caller's, via
/// [`crate::scroll`].
pub fn tree() -> gpui::Div {
    div().flex().flex_col().w_full()
}

/// One row: its guides, its chevron, and then whatever the caller puts in it.
///
/// `selected` is what the app considers chosen; `cursor` is where the keyboard
/// is. Two tones, the same pair [`crate::popover::menu_row_nav`] uses, so a tree
/// and a menu never look like two different products.
pub fn tree_row(theme: &Theme, row: &Row, selected: bool, cursor: bool) -> gpui::Div {
    let mut frame = div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .py(px(3.0))
        .pr(px(8.0))
        .text_size(px(12.5))
        .cursor_pointer();
    frame = if selected {
        frame
            .bg(bezel_theme::card_selected_bg())
            .text_color(theme.text)
    } else if cursor {
        frame.bg(bezel_theme::wash(0.05)).text_color(theme.text)
    } else {
        frame.text_color(theme.text_muted)
    };
    frame
        // One segment per ancestor level, drawn by the row it passes through:
        // the line is continuous down the page without any element having to
        // span rows or know its neighbours.
        .children((0..row.depth).map(|_| {
            div()
                .flex_none()
                .w(px(INDENT))
                .h(px(18.0))
                .border_l_1()
                .border_color(hairline(0.08))
        }))
        .child(
            div()
                .flex_none()
                .w(px(CHEVRON))
                .flex()
                .items_center()
                .justify_center()
                .when_some(row.expanded, |slot, expanded| {
                    slot.child(widgets::disclosure(theme, expanded))
                }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ```text
    /// 0  src/            branch, open
    /// 1    ui/           branch, open
    /// 2      table.rs    leaf
    /// 3      tree.rs     leaf
    /// 4    theme/        branch, closed
    /// 5  README.md       leaf
    /// ```
    fn rows() -> Vec<Row> {
        vec![
            Row::branch(0, true),
            Row::branch(1, true),
            Row::leaf(2),
            Row::leaf(2),
            Row::branch(1, false),
            Row::leaf(0),
        ]
    }

    #[test]
    fn a_parent_is_the_nearest_shallower_row_not_the_previous_one() {
        // `theme/` hangs under `src/`, four rows up — the rows between are a
        // sibling and that sibling's whole subtree.
        assert_eq!(parent_of(&rows(), 4), Some(0));
        assert_eq!(parent_of(&rows(), 3), Some(1), "a leaf under ui/");
        assert_eq!(parent_of(&rows(), 1), Some(0));
    }

    #[test]
    fn a_root_row_has_no_parent() {
        assert_eq!(parent_of(&rows(), 0), None);
        assert_eq!(parent_of(&rows(), 5), None, "the second root");
        assert_eq!(parent_of(&rows(), 99), None, "off the end");
    }

    #[test]
    fn right_opens_a_closed_branch_and_enters_an_open_one() {
        let rows = rows();
        assert_eq!(step(&rows, 4, Direction::Right), Some(Move::Expand(4)));
        // Open already: step into it, which is simply the next row.
        assert_eq!(step(&rows, 0, Direction::Right), Some(Move::To(1)));
        // A file has nothing to open and nothing to step into.
        assert_eq!(step(&rows, 2, Direction::Right), None);
    }

    #[test]
    fn left_closes_an_open_branch_and_otherwise_goes_up_a_level() {
        let rows = rows();
        assert_eq!(step(&rows, 1, Direction::Left), Some(Move::Collapse(1)));
        // A leaf leaves, for its parent.
        assert_eq!(step(&rows, 3, Direction::Left), Some(Move::To(1)));
        // A closed branch is not collapsed twice — it leaves too.
        assert_eq!(step(&rows, 4, Direction::Left), Some(Move::To(0)));
        // An open branch closes even at the root: closing comes first, and only
        // a row with nothing left to close looks for a parent.
        assert_eq!(step(&rows, 0, Direction::Left), Some(Move::Collapse(0)));
        // A root row with nothing to close has nowhere to go.
        assert_eq!(step(&rows, 5, Direction::Left), None);
    }

    #[test]
    fn neither_end_wraps() {
        let rows = rows();
        assert_eq!(step(&rows, 0, Direction::Up), None);
        assert_eq!(step(&rows, 5, Direction::Down), None);
        assert_eq!(step(&rows, 2, Direction::Up), Some(Move::To(1)));
        assert_eq!(step(&rows, 2, Direction::Down), Some(Move::To(3)));
    }

    #[test]
    fn an_empty_tree_answers_nothing_rather_than_panicking() {
        for direction in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            assert_eq!(step(&[], 0, direction), None);
            assert_eq!(step(&rows(), 99, direction), None, "cursor off the end");
        }
    }
}
