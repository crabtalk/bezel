use markdown::{Align, BlockKind, parse, serialize};

#[test]
fn rows_move_to_their_final_index_and_leave_the_header_fixed() {
    let mut doc = parse("| H |\n| --- |\n| one |\n| two |\n| three |");
    assert!(doc.move_row(0, 1, 3));
    assert_eq!(
        serialize(&doc),
        "| H |\n| --- |\n| two |\n| three |\n| one |"
    );
    assert!(doc.move_row(0, 3, 1));
    let original = doc.clone();
    for (from, to) in [(0, 1), (1, 0), (4, 1), (1, 4), (2, 2)] {
        assert!(!doc.move_row(0, from, to));
        assert_eq!(doc, original);
    }
}

#[test]
fn columns_carry_alignment_header_and_inline_marks() {
    let mut doc = parse("| A | B | C |\n| :--- | :---: | ---: |\n| **one** | two | three |");
    let original = doc.clone();
    assert!(doc.move_column(0, 0, 2));
    let BlockKind::Table {
        align,
        header,
        rows,
    } = &doc.blocks[0].kind
    else {
        panic!()
    };
    assert_eq!(align, &[Align::Center, Align::Right, Align::Left]);
    assert_eq!(header[2].text, "A");
    assert_eq!(rows[0][2].text, "one");
    assert!(!rows[0][2].marks.is_empty());
    assert!(doc.move_column(0, 2, 0));
    assert_eq!(doc, original);
    assert!(!doc.move_column(0, 3, 0));
    assert!(!doc.move_column(0, 0, 3));
    assert!(!doc.move_column(0, 0, 0));
    assert_eq!(doc, original);
}
