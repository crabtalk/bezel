use alacritty_terminal::index::{Column, Line, Point};

use terminal::emulator::*;

fn emu(cols: u16, rows: u16) -> Emulator {
    Emulator::new(cols, rows)
}

#[test]
fn plain_text_lands_on_row_zero() {
    let mut e = emu(20, 5);
    e.feed(b"hello");
    assert_eq!(e.row_text(0), "hello");
    assert_eq!(e.cursor(), Some(CursorSnapshot { row: 0, col: 5 }));
}

#[test]
fn crlf_moves_lines_and_cr_returns_to_column_zero() {
    let mut e = emu(20, 5);
    e.feed(b"one\r\ntwo\r\nthree");
    assert_eq!(e.row_text(0), "one");
    assert_eq!(e.row_text(1), "two");
    assert_eq!(e.row_text(2), "three");
    e.feed(b"\rXX");
    assert_eq!(e.row_text(2), "XXree");
}

#[test]
fn long_line_wraps_at_the_grid_width() {
    let mut e = emu(10, 4);
    e.feed(b"abcdefghijKLM");
    assert_eq!(e.row_text(0), "abcdefghij");
    assert_eq!(e.row_text(1), "KLM");
}

#[test]
fn sgr_colors_and_attributes() {
    let mut e = emu(40, 4);
    e.feed(b"\x1b[31mred\x1b[0m plain \x1b[1;44mboldbg\x1b[0m");
    let line = e.line(0);
    assert_eq!(line[0].fg, CellColor::Indexed(1));
    assert_eq!(line[0].bg, CellColor::Background);
    // After reset: defaults.
    assert_eq!(line[4].fg, CellColor::Foreground);
    // Bold + blue background segment starts at col 10 ("red plain " = 10).
    let bold_cell = line[10];
    assert!(bold_cell.bold);
    assert_eq!(bold_cell.bg, CellColor::Indexed(4));
}

#[test]
fn bright_256_and_truecolor_sgr() {
    let mut e = emu(40, 2);
    e.feed(b"\x1b[95mA\x1b[38;5;196mB\x1b[38;2;10;20;30mC");
    let line = e.line(0);
    assert_eq!(line[0].fg, CellColor::Indexed(13)); // bright magenta
    assert_eq!(line[1].fg, CellColor::Indexed(196));
    assert_eq!(line[2].fg, CellColor::Rgb(10, 20, 30));
}

#[test]
fn inverse_and_hidden_resolve_in_display_colors() {
    let mut e = emu(10, 2);
    e.feed(b"\x1b[7mI\x1b[0m\x1b[8mH");
    let inv = e.line(0)[0];
    assert!(inv.inverse);
    assert_eq!(
        inv.display_colors(),
        (CellColor::Background, CellColor::Foreground)
    );
    let hid = e.line(0)[1];
    assert!(hid.hidden);
    let (fg, bg) = hid.display_colors();
    assert_eq!(fg, bg, "hidden text paints foreground as background");
}

#[test]
fn cursor_addressing_and_relative_moves() {
    let mut e = emu(20, 6);
    e.feed(b"\x1b[3;5Hx");
    // CSI H is 1-based; cell written at row 2, col 4; cursor advanced by 1.
    assert_eq!(e.line(2)[4].ch, 'x');
    assert_eq!(e.cursor(), Some(CursorSnapshot { row: 2, col: 5 }));
    e.feed(b"\x1b[2D"); // left twice
    assert_eq!(e.cursor(), Some(CursorSnapshot { row: 2, col: 3 }));
    e.feed(b"\x1b[A"); // up
    assert_eq!(e.cursor(), Some(CursorSnapshot { row: 1, col: 3 }));
}

#[test]
fn clear_screen_and_home() {
    let mut e = emu(20, 4);
    e.feed(b"aaa\r\nbbb\r\nccc");
    e.feed(b"\x1b[2J\x1b[H");
    for row in 0..4 {
        assert_eq!(e.row_text(row), "");
    }
    assert_eq!(e.cursor(), Some(CursorSnapshot { row: 0, col: 0 }));
    e.feed(b"fresh");
    assert_eq!(e.row_text(0), "fresh");
}

