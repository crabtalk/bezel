use ui::tree::*;

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
