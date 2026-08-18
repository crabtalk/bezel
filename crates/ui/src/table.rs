//! Table — for rows that are *tuples*.
//!
//! Most lists of things in this library are records, and a record reads better
//! as a card: [`group_box`](crate::widgets::Scaffolding::group_box) + `card_row` + `row_title` +
//! `meta_line` already does that, and does it better than a table would. Reach
//! for this one only when the third column of every row has to line up, because
//! reading *down* it is the point.
//!
//! Which is also the failure this component exists to prevent. A header and a
//! body that size their own cells drift apart the moment either changes, and
//! nothing catches it — both halves look right on their own. So the columns are
//! declared once and shared:
//!
//! ```ignore
//! const COLUMNS: &[Column] = ..;                     // one declaration
//!
//! table(&theme)
//!     .child(header(&theme).children(COLUMNS.iter().enumerate().map(|(index, column)| {
//!         header_cell(&theme, column, sorted_direction(index))
//!             .id(("column", index))
//!             .on_click(cx.listener(move |view, _, _, cx| view.sort_by(index, cx)))
//!     })))
//!     .children(rows.iter().enumerate().map(|(index, item)| {
//!         row(&theme, COLUMNS, index == 0, false, vec![
//!             item.name.clone().into_any_element(),
//!             item.kind.clone().into_any_element(),
//!         ])
//!     }))
//! ```
//!
//! Sorting is the caller's: [`next_sort`] says what a click on a heading means,
//! the caller sorts its own rows, and this module paints the arrow. Nothing here
//! holds data, so nothing here can hold it out of date.

use gpui::{AnyElement, Pixels, SharedString, div, prelude::*, px, relative};

use theme::{Theme, ink};

use crate::icons;

/// How wide a column is: a fixed measure, or a share of what is left after the
/// fixed ones have taken theirs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
    Fixed(Pixels),
    Flex(f32),
}

/// Which edge a cell's content sits against.
///
/// There is no `Center`, deliberately: in a column of data it is almost always
/// the wrong answer, and offering it is how tables end up with one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
}

/// One column, declared once and handed to both the header and every row.
#[derive(Clone, Debug)]
pub struct Column {
    pub label: SharedString,
    pub width: Width,
    pub align: Align,
}

impl Column {
    pub fn new(label: impl Into<SharedString>, width: Width) -> Self {
        Self {
            label: label.into(),
            width,
            align: Align::Start,
        }
    }

    /// Right-align this column — what a number wants, so its digits line up by
    /// place value rather than by however wide the last one was.
    pub fn align_end(mut self) -> Self {
        self.align = Align::End;
        self
    }
}

/// Which column a table is sorted by, and which way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: usize,
    pub ascending: bool,
}

/// What a click on `column`'s heading means: the sorted column reverses, any
/// other column starts ascending.
///
/// Starting fresh rather than inheriting the previous column's direction is the
/// part worth being deliberate about — carrying it over means clicking a new
/// heading can sort it descending, which reads as the table ignoring the click.
pub fn next_sort(current: Option<Sort>, column: usize) -> Sort {
    match current {
        Some(sort) if sort.column == column => Sort {
            column,
            ascending: !sort.ascending,
        },
        _ => Sort {
            column,
            ascending: true,
        },
    }
}

/// Horizontal padding on every cell, header and body alike — the one number
/// that has to agree for columns to line up.
const CELL_X: f32 = 12.0;

/// The frame. Clipped, so a row's hover wash cannot square off the corners.
pub fn table(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(Theme::PANEL_RADIUS))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
}

/// The heading strip. Fill [`header_cell`]s into it, one per column.
pub fn header(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .bg(ink(0.03))
        .border_b_1()
        .border_color(theme.border)
}

/// One heading. `sorted` carries the direction when this is the sorted column,
/// and `None` when it is not.
///
/// Returns a plain `Div` like the rest of this crate: a table that does not sort
/// simply never adds the `.id`/`.on_click` that would make it.
pub fn header_cell(theme: &Theme, column: &Column, sorted: Option<bool>) -> gpui::Div {
    cell_frame(column)
        .py(px(8.0))
        .gap(px(4.0))
        .text_size(px(11.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(if sorted.is_some() {
            theme.text
        } else {
            theme.text_muted
        })
        .cursor_pointer()
        .child(column.label.clone())
        .when_some(sorted, |cell, ascending| {
            cell.child(
                icons::icon(if ascending {
                    icons::ARROW_UP
                } else {
                    icons::ARROW_DOWN
                })
                .size(px(11.0))
                .text_color(theme.text_muted),
            )
        })
}

/// One body row, its cells zipped onto the columns.
///
/// The zip is the whole point — a cell is never sized where it is written, so a
/// row cannot drift from the header. `cells` shorter than `columns` is a bug in
/// the caller rather than a shape to render, and the assert says so in debug
/// builds; release truncates rather than panicking at a user.
pub fn row(
    theme: &Theme,
    columns: &[Column],
    first: bool,
    selected: bool,
    cells: Vec<AnyElement>,
) -> gpui::Div {
    debug_assert_eq!(
        cells.len(),
        columns.len(),
        "a table row must have one cell per column"
    );
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .when(!first, |row| {
            row.border_t_1().border_color(theme.border.opacity(0.6))
        })
        .text_size(px(12.5))
        .text_color(theme.text);
    row = if selected {
        row.bg(theme::card_selected_bg())
    } else {
        // The same wash `card_row` uses, so a table and a card list read as one
        // system rather than two.
        row.hover(|s| s.bg(ink(0.015)))
    };
    row.children(
        columns
            .iter()
            .zip(cells)
            .map(|(column, content)| cell_frame(column).py(px(9.0)).child(content)),
    )
}

/// Width and alignment from the column, and nothing else — the shared shape
/// that makes a header cell and a body cell land in the same place.
fn cell_frame(column: &Column) -> gpui::Div {
    let cell = div()
        .flex()
        .flex_row()
        .items_center()
        .min_w_0()
        .px(px(CELL_X))
        .when(column.align == Align::End, |cell| cell.justify_end());
    match column.width {
        Width::Fixed(width) => cell.flex_none().w(width),
        // A zero basis, so the share is of the whole remaining space rather than
        // of whatever the content happens to measure.
        Width::Flex(weight) => cell
            .flex_grow(weight)
            .flex_shrink(1.0)
            .flex_basis(relative(0.0)),
    }
}
