//! iTerm2 inline images.

mod common;

use common::*;
use terminal::{
    emulator::Emulator,
    scanner::{Iterm, Scanner, Segment},
};

/// A `width` by `height` PNG, all red.
fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([0xff, 0, 0, 0xff]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

fn iterm(args: &str, file: &[u8]) -> Vec<u8> {
    format!("\x1b]1337;File={args}:{}\x07", base64(file)).into_bytes()
}

#[test]
fn the_scanner_takes_a_files_payload_and_leaves_the_parser_its_arguments() {
    let mut scanner = Scanner::new();
    let input = iterm("inline=1", b"abc");
    assert_eq!(
        passed(&mut scanner, &input),
        b"\x1b]1337;File=inline=1:\x07"
    );
    let mut scanner = Scanner::new();
    let files: Vec<_> = scanner
        .feed(&input)
        .into_iter()
        .filter_map(|segment| match segment {
            Segment::Iterm(command) => Some(command),
            _ => None,
        })
        .collect();
    assert_eq!(
        files,
        vec![Iterm::File {
            args: b"inline=1".to_vec(),
            payload: base64(b"abc").into_bytes()
        }]
    );
}

#[test]
fn another_osc_still_reaches_the_parser() {
    let mut emulator = Emulator::new(20, 5);
    emulator.feed(b"\x1b]0;a title\x07\x1b]1337;SetMark\x07");
    assert_eq!(emulator.title(), Some("a title"));
}

#[test]
fn an_inline_file_lands_at_the_cursor() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(b"$ \r\n");
    emulator.feed(&iterm("name=eA==;inline=1", &png(25, 40)));
    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    assert_eq!((placements[0].row, placements[0].col), (1, 0));
    assert_eq!((placements[0].cols, placements[0].rows), (3, 2));
    assert_eq!(emulator.cursor().map(|c| c.row), Some(3));
}

#[test]
fn a_file_that_is_not_inline_is_not_shown() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&iterm("name=eA==", &png(10, 20)));
    emulator.feed(&iterm("inline=0", &png(10, 20)));
    assert!(emulator.placements().is_empty());
    assert!(emulator.graphics().is_empty());
}

#[test]
fn width_and_height_come_in_cells_pixels_and_percent() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&iterm("inline=1;width=4;height=50%", &png(10, 20)));
    let placement = emulator.placements()[0];
    assert_eq!((placement.cols, placement.rows), (4, 5));

    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&iterm("inline=1;width=35px;height=auto", &png(10, 20)));
    // 35px is four 10px columns; the height follows the aspect.
    assert_eq!(emulator.placements()[0].cols, 4);
}

#[test]
fn preserve_aspect_ratio_off_fills_the_box() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&iterm(
        "inline=1;width=4;height=1;preserveAspectRatio=0",
        &png(10, 20),
    ));
    let frame = emulator.placements()[0].frame;
    assert_eq!(
        (frame.x, frame.y, frame.width, frame.height),
        (0.0, 0.0, 4.0, 1.0)
    );

    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&iterm("inline=1;width=4;height=1", &png(10, 20)));
    let frame = emulator.placements()[0].frame;
    // Kept: one cell wide, centred in the four.
    assert_eq!((frame.x, frame.width), (1.5, 1.0));
}

#[test]
fn a_file_sent_in_parts_is_shown_at_the_end() {
    let mut emulator = placed_emulator(20, 10);
    let file = png(10, 20);
    let (head, tail) = file.split_at(file.len() / 2);
    emulator.feed(b"\x1b]1337;MultipartFile=inline=1\x07");
    // Each part base64 on its own, padding and all.
    emulator.feed(format!("\x1b]1337;FilePart={}\x07", base64(head)).as_bytes());
    emulator.feed(format!("\x1b]1337;FilePart={}\x1b\\", base64(tail)).as_bytes());
    assert!(emulator.placements().is_empty(), "shown before the end");
    emulator.feed(b"\x1b]1337;FileEnd\x07");
    assert_eq!(emulator.placements().len(), 1);
}

#[test]
fn a_file_split_across_reads_is_one_file() {
    let mut emulator = placed_emulator(20, 10);
    let bytes = iterm("inline=1", &png(10, 20));
    for piece in bytes.chunks(7) {
        emulator.feed(piece);
    }
    assert_eq!(emulator.placements().len(), 1);
    assert_eq!(emulator.row_text(0), "");
}

#[test]
fn an_animated_gif_plays() {
    use image::{Delay, Frame, codecs::gif::GifEncoder};
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        for color in [[0xffu8, 0, 0, 0xff], [0, 0, 0xff, 0xff]] {
            let buffer = image::RgbaImage::from_pixel(2, 2, image::Rgba(color));
            encoder
                .encode_frame(Frame::from_parts(
                    buffer,
                    0,
                    0,
                    Delay::from_numer_denom_ms(50, 1),
                ))
                .unwrap();
        }
    }
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&iterm("inline=1", &bytes));
    let placement = emulator.placements()[0];
    let image = emulator.graphics().get(placement.image).unwrap();
    assert_eq!(image.frame_count(), 2);
    assert_eq!(image.gaps, vec![50, 50]);
    assert_eq!(
        image.animation.state,
        terminal::kitty::AnimationState::Running
    );
}
