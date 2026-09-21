//! The kitty graphics protocol under scripted byte strings — the same harness
//! the rest of the emulator is tested with, because the protocol arrives the
//! same way the escapes do.

use terminal::{
    emulator::Emulator,
    kitty::{Format, Scanner, Segment},
};

/// `ESC _ G` … `ESC \` around a body.
fn apc(body: &str) -> Vec<u8> {
    format!("\x1b_G{body}\x1b\\").into_bytes()
}

/// Base64 of `bytes`, standard alphabet with padding — what a client sends.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut bits = 0u32;
        for (ix, &byte) in chunk.iter().enumerate() {
            bits |= (byte as u32) << (16 - 8 * ix);
        }
        for ix in 0..4 {
            if ix <= chunk.len() {
                out.push(ALPHABET[(bits >> (18 - 6 * ix)) as usize & 0x3f] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// One red pixel, RGBA.
fn pixel() -> Vec<u8> {
    vec![0xff, 0x00, 0x00, 0xff]
}

fn commands(scanner: &mut Scanner, bytes: &[u8]) -> usize {
    scanner
        .feed(bytes)
        .iter()
        .filter(|segment| matches!(segment, Segment::Graphics(_)))
        .count()
}

/// Text runs a scanner passed through, joined — what the ANSI parser would
/// have seen.
fn passed(scanner: &mut Scanner, bytes: &[u8]) -> Vec<u8> {
    scanner
        .feed(bytes)
        .iter()
        .filter_map(|segment| match segment {
            Segment::Text(text) => Some(*text),
            Segment::Graphics(_) => None,
        })
        .fold(Vec::new(), |mut out, text| {
            out.extend_from_slice(text);
            out
        })
}

// ---------------------------------------------------------------------------
// The scanner
// ---------------------------------------------------------------------------

#[test]
fn text_either_side_of_a_command_passes_through() {
    let mut scanner = Scanner::new();
    let mut stream = b"before".to_vec();
    stream.extend(apc(&format!(
        "a=t,f=32,s=1,v=1,i=1;{}",
        base64(&pixel())
    )));
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
// Transmission
// ---------------------------------------------------------------------------

#[test]
fn a_transmitted_image_is_held_under_its_id() {
    let mut emulator = Emulator::new(20, 5);
    emulator.feed(&apc(&format!(
        "a=t,f=32,s=1,v=1,i=9;{}",
        base64(&pixel())
    )));
    let image = emulator.graphics().get(9).expect("no image under id 9");
    assert_eq!(image.format, Format::Rgba);
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.bytes, pixel());
}

#[test]
fn chunks_assemble_into_one_image() {
    let mut emulator = Emulator::new(20, 5);
    let pixels: Vec<u8> = (0..16).collect();
    let encoded = base64(&pixels);
    let (first, rest) = encoded.split_at(8);
    let (second, third) = rest.split_at(8);

    emulator.feed(&apc(&format!("a=T,f=32,s=2,v=2,i=4,m=1;{first}")));
    assert!(
        emulator.graphics().is_empty(),
        "an image landed before its last chunk"
    );
    emulator.feed(&apc(&format!("m=1;{second}")));
    emulator.feed(&apc(&format!("m=0;{third}")));

    let image = emulator.graphics().get(4).expect("no assembled image");
    assert_eq!(image.bytes, pixels);
    assert_eq!(emulator.graphics().len(), 1, "a chunk landed on its own");
}

#[test]
fn a_raw_payload_shorter_than_its_dimensions_is_refused() {
    let mut emulator = Emulator::new(20, 5);
    // Four pixels claimed, one sent: the paint that believed it would read
    // past the end of the buffer.
    let reply = emulator.feed(&apc(&format!(
        "a=t,f=32,s=2,v=2,i=5;{}",
        base64(&pixel())
    )));
    assert!(emulator.graphics().get(5).is_none());
    assert_eq!(reply, b"\x1b_Gi=5;EINVAL:dimensions\x1b\\");
}

#[test]
fn a_png_carries_its_own_dimensions() {
    let mut emulator = Emulator::new(20, 5);
    // Not a real PNG: the point is that nothing here checks the pixel count,
    // because the format states it and the decoder is the view's.
    emulator.feed(&apc(&format!("a=t,f=100,i=6;{}", base64(b"\x89PNG..."))));
    let image = emulator.graphics().get(6).expect("no png");
    assert_eq!(image.format, Format::Png);
}

// ---------------------------------------------------------------------------
// Replies
// ---------------------------------------------------------------------------

#[test]
fn a_transmission_is_acknowledged_on_the_pty() {
    let mut emulator = Emulator::new(20, 5);
    let reply = emulator.feed(&apc(&format!(
        "a=t,f=32,s=1,v=1,i=2;{}",
        base64(&pixel())
    )));
    assert_eq!(reply, b"\x1b_Gi=2;OK\x1b\\");
}

#[test]
fn quiet_silences_the_acknowledgement() {
    let mut emulator = Emulator::new(20, 5);
    let reply = emulator.feed(&apc(&format!(
        "a=t,f=32,s=1,v=1,i=2,q=1;{}",
        base64(&pixel())
    )));
    assert!(reply.is_empty(), "q=1 still answered: {reply:?}");
    // …and q=2 silences the failures too.
    let reply = emulator.feed(&apc("a=t,f=32,s=2,v=2,i=3,q=2;AAAA"));
    assert!(reply.is_empty(), "q=2 still answered: {reply:?}");
}

#[test]
fn quiet_asked_for_on_the_first_chunk_holds_to_the_last() {
    let mut emulator = Emulator::new(20, 5);
    let bytes = base64(&pixel());
    let (head, tail) = bytes.split_at(4);
    let reply = emulator.feed(&apc(&format!("a=T,f=32,s=1,v=1,i=9,q=2,m=1;{head}")));
    assert!(reply.is_empty(), "a chunk mid-transmission answered: {reply:?}");
    let reply = emulator.feed(&apc(&format!("m=0;{tail}")));
    assert!(reply.is_empty(), "the closing chunk answered: {reply:?}");
    assert!(emulator.graphics().get(9).is_some(), "the image was dropped");
}

#[test]
fn a_transfer_this_cannot_make_says_so_rather_than_going_quiet() {
    let mut emulator = Emulator::new(20, 5);
    // A file transfer names a path to open, which is a platform and security
    // surface the first cut does not have.
    let reply = emulator.feed(&apc("a=t,f=100,t=f,i=8;L3RtcC9pbWc="));
    assert_eq!(reply, b"\x1b_Gi=8;ENOTSUPPORTED:medium\x1b\\");
    assert!(emulator.graphics().is_empty());
}

#[test]
fn only_the_last_chunk_is_answered() {
    let mut emulator = Emulator::new(20, 5);
    let encoded = base64(&[0u8; 16]);
    let (first, second) = encoded.split_at(8);
    assert!(
        emulator
            .feed(&apc(&format!("a=t,f=32,s=2,v=2,i=1,m=1;{first}")))
            .is_empty(),
        "a reply per 4096 bytes is a reply per chunk"
    );
    assert_eq!(
        emulator.feed(&apc(&format!("m=0;{second}"))),
        b"\x1b_Gi=1;OK\x1b\\"
    );
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[test]
fn delete_takes_one_id_or_all_of_them() {
    let mut emulator = Emulator::new(20, 5);
    for id in 1..=3 {
        emulator.feed(&apc(&format!(
            "a=t,f=32,s=1,v=1,i={id};{}",
            base64(&pixel())
        )));
    }
    assert_eq!(emulator.graphics().len(), 3);

    emulator.feed(&apc("a=d,d=i,i=2"));
    assert_eq!(emulator.graphics().len(), 2);
    assert!(emulator.graphics().get(2).is_none());

    emulator.feed(&apc("a=d,d=a"));
    assert!(emulator.graphics().is_empty());
}

// ---------------------------------------------------------------------------
// The grid underneath
// ---------------------------------------------------------------------------

#[test]
fn a_command_leaves_the_grid_exactly_as_it_found_it() {
    let mut emulator = Emulator::new(20, 5);
    let mut stream = b"one\r\n".to_vec();
    stream.extend(apc(&format!(
        "a=T,f=32,s=1,v=1,i=1;{}",
        base64(&pixel())
    )));
    stream.extend_from_slice(b"two");
    emulator.feed(&stream);

    assert_eq!(emulator.row_text(0), "one");
    assert_eq!(emulator.row_text(1), "two");
    // Nothing of the payload reached the screen, and the cursor is where the
    // text left it — placement is the part that moves it, and it is not here
    // yet.
    assert_eq!(emulator.cursor().map(|c| (c.row, c.col)), Some((1, 3)));
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

/// A terminal whose cells are 10x20 pixels, which is what turns an image's
/// pixel size into the rows it covers.
fn placed_emulator(cols: u16, rows: u16) -> Emulator {
    let mut emulator = Emulator::new(cols, rows);
    emulator.set_cell_size(10.0, 20.0);
    emulator
}

/// `a=T` for an RGBA image of `width` by `height` pixels.
fn display(id: u32, width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![0xffu8; (width * height * 4) as usize];
    apc(&format!(
        "a=T,f=32,s={width},v={height},i={id};{}",
        base64(&pixels)
    ))
}

#[test]
fn a_displayed_image_lands_at_the_cursor_and_covers_its_cells() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(b"$ icat\r\n");
    emulator.feed(&display(1, 25, 40));

    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    let placement = placements[0];
    assert_eq!((placement.row, placement.col), (1, 0));
    // 25px over 10px cells is three columns; 40px over 20px rows is two.
    assert_eq!((placement.cols, placement.rows), (3, 2));
    assert_eq!(placement.image, 1);
}

#[test]
fn the_cursor_clears_the_image_it_just_placed() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display(1, 20, 40));
    // Two rows covered, so the text after it starts on the third.
    emulator.feed(b"after");
    assert_eq!(emulator.row_text(2), "after");
    assert_eq!(emulator.placements()[0].row, 0);
}

#[test]
fn a_placement_follows_its_text_as_output_scrolls() {
    let mut emulator = placed_emulator(20, 4);
    emulator.feed(b"top\r\n");
    emulator.feed(&display(1, 10, 20));
    assert_eq!(emulator.placements()[0].row, 1);

    // Fill the screen to its last row, which moves nothing.
    emulator.feed(b"a\r\nb");
    assert_eq!(emulator.placements()[0].row, 1);

    // The next line scrolls, and the image has to come up with the text it
    // was placed against.
    emulator.feed(b"\r\nc");
    assert_eq!(
        emulator.placements()[0].row,
        0,
        "the anchor did not move with the grid"
    );
    emulator.feed(b"\r\nd");
    assert!(
        emulator.placements().is_empty(),
        "the image outlived the screen its text left"
    );
}

#[test]
fn a_placement_scrolled_into_history_comes_back_with_it() {
    let mut emulator = placed_emulator(20, 4);
    emulator.feed(&display(1, 10, 20));
    emulator.feed(b"a\r\nb\r\nc\r\nd\r\ne");
    assert!(emulator.placements().is_empty());

    emulator.scroll(4);
    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "the image did not come back");
    assert_eq!(placements[0].image, 1);
}

#[test]
fn a_placement_survives_a_reflow() {
    let mut emulator = placed_emulator(20, 6);
    emulator.feed(&display(1, 10, 20));
    assert_eq!(emulator.placements().len(), 1);

    // Narrow enough to rewrap every row, then back.
    emulator.resize(8, 6);
    emulator.resize(20, 6);
    assert_eq!(
        emulator.placements().len(),
        1,
        "the anchor did not survive the rewrap"
    );
}

#[test]
fn clearing_the_screen_takes_the_image_with_it() {
    let mut emulator = placed_emulator(20, 6);
    emulator.feed(&display(1, 10, 20));
    assert_eq!(emulator.placements().len(), 1);

    emulator.feed(b"\x1b[2J");
    assert!(emulator.placements().is_empty());
}

#[test]
fn an_anchor_never_reaches_the_clipboard() {
    use terminal::emulator::{SelectionType, Side};
    let mut emulator = placed_emulator(20, 6);
    emulator.feed(&display(1, 10, 20));
    emulator.feed(b"text");

    let start = emulator.grid_point(0, 0);
    let end = emulator.grid_point(1, 19);
    emulator.start_selection(SelectionType::Simple, start, Side::Left);
    emulator.update_selection(end, Side::Right);
    let text = emulator.selection_text().unwrap_or_default();
    assert!(
        !text.chars().any(|ch| ('\u{F0000}'..'\u{FFFFD}').contains(&ch)),
        "a private-use anchor was copied: {text:?}"
    );
    assert!(text.contains("text"));
}

#[test]
fn an_image_that_arrives_before_the_first_frame_is_kept_but_not_placed() {
    // The view has not measured a cell yet, so nothing knows how many rows
    // the image covers.
    let mut emulator = Emulator::new(20, 6);
    emulator.feed(&display(1, 10, 20));
    assert!(emulator.graphics().get(1).is_some());
    assert!(emulator.placements().is_empty());
}

// ---------------------------------------------------------------------------
// Paint
// ---------------------------------------------------------------------------

#[test]
fn a_placed_image_decodes_to_the_channel_order_gpui_paints() {
    use terminal::view::Images;

    let mut emulator = placed_emulator(20, 6);
    // One opaque red pixel and one opaque blue one, sent as RGBA.
    let pixels = [0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0xff];
    emulator.feed(&apc(&format!(
        "a=T,f=32,s=2,v=1,i=1;{}",
        base64(&pixels)
    )));

    let mut images = Images::new();
    let placed = images.placed(&emulator);
    assert_eq!(placed.len(), 1, "nothing to paint");
    assert_eq!(placed[0].image.size(0).width.0, 2);
    // `RenderImage` holds BGRA — gpui swaps the channels on the way in, and an
    // image that skipped the swap paints red as blue.
    assert_eq!(
        placed[0].image.as_bytes(0),
        Some(&[0x00, 0x00, 0xff, 0xff, 0xff, 0x00, 0x00, 0xff][..])
    );
}

#[test]
fn rgb_without_an_alpha_channel_is_opaque() {
    use terminal::view::Images;

    let mut emulator = placed_emulator(20, 6);
    emulator.feed(&apc(&format!(
        "a=T,f=24,s=1,v=1,i=1;{}",
        base64(&[0x12, 0x34, 0x56])
    )));
    let mut images = Images::new();
    let placed = images.placed(&emulator);
    assert_eq!(
        placed.first().and_then(|placed| placed.image.as_bytes(0)),
        Some(&[0x56, 0x34, 0x12, 0xff][..])
    );
}

#[test]
fn a_payload_that_does_not_decode_is_not_painted() {
    use terminal::view::Images;

    let mut emulator = placed_emulator(20, 6);
    // Enough of a PNG header to be sized and placed, and nothing a decoder
    // will accept.
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(&[0, 0, 0, 13]);
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&2u32.to_be_bytes());
    png.extend_from_slice(&1u32.to_be_bytes());
    emulator.feed(&apc(&format!("a=T,f=100,i=1;{}", base64(&png))));
    assert_eq!(emulator.placements().len(), 1, "the header did not size it");

    let mut images = Images::new();
    assert!(
        images.placed(&emulator).is_empty(),
        "a hole was painted where an undecodable image is"
    );
}

