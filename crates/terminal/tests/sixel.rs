//! Sixel images.

mod common;

use common::*;
use terminal::{
    emulator::Emulator,
    scanner::{Scanner, Segment},
};

fn sixel(data: &str) -> Vec<u8> {
    format!("\x1bP0;1;0q{data}\x1b\\").into_bytes()
}

#[test]
fn the_scanner_takes_a_sixel_images_data_and_leaves_the_parser_an_empty_dcs() {
    let mut scanner = Scanner::new();
    let segments = scanner.feed(b"a\x1bP0;1;0q#1~~\x1b\\b");
    let data: Vec<&Vec<u8>> = segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Sixel(data) => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(data, vec![&b"#1~~".to_vec()]);
    let mut scanner = Scanner::new();
    assert_eq!(
        passed(&mut scanner, b"a\x1bP0;1;0q#1~~\x1b\\b"),
        b"a\x1bP0;1;0q\x1b\\b"
    );
}

#[test]
fn another_dcs_passes_through_whole() {
    let mut scanner = Scanner::new();
    let input = b"\x1bP$qm\x1b\\\x1bP+q544e\x1b\\";
    assert_eq!(passed(&mut scanner, input), input);
}

#[test]
fn a_sixel_image_split_across_reads_is_one_image() {
    let mut scanner = Scanner::new();
    let mut images = 0;
    for read in [&b"\x1bP"[..], b"q#1", b"~~\x1b", b"\\"] {
        images += scanner
            .feed(read)
            .iter()
            .filter(|segment| matches!(segment, Segment::Sixel(_)))
            .count();
    }
    assert_eq!(images, 1);
}

#[test]
fn a_cancelled_sixel_image_is_dropped() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(b"\x1bPq#1~~\x18after");
    assert!(emulator.placements().is_empty());
    assert_eq!(emulator.row_text(0), "after");
}

#[test]
fn a_sixel_image_lands_at_the_cursor() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(b"$ \r\n");
    // Red, two columns wide and six rows tall.
    emulator.feed(&sixel("#1;2;100;0;0#1~~"));
    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    assert_eq!((placements[0].row, placements[0].col), (1, 0));
    let image = emulator.graphics().get(placements[0].image).unwrap();
    assert_eq!((image.width, image.height), (2, 6));
    assert_eq!(&image.bytes[..4], &[0xff, 0, 0, 0xff]);
    // One row below the image.
    assert_eq!(emulator.cursor().map(|c| c.row), Some(2));
}

#[test]
fn sixel_commands_draw_where_they_say() {
    let mut emulator = placed_emulator(20, 10);
    // Raster 4x12; a repeat of three; a graphics new line; a carriage return
    // and a second color over the first column.
    emulator.feed(&sixel("\"1;1;4;12#1;2;0;0;100!3@-@$#2;2;0;100;0@"));
    let placement = emulator.placements()[0];
    let image = emulator.graphics().get(placement.image).unwrap();
    assert_eq!((image.width, image.height), (4, 12));
    let pixel = |x: usize, y: usize| image.bytes[4 * (y * 4 + x)..4 * (y * 4 + x) + 4].to_vec();
    // `@` is the low bit: the top row of each band.
    assert_eq!(pixel(2, 0), vec![0, 0, 0xff, 0xff]);
    assert_eq!(pixel(3, 0), vec![0, 0, 0, 0], "the repeat ran long");
    assert_eq!(
        pixel(0, 6),
        vec![0, 0xff, 0, 0xff],
        "the second color did not land"
    );
    assert_eq!(pixel(0, 1), vec![0, 0, 0, 0]);
}

#[test]
fn an_hls_register_turns_the_dec_hue_wheel() {
    let mut emulator = placed_emulator(20, 10);
    // DEC puts red at 120 degrees.
    emulator.feed(&sixel("#1;1;120;50;100#1~"));
    let placement = emulator.placements()[0];
    let image = emulator.graphics().get(placement.image).unwrap();
    assert_eq!(&image.bytes[..4], &[0xff, 0, 0, 0xff]);
}

#[test]
fn the_primary_attributes_advertise_sixel_once_asked_to() {
    let mut emulator = Emulator::new(20, 5);
    assert_eq!(emulator.feed(b"\x1b[c"), b"\x1b[?6c");
    emulator.set_advertise_sixel(true);
    assert_eq!(emulator.feed(b"\x1b[c"), b"\x1b[?62;4;22c");
}
