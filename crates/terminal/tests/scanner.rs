//! The stream scanner: what it takes off the pty stream, and what it leaves
//! the parser.

mod common;

use common::*;
use terminal::scanner::{Scanner, Segment};

fn commands(scanner: &mut Scanner, bytes: &[u8]) -> usize {
    scanner
        .feed(bytes)
        .iter()
        .filter(|segment| matches!(segment, Segment::Graphics(_)))
        .count()
}

// ---------------------------------------------------------------------------
// Kitty graphics
// ---------------------------------------------------------------------------

#[test]
fn text_either_side_of_a_command_passes_through() {
    let mut scanner = Scanner::new();
    let mut stream = b"before".to_vec();
    stream.extend(apc(&format!("a=t,f=32,s=1,v=1,i=1;{}", base64(&pixel()))));
    stream.extend_from_slice(b"after");
    assert_eq!(passed(&mut scanner, &stream), b"beforeafter");
}

#[test]
fn a_command_split_across_two_reads_is_one_command() {
    let whole = apc(&format!("a=t,f=32,s=1,v=1,i=7;{}", base64(&pixel())));
    // Every split, including the two that land between `ESC` and `_` and
    // between `ESC` and `\` — the boundaries a pty read is as likely to fall
    // on as any other.
    for at in 0..whole.len() {
        let mut scanner = Scanner::new();
        let first = commands(&mut scanner, &whole[..at]);
        let second = commands(&mut scanner, &whole[at..]);
        assert_eq!(
            first + second,
            1,
            "split at {at} produced {first} + {second} commands"
        );
    }
}

#[test]
fn an_escape_held_across_a_read_is_handed_back_whole() {
    let mut scanner = Scanner::new();
    // `ESC` alone, then the sequence it turned out to introduce: the parser
    // has to receive both bytes or the cursor never moves.
    assert_eq!(passed(&mut scanner, b"\x1b"), b"");
    assert_eq!(passed(&mut scanner, b"[H"), b"\x1b[H");
}

#[test]
fn another_apc_sequence_is_swallowed_rather_than_printed() {
    let mut scanner = Scanner::new();
    // Not ours — but it was never going to reach the screen either way, since
    // `vte` discards the run. What matters is that the payload does not.
    let stream = b"a\x1b_NOTGRAPHICS\x1b\\b";
    assert_eq!(passed(&mut scanner, stream), b"ab");
    assert_eq!(commands(&mut Scanner::new(), stream), 0);
}

#[test]
fn a_run_with_no_terminator_does_not_grow_forever() {
    let mut scanner = Scanner::new();
    let mut stream = b"\x1b_Ga=t;".to_vec();
    stream.extend(std::iter::repeat_n(b'A', 1 << 20));
    assert_eq!(commands(&mut scanner, &stream), 0);
    // And the stream recovers at the next terminator rather than eating the
    // rest of the session.
    assert_eq!(passed(&mut scanner, b"\x1b\\back"), b"back");
}

#[test]
fn a_bel_terminates_a_run_as_well_as_st() {
    let mut scanner = Scanner::new();
    let mut stream = format!("\x1b_Ga=t,f=32,s=1,v=1,i=3;{}\x07", base64(&pixel())).into_bytes();
    stream.extend_from_slice(b"tail");
    assert_eq!(commands(&mut Scanner::new(), &stream), 1);
    assert_eq!(passed(&mut scanner, &stream), b"tail");
}

// ---------------------------------------------------------------------------
// Synchronized output and cell size queries
// ---------------------------------------------------------------------------

#[test]
fn the_scanner_reports_mode_2026_and_still_passes_its_bytes_on() {
    let mut scanner = Scanner::new();
    let input = b"a\x1b[?2026hb\x1b[?2026lc";
    let segments = scanner.feed(input);

    let holds: Vec<bool> = segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Sync(hold) => Some(*hold),
            _ => None,
        })
        .collect();
    assert_eq!(holds, vec![true, false]);

    let text: Vec<u8> = segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Text(text) => Some(*text),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    assert_eq!(text, input);
}

#[test]
fn a_mode_2026_run_split_across_reads_is_one_edge() {
    let mut scanner = Scanner::new();
    assert!(!scanner.feed(b"\x1b[?20").iter().any(is_sync));
    assert!(scanner.feed(b"26h").iter().any(is_sync));
}

#[test]
fn a_mode_2026_query_is_not_an_edge() {
    let mut scanner = Scanner::new();
    assert!(!scanner.feed(b"\x1b[?2026$p").iter().any(is_sync));
}

#[test]
fn a_graphics_payload_cannot_finish_a_mode_2026_run() {
    let mut scanner = Scanner::new();
    // The `ESC` opening the APC would otherwise count as the first byte of a
    // BSU, leaving the rest to be completed by whatever follows the run.
    assert!(!scanner.feed(b"\x1b_Ga=q\x1b\\[?2026h").iter().any(is_sync));
}

#[test]
fn the_scanner_reports_a_cell_size_query_and_still_passes_its_bytes_on() {
    let mut scanner = Scanner::new();
    let input = b"a\x1b[16tb";
    let segments = scanner.feed(input);
    assert_eq!(
        segments
            .iter()
            .filter(|s| matches!(s, Segment::CellSizeQuery))
            .count(),
        1
    );
    let text: Vec<u8> = segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Text(text) => Some(*text),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    assert_eq!(text, input);
}

#[test]
fn a_cell_size_query_split_across_reads_is_one_query() {
    let mut scanner = Scanner::new();
    assert!(!scanner.feed(b"\x1b[1").iter().any(is_cell_size_query));
    assert!(scanner.feed(b"6t").iter().any(is_cell_size_query));
}

#[test]
fn other_window_ops_are_not_a_cell_size_query() {
    let mut scanner = Scanner::new();
    assert!(
        !scanner
            .feed(b"\x1b[14t\x1b[18t\x1b[16;1t\x1b[116t")
            .iter()
            .any(is_cell_size_query)
    );
}

fn is_cell_size_query(segment: &Segment<'_>) -> bool {
    matches!(segment, Segment::CellSizeQuery)
}

fn is_sync(segment: &Segment<'_>) -> bool {
    matches!(segment, Segment::Sync(_))
}