/// The grid element over an emulator that has been sent an image — the paint
/// path end to end, since nothing below this point is exercised by a snapshot
/// that carries no images.
struct Painted {
    emulator: Emulator,
    images: terminal::view::Images,
    painted: std::rc::Rc<std::cell::Cell<usize>>,
}

impl gpui::Render for Painted {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        let this = cx.entity();
        terminal::view::TerminalElement::new(
            move |geometry, cx| {
                this.update(cx, |this, _| {
                    this.emulator.set_cell_size(geometry.cell_w, geometry.line_h);
                    let images = this.images.placed(&this.emulator);
                    this.painted.set(images.len());
                    Some(terminal::view::GridSnapshot {
                        lines: this.emulator.lines(),
                        cursor: this.emulator.cursor(),
                        images,
                    })
                })
            },
            true,
        )
    }
}

#[gpui::test]
fn an_image_reaches_the_paint(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let painted = std::rc::Rc::new(std::cell::Cell::new(0));
    let window = cx.add_window({
        let painted = painted.clone();
        |_, _| Painted {
            emulator: Emulator::new(80, 24),
            images: terminal::view::Images::new(),
            painted,
        }
    });
    let view = window.root(cx).unwrap();
    let mut cx = gpui::VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(gpui::size(gpui::px(600.0), gpui::px(300.0)));
    cx.run_until_parked();
    // The first frame is what measures a cell, so the image is sent after it:
    // the same order a pty read arrives in.
    assert_eq!(painted.get(), 0);

    let pixels = vec![0x40u8; 32 * 16 * 4];
    let command = apc(&format!("a=T,f=32,s=32,v=16,i=1;{}", base64(&pixels)));
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.emulator.feed(&command);
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert_eq!(painted.get(), 1, "the image never reached a frame");
}