#[test]
fn erase_line_variants() {
    let mut e = emu(20, 2);
    e.feed(b"abcdef\x1b[3D\x1b[K"); // erase from cursor (col 3) to end
    assert_eq!(e.row_text(0), "abc");
}

#[test]
fn scrollback_history_and_scrolling() {
    let mut e = emu(10, 3);
    for i in 1..=8 {
        e.feed(format!("line{i}\r\n").as_bytes());
    }
    // Viewport shows the tail (line7, line8, then the blank prompt row).
    assert_eq!(e.row_text(0), "line7");
    assert_eq!(e.history_lines(), 6);
    assert_eq!(e.display_offset(), 0);
    // Scroll up into history.
    e.scroll(2);
    assert_eq!(e.display_offset(), 2);
    assert_eq!(e.row_text(0), "line5");
    // Cursor is below the viewport while scrolled back.
    assert_eq!(e.cursor(), None);
    // Over-scroll clamps to the top of history.
    e.scroll(100);
    assert_eq!(e.display_offset(), 6);
    assert_eq!(e.row_text(0), "line1");
    e.scroll_to_bottom();
    assert_eq!(e.display_offset(), 0);
    assert_eq!(e.row_text(0), "line7");
}

#[test]
fn alt_screen_restores_primary_content() {
    let mut e = emu(20, 4);
    e.feed(b"primary");
    // Enter the alt screen; 1049 keeps the cursor position, so home first.
    e.feed(b"\x1b[?1049h\x1b[H");
    e.feed(b"alt-content");
    assert_eq!(e.row_text(0), "alt-content");
    e.feed(b"\x1b[?1049l"); // leave
    assert_eq!(e.row_text(0), "primary");
}

#[test]
fn dsr_cursor_report_produces_pty_response() {
    let mut e = emu(20, 4);
    e.feed(b"\x1b[2;3H");
    let responses = e.feed(b"\x1b[6n");
    assert_eq!(String::from_utf8_lossy(&responses), "\x1b[2;3R");
}

#[test]
fn osc_title_and_bell() {
    let mut e = emu(20, 2);
    assert_eq!(e.title(), None);
    e.feed(b"\x1b]0;my title\x07");
    assert_eq!(e.title(), Some("my title"));
    assert!(!e.take_bell());
    e.feed(b"\x07");
    assert!(e.take_bell());
    assert!(!e.take_bell(), "bell reads clear it");
}

#[test]
fn app_cursor_and_bracketed_paste_modes_toggle() {
    let mut e = emu(10, 2);
    assert!(!e.app_cursor_mode());
    e.feed(b"\x1b[?1h");
    assert!(e.app_cursor_mode());
    e.feed(b"\x1b[?1l");
    assert!(!e.app_cursor_mode());
    e.feed(b"\x1b[?2004h");
    assert!(e.bracketed_paste_mode());
}

#[test]
fn hidden_cursor_mode() {
    let mut e = emu(10, 2);
    e.feed(b"\x1b[?25l");
    assert_eq!(e.cursor(), None);
    e.feed(b"\x1b[?25h");
    assert!(e.cursor().is_some());
}

#[test]
fn resize_preserves_content_and_reflows_cursor() {
    let mut e = emu(20, 5);
    e.feed(b"keepme\r\nsecond");
    e.resize(30, 3);
    assert_eq!(e.cols(), 30);
    assert_eq!(e.rows(), 3);
    assert_eq!(e.row_text(0), "keepme");
    assert_eq!(e.row_text(1), "second");
}

#[test]
fn wide_chars_occupy_two_cells_with_spacer() {
    let mut e = emu(10, 2);
    e.feed("宽w".as_bytes());
    let line = e.line(0);
    assert!(line[0].wide);
    assert_eq!(line[0].ch, '宽');
    assert!(line[1].wide_spacer);
    assert_eq!(line[2].ch, 'w');
    assert_eq!(e.row_text(0), "宽w");
    assert_eq!(e.cursor(), Some(CursorSnapshot { row: 0, col: 3 }));
}

