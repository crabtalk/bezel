#[cfg(debug_assertions)] // the only user is the should_panic test below
use gpui::{IntoElement, div};

use ui::table::*;

#[test]
fn the_first_click_sorts_ascending() {
    assert_eq!(
        next_sort(None, 2),
        Sort {
            column: 2,
            ascending: true
        }
    );
}

#[test]
fn clicking_the_sorted_column_reverses_it() {
    let ascending = Sort {
        column: 1,
        ascending: true,
    };
    let descending = next_sort(Some(ascending), 1);
    assert_eq!(
        descending,
        Sort {
            column: 1,
            ascending: false
        }
    );
    assert_eq!(next_sort(Some(descending), 1), ascending, "and back again");
}

#[test]
fn another_column_starts_ascending_rather_than_inheriting() {
    // The case a plain toggle gets wrong: moving to a new column while the
    // old one was descending would sort the new one descending, which reads
    // as the table having ignored the click.
    let descending = Sort {
        column: 0,
        ascending: false,
    };
    assert_eq!(
        next_sort(Some(descending), 3),
        Sort {
            column: 3,
            ascending: true
        }
    );
}

#[test]
#[cfg(debug_assertions)] // the guard is a debug_assert; release truncates by design
#[should_panic(expected = "one cell per column")]
fn a_row_short_of_cells_is_caught_rather_than_quietly_misaligned() {
    // Zipping alone would truncate and render a row that looks fine on its
    // own while sitting under the wrong headings — the exact drift this
    // module exists to prevent, so the guard is worth having teeth.
    let theme = theme::Theme::for_appearance(theme::Appearance::Dark);
    let columns = [
        Column::new("Name", Width::Flex(1.0)),
        Column::new("Size", Width::Flex(1.0)),
    ];
    row(
        &theme,
        &columns,
        true,
        false,
        vec![div().into_any_element()],
    );
}

#[test]
fn a_column_is_left_aligned_until_told_otherwise() {
    let column = Column::new("Name", Width::Flex(1.0));
    assert_eq!(column.align, Align::Start);
    assert_eq!(column.align_end().align, Align::End);
}