/// Viewport row → grid line, which is the translation every selection
/// anchor goes through. Unscrolled they coincide; scrolled back, the same
/// viewport row names a line further up history.
#[test]
fn grid_point_offsets_by_the_scrollback_position() {
    let mut e = emu(10, 3);
    for i in 1..=8 {
        e.feed(format!("line{i}\r\n").as_bytes());
    }
    assert_eq!(e.grid_point(0, 2), Point::new(Line(0), Column(2)));
    e.scroll(4);
    assert_eq!(e.grid_point(0, 2), Point::new(Line(-4), Column(2)));
    // Columns clamp into the grid so an over-wide pointer cannot anchor
    // outside it.
    assert_eq!(e.grid_point(0, 99).column, Column(9));
}

#[test]
fn simple_selection_yields_its_text_and_marks_its_cells() {
    let mut e = emu(20, 3);
    e.feed(b"hello world");
    assert!(!e.has_selection());
    assert_eq!(e.selection_text(), None);

    // Drag across "hello".
    e.start_selection(SelectionType::Simple, e.grid_point(0, 0), Side::Left);
    e.update_selection(e.grid_point(0, 4), Side::Right);
    assert!(e.has_selection());
    assert_eq!(e.selection_text().as_deref(), Some("hello"));

    let line = e.line(0);
    assert!(line[..5].iter().all(|c| c.selected));
    assert!(!line[5].selected, "the space past the drag is not selected");

    e.clear_selection();
    assert!(!e.has_selection());
    assert!(e.line(0).iter().all(|c| !c.selected));
}

/// Double-click granularity: the anchor expands to the whole word without
/// the caller computing any boundaries.
#[test]
fn semantic_selection_expands_to_the_word() {
    let mut e = emu(30, 2);
    e.feed(b"alpha beta gamma");
    e.start_selection(SelectionType::Semantic, e.grid_point(0, 7), Side::Left);
    assert_eq!(e.selection_text().as_deref(), Some("beta"));
}

/// Triple-click granularity. The trailing newline is part of the copy —
/// pasting a line-selection should reproduce the line break, the way it
/// does in every other terminal.
#[test]
fn line_selection_takes_the_whole_row() {
    let mut e = emu(30, 3);
    e.feed(b"first row\r\nsecond row");
    e.start_selection(SelectionType::Lines, e.grid_point(1, 3), Side::Left);
    assert_eq!(e.selection_text().as_deref(), Some("second row\n"));
}

/// A selection made across a line break keeps the newline, so pasting the
/// copy reproduces the rows.
#[test]
fn selection_spans_rows_with_a_newline() {
    let mut e = emu(10, 3);
    e.feed(b"ab\r\ncd");
    e.start_selection(SelectionType::Simple, e.grid_point(0, 0), Side::Left);
    e.update_selection(e.grid_point(1, 1), Side::Right);
    assert_eq!(e.selection_text().as_deref(), Some("ab\ncd"));
}

/// The reason anchors live in grid space: output that scrolls the grid must
/// carry the selection with its text, not leave it pinned to a screen row.
#[test]
fn selection_follows_its_text_when_output_scrolls() {
    let mut e = emu(10, 3);
    e.feed(b"target\r\n");
    e.start_selection(SelectionType::Simple, e.grid_point(0, 0), Side::Left);
    e.update_selection(e.grid_point(0, 5), Side::Right);
    assert_eq!(e.selection_text().as_deref(), Some("target"));
    // Push it up the screen; the text is unchanged, so the copy is too.
    e.feed(b"a\r\nb\r\nc\r\n");
    assert_eq!(e.selection_text().as_deref(), Some("target"));
}

/// A click with no drag selects nothing, and must not report a selection —
/// otherwise the copy action fires on every bare click.
#[test]
fn a_click_without_a_drag_selects_nothing() {
    let mut e = emu(20, 2);
    e.feed(b"hello");
    e.start_selection(SelectionType::Simple, e.grid_point(0, 2), Side::Left);
    assert_eq!(e.selection_text(), None);
    assert!(!e.has_selection());
}

#[test]
fn utf8_split_across_feeds_reassembles() {
    let mut e = emu(10, 2);
    let bytes = "é".as_bytes();
    e.feed(&bytes[..1]);
    e.feed(&bytes[1..]);
    assert_eq!(e.row_text(0), "é");
}
