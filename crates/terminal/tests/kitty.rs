//! The kitty graphics protocol under scripted byte strings — the same harness
//! the rest of the emulator is tested with, because the protocol arrives the
//! same way the escapes do.

use terminal::{
    emulator::{CursorSnapshot, Emulator},
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
            _ => None,
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
// Transmission
// ---------------------------------------------------------------------------

#[test]
fn a_transmitted_image_is_held_under_its_id() {
    let mut emulator = Emulator::new(20, 5);
    emulator.feed(&apc(&format!("a=t,f=32,s=1,v=1,i=9;{}", base64(&pixel()))));
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
    let reply = emulator.feed(&apc(&format!("a=t,f=32,s=2,v=2,i=5;{}", base64(&pixel()))));
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
    let reply = emulator.feed(&apc(&format!("a=t,f=32,s=1,v=1,i=2;{}", base64(&pixel()))));
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
    assert!(
        reply.is_empty(),
        "a chunk mid-transmission answered: {reply:?}"
    );
    let reply = emulator.feed(&apc(&format!("m=0;{tail}")));
    assert!(reply.is_empty(), "the closing chunk answered: {reply:?}");
    assert!(
        emulator.graphics().get(9).is_some(),
        "the image was dropped"
    );
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
fn an_upper_case_delete_frees_one_id_or_all_of_them() {
    let mut emulator = Emulator::new(20, 5);
    for id in 1..=3 {
        emulator.feed(&apc(&format!(
            "a=t,f=32,s=1,v=1,i={id};{}",
            base64(&pixel())
        )));
    }
    assert_eq!(emulator.graphics().len(), 3);

    emulator.feed(&apc("a=d,d=I,i=2"));
    assert_eq!(emulator.graphics().len(), 2);
    assert!(emulator.graphics().get(2).is_none());

    emulator.feed(&apc("a=d,d=A"));
    assert!(emulator.graphics().is_empty());
}

#[test]
fn a_lower_case_delete_keeps_the_data() {
    let mut emulator = placed_emulator(20, 5);
    emulator.feed(&display_keys(1, 10, 20, ",C=1"));
    emulator.feed(&apc("a=d,d=i,i=1"));
    assert!(emulator.placements().is_empty());
    assert!(emulator.graphics().get(1).is_some());

    emulator.feed(&apc("a=p,i=1,C=1"));
    assert_eq!(emulator.placements().len(), 1);
    emulator.feed(&apc("a=d,d=a"));
    assert!(emulator.placements().is_empty());
    assert!(emulator.graphics().get(1).is_some());
}

#[test]
fn an_unknown_delete_is_refused() {
    let mut emulator = Emulator::new(20, 5);
    assert_eq!(
        emulator.feed(&apc("a=d,d=k,i=1")),
        b"\x1b_Gi=1;EINVAL:delete\x1b\\"
    );
}

// ---------------------------------------------------------------------------
// The grid underneath
// ---------------------------------------------------------------------------

#[test]
fn a_command_leaves_the_grid_exactly_as_it_found_it() {
    let mut emulator = Emulator::new(20, 5);
    let mut stream = b"one\r\n".to_vec();
    stream.extend(apc(&format!("a=T,f=32,s=1,v=1,i=1;{}", base64(&pixel()))));
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
    display_keys(id, width, height, "")
}

/// [`display`] with `keys` folded in ahead of the payload.
fn display_keys(id: u32, width: u32, height: u32, keys: &str) -> Vec<u8> {
    let pixels = vec![0xffu8; (width * height * 4) as usize];
    apc(&format!(
        "a=T,f=32,s={width},v={height},i={id}{keys};{}",
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
        !text
            .chars()
            .any(|ch| ('\u{F0000}'..'\u{FFFFD}').contains(&ch)),
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
    emulator.feed(&apc(&format!("a=T,f=32,s=2,v=1,i=1;{}", base64(&pixels))));

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
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let this = cx.entity();
        terminal::view::TerminalElement::new(
            move |geometry, cx| {
                this.update(cx, |this, _| {
                    this.emulator
                        .set_cell_size(geometry.cell_w, geometry.line_h);
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

// ---------------------------------------------------------------------------
// Display keys
// ---------------------------------------------------------------------------

#[test]
fn columns_and_rows_set_the_extent_the_pixels_would_have() {
    let mut emulator = placed_emulator(20, 10);
    // 25x40 pixels over 10x20 cells is three columns by two rows.
    emulator.feed(&display_keys(1, 25, 40, ",c=6,r=4"));

    let placement = emulator.placements()[0];
    assert_eq!((placement.cols, placement.rows), (6, 4));
}

#[test]
fn an_extent_wider_than_the_grid_is_clamped_to_it() {
    let mut emulator = placed_emulator(20, 10);
    // `C=1` so the clamped row count is not immediately scrolled away by the
    // linefeeds that would reserve it.
    emulator.feed(&display_keys(1, 10, 20, ",c=99,r=99,C=1"));

    let placement = emulator.placements()[0];
    assert_eq!((placement.cols, placement.rows), (20, 10));
}

#[test]
fn no_cursor_movement_reserves_nothing_and_moves_nothing() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(b"ab");
    // Two rows tall: without `C=1` the cursor would end up on row 2.
    emulator.feed(&display_keys(1, 10, 40, ",C=1"));

    assert_eq!(emulator.cursor(), Some(CursorSnapshot { row: 0, col: 2 }));
    let placement = emulator.placements()[0];
    assert_eq!((placement.row, placement.col), (0, 2));
    assert_eq!((placement.cols, placement.rows), (1, 2));

    // The rows under it were never reserved, so a program drawing there draws
    // under the image rather than below it.
    emulator.feed(b"\x1b[2;1Hunder");
    assert_eq!(emulator.row_text(1), "under");
    assert_eq!(emulator.placements().len(), 1);
}

// ---------------------------------------------------------------------------
// Synchronized output
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

#[test]
fn an_image_inside_a_synchronized_frame_lands_after_the_frames_text() {
    let mut emulator = placed_emulator(20, 10);
    // One frame: home the cursor, clear, write a header, then display an
    // image on the row below it.
    emulator.feed(b"\x1b[?2026h\x1b[H\x1b[2Jheader\r\n");
    emulator.feed(&display(1, 10, 20));
    emulator.feed(b"\x1b[?2026l");

    assert_eq!(emulator.row_text(0), "header");
    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    assert_eq!((placements[0].row, placements[0].col), (1, 0));
}

// ---------------------------------------------------------------------------
// Anchors under a redraw
// ---------------------------------------------------------------------------

#[test]
fn text_over_the_left_of_an_image_leaves_it_where_it_was() {
    let mut emulator = placed_emulator(20, 10);
    // Six cells wide, drawn at the cursor and leaving it there.
    emulator.feed(&display_keys(1, 60, 20, ",C=1"));
    // The program writes over the image's first two cells.
    emulator.feed(b"ab");

    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    assert_eq!(
        (placements[0].row, placements[0].col),
        (0, 0),
        "the surviving anchors forgot where the left edge was"
    );
}

#[test]
fn text_over_the_whole_top_row_takes_the_image_with_it() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 30, 20, ",C=1"));
    emulator.feed(b"abc");
    assert!(emulator.placements().is_empty());
}

#[test]
fn displaying_an_image_again_replaces_the_placement_it_had() {
    let mut emulator = placed_emulator(20, 10);
    for _ in 0..5 {
        emulator.feed(&display_keys(1, 30, 20, ",C=1"));
    }

    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    assert_eq!(placements[0].image, 1);
}

#[test]
fn an_image_redrawn_somewhere_else_leaves_no_ghost() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 30, 20, ",C=1"));
    emulator.feed(b"\x1b[3;5H");
    emulator.feed(&display_keys(1, 30, 20, ",C=1"));

    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    assert_eq!((placements[0].row, placements[0].col), (2, 4));
}

#[test]
fn two_images_side_by_side_keep_their_own_edges() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 30, 20, ",C=1"));
    emulator.feed(b"\x1b[1;7H");
    emulator.feed(&display_keys(2, 30, 20, ",C=1"));

    let placements = emulator.placements();
    assert_eq!(placements.len(), 2, "{placements:?}");
    assert_eq!((placements[0].image, placements[0].col), (1, 0));
    assert_eq!((placements[1].image, placements[1].col), (2, 6));
}

// ---------------------------------------------------------------------------
// Placements by id
// ---------------------------------------------------------------------------

/// `a=t` for an RGBA image of `width` by `height` pixels, placed nowhere.
fn transmit(id: u32, width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![0xffu8; (width * height * 4) as usize];
    apc(&format!(
        "a=t,f=32,s={width},v={height},i={id};{}",
        base64(&pixels)
    ))
}

fn at(emulator: &Emulator) -> Vec<(u32, usize, usize)> {
    let mut out: Vec<_> = emulator
        .placements()
        .iter()
        .map(|p| (p.image, p.row, p.col))
        .collect();
    out.sort();
    out
}

#[test]
fn a_stored_image_is_placed_at_the_cursor() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&transmit(1, 30, 40));
    assert!(emulator.placements().is_empty());

    emulator.feed(b"\x1b[3;5H");
    let reply = emulator.feed(&apc("a=p,i=1,p=7"));
    assert_eq!(reply, b"\x1b_Gi=1,p=7;OK\x1b\\");
    let placements = emulator.placements();
    assert_eq!(placements.len(), 1, "{placements:?}");
    assert_eq!((placements[0].row, placements[0].col), (2, 4));
    assert_eq!((placements[0].cols, placements[0].rows), (3, 2));
    // `C` is the placement's, as it is on `a=T`.
    assert_eq!(emulator.cursor().map(|c| c.row), Some(4));
}

#[test]
fn placing_an_image_nobody_sent_is_refused() {
    let mut emulator = placed_emulator(20, 10);
    assert_eq!(
        emulator.feed(&apc("a=p,i=4")),
        b"\x1b_Gi=4;ENOENT:image\x1b\\"
    );
    assert!(emulator.placements().is_empty());
}

#[test]
fn one_image_holds_a_placement_per_placement_id() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&transmit(1, 10, 20));
    emulator.feed(&apc("a=p,i=1,p=1,C=1"));
    emulator.feed(b"\x1b[1;5H");
    emulator.feed(&apc("a=p,i=1,p=2,C=1"));
    assert_eq!(at(&emulator), vec![(1, 0, 0), (1, 0, 4)]);

    // The same pair again moves that placement rather than adding one.
    emulator.feed(b"\x1b[4;9H");
    emulator.feed(&apc("a=p,i=1,p=1,C=1"));
    assert_eq!(at(&emulator), vec![(1, 0, 4), (1, 3, 8)]);
}

#[test]
fn a_display_carries_its_placement_id() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 10, 20, ",p=3,C=1"));
    emulator.feed(b"\x1b[1;5H");
    emulator.feed(&apc("a=p,i=1,p=4,C=1"));
    emulator.feed(&apc("a=d,d=i,i=1,p=3"));
    assert_eq!(at(&emulator), vec![(1, 0, 4)]);
}

#[test]
fn a_delete_by_placement_id_takes_that_one_alone() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&transmit(1, 10, 20));
    emulator.feed(&apc("a=p,i=1,p=1,C=1"));
    emulator.feed(b"\x1b[1;5H");
    emulator.feed(&apc("a=p,i=1,p=2,C=1"));

    emulator.feed(&apc("a=d,d=I,i=1,p=1"));
    assert_eq!(at(&emulator), vec![(1, 0, 4)]);
    assert!(
        emulator.graphics().get(1).is_some(),
        "freed while a placement still shows it"
    );
    emulator.feed(&apc("a=d,d=I,i=1,p=2"));
    assert!(emulator.placements().is_empty());
    assert!(emulator.graphics().get(1).is_none());
}

/// Three 2x1 images on row 0 at columns 0, 4 and 8, the last at `z=5`.
fn row_of_three() -> Emulator {
    let mut emulator = placed_emulator(20, 10);
    for (id, col) in [(1, 1), (2, 5), (3, 9)] {
        emulator.feed(&transmit(id, 20, 20));
        emulator.feed(format!("\x1b[1;{col}H").as_bytes());
        let z = if id == 3 { ",z=5" } else { "" };
        emulator.feed(&apc(&format!("a=p,i={id},C=1{z}")));
    }
    emulator
}

#[test]
fn deletes_by_position_take_what_covers_it() {
    let mut emulator = row_of_three();
    emulator.feed(b"\x1b[1;2H");
    emulator.feed(&apc("a=d,d=c"));
    assert_eq!(at(&emulator), vec![(2, 0, 4), (3, 0, 8)]);

    let mut emulator = row_of_three();
    emulator.feed(&apc("a=d,d=p,x=6,y=1"));
    assert_eq!(at(&emulator), vec![(1, 0, 0), (3, 0, 8)]);

    let mut emulator = row_of_three();
    emulator.feed(&apc("a=d,d=q,x=10,y=1,z=4"));
    assert_eq!(at(&emulator).len(), 3, "a z the placement lacks matched");
    emulator.feed(&apc("a=d,d=q,x=10,y=1,z=5"));
    assert_eq!(at(&emulator), vec![(1, 0, 0), (2, 0, 4)]);

    let mut emulator = row_of_three();
    emulator.feed(&apc("a=d,d=x,x=2"));
    assert_eq!(at(&emulator), vec![(2, 0, 4), (3, 0, 8)]);

    let mut emulator = row_of_three();
    emulator.feed(&apc("a=d,d=y,y=2"));
    assert_eq!(at(&emulator).len(), 3, "row 2 holds nothing");
    emulator.feed(&apc("a=d,d=y,y=1"));
    assert!(emulator.placements().is_empty());
}

#[test]
fn deletes_by_z_and_by_id_range() {
    let mut emulator = row_of_three();
    emulator.feed(&apc("a=d,d=z,z=5"));
    assert_eq!(at(&emulator), vec![(1, 0, 0), (2, 0, 4)]);

    let mut emulator = row_of_three();
    emulator.feed(&apc("a=d,d=R,x=2,y=3"));
    assert_eq!(at(&emulator), vec![(1, 0, 0)]);
    assert!(emulator.graphics().get(1).is_some());
    assert!(emulator.graphics().get(2).is_none());
    assert!(emulator.graphics().get(3).is_none());
}

#[test]
fn a_position_delete_frees_only_what_it_left_unplaced() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&transmit(1, 10, 20));
    emulator.feed(&apc("a=p,i=1,p=1,C=1"));
    emulator.feed(b"\x1b[1;5H");
    emulator.feed(&apc("a=p,i=1,p=2,C=1"));

    emulator.feed(&apc("a=d,d=X,x=1"));
    assert!(emulator.graphics().get(1).is_some());
    emulator.feed(&apc("a=d,d=X,x=5"));
    assert!(emulator.graphics().get(1).is_none());
}

// ---------------------------------------------------------------------------
// Image numbers
// ---------------------------------------------------------------------------

#[test]
fn a_transmission_by_number_is_answered_with_the_id_it_got() {
    let mut emulator = placed_emulator(20, 10);
    let reply = emulator.feed(&apc(&format!("a=t,f=32,s=1,v=1,I=13;{}", base64(&pixel()))));
    let reply = String::from_utf8(reply).unwrap();
    let id: u32 = reply
        .strip_prefix("\x1b_Gi=")
        .and_then(|rest| rest.strip_suffix(",I=13;OK\x1b\\"))
        .and_then(|id| id.parse().ok())
        .unwrap_or_else(|| panic!("{reply:?}"));
    assert!(emulator.graphics().get(id).is_some());

    emulator.feed(&apc("a=p,I=13,C=1"));
    assert_eq!(emulator.placements().first().map(|p| p.image), Some(id));

    emulator.feed(&apc("a=d,d=N,I=13"));
    assert!(emulator.placements().is_empty());
    assert!(emulator.graphics().get(id).is_none());
}

#[test]
fn a_number_names_the_newest_image_sent_under_it() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&apc(&format!("a=t,f=32,s=1,v=1,I=2;{}", base64(&pixel()))));
    emulator.feed(&apc(&format!("a=t,f=32,s=1,v=1,I=2;{}", base64(&pixel()))));
    assert_eq!(emulator.graphics().len(), 2);
    emulator.feed(&apc("a=d,d=N,I=2"));
    assert_eq!(emulator.graphics().len(), 1);
}

#[test]
fn an_id_and_a_number_together_are_refused() {
    let mut emulator = placed_emulator(20, 10);
    let reply = emulator.feed(&apc(&format!(
        "a=t,f=32,s=1,v=1,i=1,I=2;{}",
        base64(&pixel())
    )));
    assert_eq!(reply, b"\x1b_Gi=1,I=2;EINVAL:i and I\x1b\\");
    assert!(emulator.graphics().is_empty());
}

// ---------------------------------------------------------------------------
// Source rectangle, offsets, fit and z
// ---------------------------------------------------------------------------

fn frame(placement: &terminal::emulator::Placement) -> (f32, f32, f32, f32) {
    let frame = placement.frame;
    (frame.x, frame.y, frame.width, frame.height)
}

#[test]
fn a_source_rectangle_draws_that_part_alone() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 60, 40, ",x=10,y=20,w=20,h=20,C=1"));
    let placement = emulator.placements()[0];
    let source = placement.source;
    assert_eq!(
        (source.x, source.y, source.width, source.height),
        (10, 20, 20, 20)
    );
    assert_eq!((placement.cols, placement.rows), (2, 1));
    assert_eq!(frame(&placement), (0.0, 0.0, 2.0, 1.0));
}

#[test]
fn a_source_rectangle_is_cut_to_the_image() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 30, 20, ",x=20,w=50,C=1"));
    let source = emulator.placements()[0].source;
    assert_eq!(
        (source.x, source.y, source.width, source.height),
        (20, 0, 10, 20)
    );
}

#[test]
fn an_offset_moves_the_picture_inside_its_first_cell() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 10, 20, ",X=5,Y=10,C=1"));
    let placement = emulator.placements()[0];
    // The offset pushes the picture into a second column and a second row.
    assert_eq!((placement.cols, placement.rows), (2, 2));
    assert_eq!(frame(&placement), (0.5, 0.5, 1.0, 1.0));

    // An offset past the cell stops at its last pixel.
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 10, 20, ",X=50,C=1"));
    assert_eq!(emulator.placements()[0].frame.x, 0.9);
}

#[test]
fn one_of_columns_and_rows_keeps_the_aspect() {
    let mut emulator = placed_emulator(20, 10);
    // 20x40 across four 10px columns is 40x80: four 20px rows.
    emulator.feed(&display_keys(1, 20, 40, ",c=4,C=1"));
    let placement = emulator.placements()[0];
    assert_eq!((placement.cols, placement.rows), (4, 4));
    assert_eq!(frame(&placement), (0.0, 0.0, 4.0, 4.0));

    let mut emulator = placed_emulator(20, 10);
    // 20x40 down one 20px row is 10x20: one column.
    emulator.feed(&display_keys(1, 20, 40, ",r=1,C=1"));
    let placement = emulator.placements()[0];
    assert_eq!((placement.cols, placement.rows), (1, 1));
    assert_eq!(frame(&placement), (0.0, 0.0, 1.0, 1.0));
}

#[test]
fn both_columns_and_rows_letterbox_the_picture() {
    let mut emulator = placed_emulator(20, 10);
    // A square into a 60x40 box: 40x40, centred, so 10px in from the left.
    emulator.feed(&display_keys(1, 20, 20, ",c=6,r=2,C=1"));
    let placement = emulator.placements()[0];
    assert_eq!((placement.cols, placement.rows), (6, 2));
    assert_eq!(frame(&placement), (1.0, 0.0, 4.0, 2.0));
}

#[test]
fn a_placement_carries_its_z_index() {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&display_keys(1, 10, 20, ",z=-1,C=1"));
    assert_eq!(emulator.placements()[0].z, -1);
    emulator.feed(&apc("a=p,i=1,p=2,z=-1073741825,C=1"));
    let mut zs: Vec<i32> = emulator.placements().iter().map(|p| p.z).collect();
    zs.sort();
    assert_eq!(zs, vec![-1073741825, -1]);
}

// ---------------------------------------------------------------------------
// Compression
// ---------------------------------------------------------------------------

fn zlib(bytes: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(bytes, 6)
}

/// `keys` over a payload sent in 4096-byte base64 chunks, the way a client
/// sends one too big for a single run.
fn chunked(keys: &str, payload: &[u8]) -> Vec<u8> {
    let encoded = base64(payload);
    let pieces: Vec<&str> = encoded
        .as_bytes()
        .chunks(4096)
        .map(|piece| std::str::from_utf8(piece).unwrap())
        .collect();
    let mut out = Vec::new();
    for (at, piece) in pieces.iter().enumerate() {
        let more = u8::from(at + 1 < pieces.len());
        let head = if at == 0 {
            format!("{keys},")
        } else {
            String::new()
        };
        out.extend(apc(&format!("{head}m={more};{piece}")));
    }
    out
}

#[test]
fn a_zlib_payload_is_held_inflated() {
    let mut emulator = Emulator::new(20, 5);
    let reply = emulator.feed(&apc(&format!(
        "a=t,f=32,s=1,v=1,o=z,i=3;{}",
        base64(&zlib(&pixel()))
    )));
    assert_eq!(reply, b"\x1b_Gi=3;OK\x1b\\");
    assert_eq!(
        emulator.graphics().get(3).map(|image| &image.bytes),
        Some(&pixel())
    );
}

#[test]
fn a_zlib_stream_split_across_chunks_is_inflated_whole() {
    let mut emulator = Emulator::new(20, 5);
    // Noise, so the stream stays long enough to need several chunks.
    let mut seed = 1u32;
    let pixels: Vec<u8> = (0..64 * 64 * 4)
        .map(|_| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) as u8
        })
        .collect();
    let compressed = zlib(&pixels);
    assert!(base64(&compressed).len() > 4096, "one chunk proves nothing");
    emulator.feed(&chunked("a=t,f=32,s=64,v=64,o=z,i=4", &compressed));
    assert_eq!(
        emulator.graphics().get(4).map(|image| &image.bytes),
        Some(&pixels)
    );
}

#[test]
fn a_payload_that_does_not_inflate_is_refused() {
    let mut emulator = Emulator::new(20, 5);
    let reply = emulator.feed(&apc(&format!(
        "a=t,f=32,s=1,v=1,o=z,i=5;{}",
        base64(&pixel())
    )));
    assert_eq!(reply, b"\x1b_Gi=5;EINVAL:compression\x1b\\");
    assert!(emulator.graphics().is_empty());
}

#[test]
fn an_unknown_compression_is_refused() {
    let mut emulator = Emulator::new(20, 5);
    let reply = emulator.feed(&apc(&format!(
        "a=t,f=32,s=1,v=1,o=x,i=6;{}",
        base64(&pixel())
    )));
    assert_eq!(reply, b"\x1b_Gi=6;ENOTSUPPORTED:compression\x1b\\");
}

#[test]
fn a_payload_that_inflates_past_the_ceiling_is_refused() {
    let mut emulator = Emulator::new(20, 5);
    let compressed = zlib(&vec![0u8; (64 << 20) + 1]);
    let reply = emulator.feed(&chunked("a=t,f=32,s=4097,v=4096,o=z,i=7", &compressed));
    assert_eq!(reply, b"\x1b_Gi=7;EFBIG:payload\x1b\\");
    assert!(emulator.graphics().is_empty());
}

// ---------------------------------------------------------------------------
// Unicode placeholders
// ---------------------------------------------------------------------------

/// The diacritics for 0, 1 and 2.
const D: [char; 3] = ['\u{0305}', '\u{030D}', '\u{030E}'];
const P: char = '\u{10EEEE}';

/// A 20x40 image under id 42 with a 2x2 virtual placement: one cell per
/// 10x20 quarter.
fn virtual_emulator() -> Emulator {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&transmit(42, 20, 40));
    emulator.feed(&apc("a=p,U=1,i=42,c=2,r=2,q=2"));
    emulator
}

fn pieces(emulator: &Emulator) -> Vec<(usize, usize, u16, f32, f32)> {
    let mut out: Vec<_> = emulator
        .placements()
        .iter()
        .map(|p| (p.row, p.col, p.cols, p.frame.x, p.frame.y))
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

#[test]
fn a_virtual_placement_puts_nothing_on_the_grid() {
    let emulator = virtual_emulator();
    assert!(emulator.placements().is_empty());
    assert_eq!(emulator.cursor().map(|c| (c.row, c.col)), Some((0, 0)));
}

#[test]
fn placeholder_cells_show_their_slices() {
    let mut emulator = virtual_emulator();
    let text = format!(
        "\x1b[38;5;42m{P}{}{}{P}{}{}\x1b[39m\r\n\x1b[38;5;42m{P}{}{}{P}{}{}\x1b[39m",
        D[0], D[0], D[0], D[1], D[1], D[0], D[1], D[1]
    );
    emulator.feed(text.as_bytes());
    // One run per row, each drawing the whole frame shifted by its row.
    assert_eq!(
        pieces(&emulator),
        vec![(0, 0, 2, 0.0, 0.0), (1, 0, 2, 0.0, -1.0)]
    );
    assert!(emulator.placements().iter().all(|p| p.image == 42));
    // The placeholder never reaches the glyphs.
    assert_eq!(emulator.row_text(0), "");
}

#[test]
fn a_cell_without_marks_continues_the_one_to_its_left() {
    let mut emulator = virtual_emulator();
    let text = format!("\x1b[38;5;42m{P}{}{P}\r\n{P}{}{P}\x1b[39m", D[0], D[1]);
    emulator.feed(text.as_bytes());
    assert_eq!(
        pieces(&emulator),
        vec![(0, 0, 2, 0.0, 0.0), (1, 0, 2, 0.0, -1.0)]
    );
}

#[test]
fn a_slice_starts_where_its_column_says() {
    let mut emulator = virtual_emulator();
    // The right-hand column alone, drawn at the left edge of the screen.
    let text = format!("\x1b[38;5;42m{P}{}{}\x1b[39m", D[0], D[1]);
    emulator.feed(text.as_bytes());
    assert_eq!(pieces(&emulator), vec![(0, 0, 1, -1.0, 0.0)]);
}

#[test]
fn a_third_mark_is_the_high_byte_of_the_id() {
    let mut emulator = placed_emulator(20, 10);
    let id = 42 + (2 << 24);
    emulator.feed(&transmit(id, 10, 20));
    emulator.feed(&apc(&format!("a=p,U=1,i={id},c=1,r=1,q=2")));
    let text = format!("\x1b[38;5;42m{P}{}{}{}\x1b[39m", D[0], D[0], D[2]);
    emulator.feed(text.as_bytes());
    assert_eq!(emulator.placements().first().map(|p| p.image), Some(id));
}

#[test]
fn the_underline_color_picks_the_placement() {
    let mut emulator = virtual_emulator();
    // A second virtual placement of the same image, two cells by one.
    emulator.feed(&apc("a=p,U=1,i=42,p=7,c=2,r=1,q=2"));
    let text = format!("\x1b[38;5;42;58;5;7m{P}{}{}\x1b[m", D[0], D[0]);
    emulator.feed(text.as_bytes());
    let frame = emulator.placements()[0].frame;
    // The 20x20 box pillarboxes the 10x20 fit half a cell in from the left.
    assert_eq!(
        (frame.x, frame.y, frame.width, frame.height),
        (0.5, 0.0, 1.0, 1.0)
    );
}

#[test]
fn a_cell_naming_no_placement_is_not_drawn() {
    let mut emulator = virtual_emulator();
    let text = format!("\x1b[38;5;41m{P}{}{}\x1b[39m", D[0], D[0]);
    emulator.feed(text.as_bytes());
    assert!(emulator.placements().is_empty());
}

#[test]
fn only_deletes_by_image_reach_a_virtual_placement() {
    let mut emulator = virtual_emulator();
    let text = format!("\x1b[38;5;42m{P}{}{}\x1b[39m", D[0], D[0]);
    emulator.feed(text.as_bytes());
    emulator.feed(&apc("a=d,d=a"));
    emulator.feed(&apc("a=d,d=p,x=1,y=1"));
    assert_eq!(emulator.placements().len(), 1);

    emulator.feed(&apc("a=d,d=I,i=42"));
    assert!(emulator.placements().is_empty());
    assert!(emulator.graphics().get(42).is_none());
}

#[test]
fn a_virtual_placement_keeps_its_image_from_being_freed() {
    let mut emulator = virtual_emulator();
    emulator.feed(&apc("a=p,i=42,C=1"));
    emulator.feed(&apc("a=d,d=A"));
    // `A` frees everything, virtual or not.
    assert!(emulator.graphics().get(42).is_none());

    let mut emulator = virtual_emulator();
    emulator.feed(&apc("a=p,i=42,C=1"));
    emulator.feed(&apc("a=d,d=C"));
    assert!(emulator.graphics().get(42).is_some());
}

// ---------------------------------------------------------------------------
// Named mediums
// ---------------------------------------------------------------------------

/// A file under the temp directory holding `bytes`, named so no two tests
/// share one.
fn temp_file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("bezel-{}-{tag}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

fn local_emulator() -> Emulator {
    let mut emulator = Emulator::new(20, 5);
    emulator.set_local_media(true);
    emulator
}

fn named(keys: &str, name: &str) -> Vec<u8> {
    apc(&format!("{keys};{}", base64(name.as_bytes())))
}

const UNREADABLE: &[u8] = b"\x1b_Gi=1;EBADF:Failed to read image file\x1b\\";

#[test]
fn a_named_medium_is_refused_until_the_host_allows_it() {
    let path = temp_file("refused", &pixel());
    let mut emulator = Emulator::new(20, 5);
    let reply = emulator.feed(&named("a=t,f=32,s=1,v=1,t=f,i=1", path.to_str().unwrap()));
    assert_eq!(reply, b"\x1b_Gi=1;ENOTSUPPORTED:medium\x1b\\");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_file_is_read_and_left_where_it_was() {
    let path = temp_file("file", &pixel());
    let mut emulator = local_emulator();
    let reply = emulator.feed(&named("a=t,f=32,s=1,v=1,t=f,i=1", path.to_str().unwrap()));
    assert_eq!(reply, b"\x1b_Gi=1;OK\x1b\\");
    assert_eq!(
        emulator.graphics().get(1).map(|image| &image.bytes),
        Some(&pixel())
    );
    assert!(path.exists());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn an_offset_and_size_read_part_of_a_file() {
    let mut bytes = vec![9u8; 3];
    bytes.extend(pixel());
    bytes.extend([9u8; 5]);
    let path = temp_file("span", &bytes);
    let mut emulator = local_emulator();
    emulator.feed(&named(
        "a=t,f=32,s=1,v=1,t=f,O=3,S=4,i=1",
        path.to_str().unwrap(),
    ));
    assert_eq!(
        emulator.graphics().get(1).map(|image| &image.bytes),
        Some(&pixel())
    );

    // A size past the end of the file is a short read.
    let reply = emulator.feed(&named(
        "a=t,f=32,s=1,v=1,t=f,O=10,S=4,i=1",
        path.to_str().unwrap(),
    ));
    assert_eq!(reply, UNREADABLE);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_temporary_file_is_deleted_only_when_it_says_it_may_be() {
    let marked = temp_file("tty-graphics-protocol-a", &pixel());
    let mut emulator = local_emulator();
    emulator.feed(&named("a=t,f=32,s=1,v=1,t=t,i=1", marked.to_str().unwrap()));
    assert!(emulator.graphics().get(1).is_some());
    assert!(
        !marked.exists(),
        "a marked temporary file outlived its read"
    );

    let unmarked = temp_file("unmarked", &pixel());
    emulator.feed(&named(
        "a=t,f=32,s=1,v=1,t=t,i=2",
        unmarked.to_str().unwrap(),
    ));
    assert!(emulator.graphics().get(2).is_some());
    assert!(unmarked.exists(), "a file without the marker was deleted");
    std::fs::remove_file(unmarked).unwrap();
}

#[test]
fn every_unreadable_name_gets_the_same_answer() {
    let mut emulator = local_emulator();
    let dir = std::env::temp_dir();
    for name in [
        "/nonexistent/bezel/image",
        dir.to_str().unwrap(),
        "/dev/null",
        "relative/path",
    ] {
        let reply = emulator.feed(&named("a=t,f=32,s=1,v=1,t=f,i=1", name));
        assert_eq!(reply, UNREADABLE, "{name}");
    }
    assert!(emulator.graphics().is_empty());
}

#[cfg(unix)]
#[test]
fn a_shared_memory_object_is_read_and_unlinked() {
    use std::ffi::CString;

    let name = format!("/bezel-{}", std::process::id());
    let c_name = CString::new(name.clone()).unwrap();
    let bytes = pixel();
    // SAFETY: plain libc calls on a name this test owns.
    unsafe {
        let fd = libc::shm_open(c_name.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o600);
        assert!(fd >= 0, "shm_open failed");
        assert_eq!(libc::ftruncate(fd, bytes.len() as libc::off_t), 0);
        let map = libc::mmap(
            std::ptr::null_mut(),
            bytes.len(),
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        );
        assert_ne!(map, libc::MAP_FAILED);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), map as *mut u8, bytes.len());
        libc::munmap(map, bytes.len());
        libc::close(fd);
    }

    let mut emulator = local_emulator();
    // macOS rounds the object up to a page, so the size says where it ends.
    let reply = emulator.feed(&named("a=t,f=32,s=1,v=1,t=s,S=4,i=1", &name));
    assert_eq!(reply, b"\x1b_Gi=1;OK\x1b\\");
    assert_eq!(
        emulator.graphics().get(1).map(|image| &image.bytes),
        Some(&bytes)
    );
    // SAFETY: as above.
    let reopened = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDONLY, 0) };
    assert!(reopened < 0, "the object was not unlinked");
}

// ---------------------------------------------------------------------------
// Relative placements
// ---------------------------------------------------------------------------

/// Image 1 placed at row 2, column 3 under placement id 1, and image 2 held
/// to hang off it.
fn parented() -> Emulator {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&transmit(1, 10, 20));
    emulator.feed(&transmit(2, 10, 20));
    emulator.feed(b"\x1b[3;4H");
    emulator.feed(&apc("a=p,i=1,p=1,C=1"));
    emulator
}

#[test]
fn a_relative_placement_sits_off_its_parent() {
    let mut emulator = parented();
    let reply = emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=1,H=4,V=-1"));
    assert_eq!(reply, b"\x1b_Gi=2,p=5;OK\x1b\\");
    assert_eq!(at(&emulator), vec![(1, 2, 3), (2, 1, 7)]);
    // No `C=1`, and still the cursor stays put.
    assert_eq!(emulator.cursor().map(|c| (c.row, c.col)), Some((2, 3)));
}

#[test]
fn a_relative_placement_moves_with_its_parent() {
    let mut emulator = parented();
    emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=1,H=1"));
    emulator.feed(b"\x1b[10;1H\n\n");
    assert_eq!(at(&emulator), vec![(1, 0, 3), (2, 0, 4)]);
}

#[test]
fn a_relative_placement_follows_a_chain() {
    let mut emulator = parented();
    emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=1,H=1"));
    emulator.feed(&apc("a=p,i=2,p=6,P=2,Q=5,V=1"));
    assert_eq!(at(&emulator), vec![(1, 2, 3), (2, 2, 4), (2, 3, 4)]);
}

#[test]
fn a_parent_that_is_not_there_is_refused() {
    let mut emulator = parented();
    let reply = emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=9"));
    assert_eq!(reply, b"\x1b_Gi=2,p=5;ENOPARENT\x1b\\");
    assert_eq!(emulator.placements().len(), 1);
}

#[test]
fn a_cycle_is_refused() {
    let mut emulator = parented();
    emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=1"));
    emulator.feed(&apc("a=p,i=2,p=6,P=2,Q=5"));
    let reply = emulator.feed(&apc("a=p,i=1,p=1,P=2,Q=6"));
    assert_eq!(reply, b"\x1b_Gi=1,p=1;ECYCLE\x1b\\");
}

#[test]
fn a_chain_past_eight_is_refused() {
    let mut emulator = parented();
    for p in 2..=9 {
        let reply = emulator.feed(&apc(&format!(
            "a=p,i=2,p={p},P={},Q={}",
            if p == 2 { 1 } else { 2 },
            p - 1
        )));
        assert_eq!(
            reply,
            format!("\x1b_Gi=2,p={p};OK\x1b\\").into_bytes(),
            "p={p}"
        );
    }
    let reply = emulator.feed(&apc("a=p,i=2,p=10,P=2,Q=9"));
    assert_eq!(reply, b"\x1b_Gi=2,p=10;ETOODEEP\x1b\\");
}

#[test]
fn a_virtual_placement_cannot_be_relative() {
    let mut emulator = parented();
    let reply = emulator.feed(&apc("a=p,U=1,i=2,p=5,P=1,Q=1"));
    assert_eq!(
        reply,
        b"\x1b_Gi=2,p=5;EINVAL:a virtual placement cannot be relative\x1b\\"
    );
}

#[test]
fn a_quiet_command_is_refused_quietly() {
    let mut emulator = parented();
    assert!(emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=9,q=2")).is_empty());
    // `q=1` silences a success, not a refusal.
    assert_eq!(
        emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=9,q=1")),
        b"\x1b_Gi=2,p=5;ENOPARENT\x1b\\"
    );
}

#[test]
fn deleting_a_parent_takes_its_relatives_and_their_images() {
    let mut emulator = parented();
    emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=1,H=2"));
    emulator.feed(&apc("a=d,d=i,i=1,p=1"));
    assert!(emulator.placements().is_empty());
    assert!(
        emulator.graphics().get(1).is_some(),
        "a lower-case delete freed the parent's image"
    );
    assert!(
        emulator.graphics().get(2).is_none(),
        "the relative's image outlived its last placement"
    );
}

#[test]
fn a_delete_by_position_finds_a_relative_where_it_sits() {
    let mut emulator = parented();
    emulator.feed(&apc("a=p,i=2,p=5,P=1,Q=1,H=5"));
    emulator.feed(&apc("a=d,d=p,x=9,y=3"));
    assert_eq!(at(&emulator), vec![(1, 2, 3)]);
}

#[test]
fn a_virtual_parent_is_where_its_placeholders_are() {
    let mut emulator = virtual_emulator();
    emulator.feed(&transmit(2, 10, 20));
    let text = format!("\x1b[2;5H\x1b[38;5;42m{P}{}{}{P}\x1b[39m", D[0], D[0]);
    emulator.feed(text.as_bytes());
    emulator.feed(&apc("a=p,i=2,p=5,P=42,H=1,V=2"));
    let relative: Vec<_> = at(&emulator).into_iter().filter(|p| p.0 == 2).collect();
    assert_eq!(relative, vec![(2, 3, 5)]);
}

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

const RED: [u8; 4] = [0xff, 0, 0, 0xff];
const BLUE: [u8; 4] = [0, 0, 0xff, 0xff];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// A 2x1 RGBA image under id 1, both pixels red.
fn animated() -> Emulator {
    let mut emulator = placed_emulator(20, 10);
    emulator.feed(&apc(&format!(
        "a=t,f=32,s=2,v=1,i=1;{}",
        base64(&[RED, RED].concat())
    )));
    emulator
}

fn frame_bytes(emulator: &Emulator, index: usize) -> Vec<u8> {
    emulator
        .graphics()
        .get(1)
        .and_then(|image| image.frame(index))
        .unwrap_or_default()
        .to_vec()
}

/// `a=f` with `keys`, carrying one RGBA pixel.
fn one_pixel_frame(keys: &str, pixel: [u8; 4]) -> Vec<u8> {
    apc(&format!("a=f,i=1,f=32,s=1,v=1{keys};{}", base64(&pixel)))
}

#[test]
fn a_frame_lands_on_a_blank_canvas_by_default() {
    let mut emulator = animated();
    let reply = emulator.feed(&one_pixel_frame(",x=1", BLUE));
    assert_eq!(reply, b"\x1b_Gi=1;OK\x1b\\");
    let image = emulator.graphics().get(1).unwrap();
    assert_eq!(image.frame_count(), 2);
    assert_eq!(
        image.gaps,
        vec![0, 40],
        "the root has no gap, a new frame the default"
    );
    assert_eq!(frame_bytes(&emulator, 1), [CLEAR, BLUE].concat());
}

#[test]
fn a_frame_can_start_from_another_or_from_a_color() {
    let mut emulator = animated();
    emulator.feed(&one_pixel_frame(",c=1", BLUE));
    assert_eq!(frame_bytes(&emulator, 1), [BLUE, RED].concat());

    // 0x00ff00ff: opaque green.
    emulator.feed(&one_pixel_frame(",Y=16711935,x=1", BLUE));
    assert_eq!(
        frame_bytes(&emulator, 2),
        [[0, 0xff, 0, 0xff], BLUE].concat()
    );
}

#[test]
fn a_frame_edit_draws_over_the_frame_it_names() {
    let mut emulator = animated();
    emulator.feed(&one_pixel_frame(",r=1,x=1,z=100", BLUE));
    let image = emulator.graphics().get(1).unwrap();
    assert_eq!(image.frame_count(), 1, "an edit made a frame");
    assert_eq!(image.gaps, vec![100]);
    assert_eq!(frame_bytes(&emulator, 0), [RED, BLUE].concat());
}

#[test]
fn a_translucent_frame_blends_unless_told_to_replace() {
    let half_blue = [0, 0, 0xff, 0x80];
    let mut emulator = animated();
    emulator.feed(&one_pixel_frame(",r=1", half_blue));
    let blended = frame_bytes(&emulator, 0);
    assert_eq!(blended[3], 0xff, "blending onto opaque stays opaque");
    assert!(blended[0] > 0 && blended[2] > 0, "{blended:?}");

    let mut emulator = animated();
    emulator.feed(&one_pixel_frame(",r=1,X=1", half_blue));
    assert_eq!(&frame_bytes(&emulator, 0)[..4], &half_blue);
}

#[test]
fn a_frame_for_an_image_that_is_not_there_is_refused() {
    let mut emulator = animated();
    assert_eq!(
        emulator.feed(&apc(&format!("a=f,i=9,f=32,s=1,v=1;{}", base64(&BLUE)))),
        b"\x1b_Gi=9;ENOENT:image\x1b\\"
    );
    let reply = emulator.feed(&apc(&format!(
        "a=f,i=1,f=32,s=3,v=1;{}",
        base64(&[BLUE; 3].concat())
    )));
    assert_eq!(reply, b"\x1b_Gi=1;EINVAL:frame larger than the image\x1b\\");
}

#[test]
fn the_control_command_sets_state_frame_loops_and_gaps() {
    use terminal::kitty::AnimationState;
    let mut emulator = animated();
    emulator.feed(&one_pixel_frame("", BLUE));
    emulator.feed(&apc("a=a,i=1,s=3,c=2,v=3,r=1,z=25"));
    let image = emulator.graphics().get(1).unwrap();
    assert_eq!(image.animation.state, AnimationState::Running);
    assert_eq!(image.animation.current, 1);
    assert_eq!(image.animation.loops, 2);
    assert_eq!(image.gaps, vec![25, 40]);

    emulator.feed(&apc("a=a,i=1,s=1,r=2,z=-1"));
    let image = emulator.graphics().get(1).unwrap();
    assert_eq!(image.animation.state, AnimationState::Stopped);
    assert_eq!(image.gaps, vec![25, 0], "a negative gap is gapless");
}

#[test]
fn composing_copies_a_rectangle_between_frames() {
    let mut emulator = animated();
    emulator.feed(&one_pixel_frame("", BLUE));
    // Frame 2's left pixel onto frame 1's right one, replacing.
    let reply = emulator.feed(&apc("a=c,i=1,r=2,c=1,w=1,h=1,x=1,C=1"));
    assert_eq!(reply, b"\x1b_Gi=1;OK\x1b\\");
    assert_eq!(frame_bytes(&emulator, 0), [RED, BLUE].concat());
}

#[test]
fn composing_refuses_what_it_cannot_do() {
    let mut emulator = animated();
    assert_eq!(
        emulator.feed(&apc("a=c,i=1,r=1,c=3")),
        b"\x1b_Gi=1;ENOENT:frame\x1b\\"
    );
    emulator.feed(&one_pixel_frame("", BLUE));
    assert_eq!(
        emulator.feed(&apc("a=c,i=1,r=2,c=1,w=2,h=1,x=1")),
        b"\x1b_Gi=1;EINVAL:rectangle out of bounds\x1b\\"
    );
    assert_eq!(
        emulator.feed(&apc("a=c,i=1,r=1,c=1,w=2,h=1")),
        b"\x1b_Gi=1;EINVAL:rectangles overlap\x1b\\"
    );
}

#[test]
fn deleting_a_frame_moves_the_rest_up() {
    let mut emulator = animated();
    emulator.feed(&one_pixel_frame("", BLUE));
    emulator.feed(&one_pixel_frame(",c=1", BLUE));
    emulator.feed(&apc("a=d,d=f,i=1"));
    let image = emulator.graphics().get(1).unwrap();
    assert_eq!(image.frame_count(), 2);
    assert_eq!(
        frame_bytes(&emulator, 0),
        [BLUE, CLEAR].concat(),
        "the second frame is the root now"
    );

    emulator.feed(&apc("a=d,d=f,i=1,r=2"));
    assert_eq!(emulator.graphics().get(1).unwrap().frame_count(), 1);
    // `F` on the one frame left frees the image.
    emulator.feed(&apc("a=d,d=F,i=1"));
    assert!(emulator.graphics().get(1).is_none());
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

/// A placed 2x1 image with three frames, 100ms each, in `state`.
fn playing(control: &str) -> Emulator {
    let mut emulator = animated();
    emulator.feed(&apc("a=p,i=1,C=1"));
    emulator.feed(&one_pixel_frame(",z=100", BLUE));
    emulator.feed(&one_pixel_frame(",z=100", BLUE));
    emulator.feed(&apc(&format!("a=a,i=1,r=1,z=100,{control}")));
    emulator
}

fn frame_at(
    images: &mut terminal::view::Images,
    emulator: &Emulator,
    at: std::time::Instant,
) -> (usize, bool) {
    let placed = images.placed_at(emulator, at);
    (placed[0].frame_index, placed[0].animating)
}

#[test]
fn a_stopped_animation_shows_its_current_frame() {
    let emulator = playing("c=2");
    let mut images = terminal::view::Images::new();
    let start = std::time::Instant::now();
    assert_eq!(frame_at(&mut images, &emulator, start), (1, false));
    assert_eq!(
        frame_at(
            &mut images,
            &emulator,
            start + std::time::Duration::from_secs(5)
        ),
        (1, false)
    );
}

#[test]
fn a_running_animation_steps_by_its_gaps_and_loops() {
    use std::time::Duration;
    let emulator = playing("s=3");
    let mut images = terminal::view::Images::new();
    let start = std::time::Instant::now();
    assert_eq!(frame_at(&mut images, &emulator, start), (0, true));
    assert_eq!(
        frame_at(&mut images, &emulator, start + Duration::from_millis(150)),
        (1, true)
    );
    assert_eq!(
        frame_at(&mut images, &emulator, start + Duration::from_millis(250)),
        (2, true)
    );
    assert_eq!(
        frame_at(&mut images, &emulator, start + Duration::from_millis(350)),
        (0, true)
    );
}

#[test]
fn a_limited_animation_stops_on_its_last_frame() {
    use std::time::Duration;
    // `v=2`: one loop, and then it stops.
    let emulator = playing("s=3,v=2");
    let mut images = terminal::view::Images::new();
    let start = std::time::Instant::now();
    frame_at(&mut images, &emulator, start);
    assert_eq!(
        frame_at(&mut images, &emulator, start + Duration::from_secs(5)),
        (2, false)
    );
}

#[test]
fn a_loading_animation_waits_at_the_end_for_more_frames() {
    use std::time::Duration;
    let mut emulator = playing("s=2");
    let mut images = terminal::view::Images::new();
    let start = std::time::Instant::now();
    frame_at(&mut images, &emulator, start);
    assert_eq!(
        frame_at(&mut images, &emulator, start + Duration::from_secs(5)),
        (2, false)
    );

    emulator.feed(&one_pixel_frame(",z=100", BLUE));
    let later = start + Duration::from_secs(5) + Duration::from_millis(10);
    assert_eq!(frame_at(&mut images, &emulator, later), (3, true));
}

#[test]
fn every_frame_reaches_the_paint() {
    let emulator = playing("s=3");
    let mut images = terminal::view::Images::new();
    let placed = images.placed_at(&emulator, std::time::Instant::now());
    assert_eq!(placed[0].image.frame_count(), 3);
}

// ---------------------------------------------------------------------------
// Sixel
// ---------------------------------------------------------------------------

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
fn the_primary_attributes_advertise_sixel() {
    let mut emulator = Emulator::new(20, 5);
    assert_eq!(emulator.feed(b"\x1b[c"), b"\x1b[?62;4;22c");
}

// ---------------------------------------------------------------------------
// iTerm2 inline images
// ---------------------------------------------------------------------------

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
        vec![terminal::kitty::Iterm::File {
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

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

#[test]
fn a_bare_query_is_answered_yes() {
    let mut emulator = Emulator::new(20, 5);
    assert_eq!(emulator.feed(&apc("a=q,i=31")), b"\x1b_Gi=31;OK\x1b\\");
}

#[test]
fn a_query_answers_what_the_transmission_would_and_stores_nothing() {
    let mut emulator = Emulator::new(20, 5);
    let reply = emulator.feed(&apc(&format!("a=q,f=32,s=1,v=1,i=31;{}", base64(&pixel()))));
    assert_eq!(reply, b"\x1b_Gi=31;OK\x1b\\");
    let reply = emulator.feed(&apc(&format!("a=q,f=32,s=2,v=2,i=31;{}", base64(&pixel()))));
    assert_eq!(reply, b"\x1b_Gi=31;EINVAL:dimensions\x1b\\");
    let reply = emulator.feed(&apc(&format!(
        "a=q,f=32,s=1,v=1,o=z,i=31;{}",
        base64(&pixel())
    )));
    assert_eq!(reply, b"\x1b_Gi=31;EINVAL:compression\x1b\\");
    assert!(emulator.graphics().is_empty());
}

#[test]
fn a_query_for_a_named_medium_says_whether_it_is_read() {
    let path = temp_file("query", &pixel());
    let keys = "a=q,f=32,s=1,v=1,t=f,i=31";

    let mut emulator = Emulator::new(20, 5);
    assert_eq!(
        emulator.feed(&named(keys, path.to_str().unwrap())),
        b"\x1b_Gi=31;ENOTSUPPORTED:medium\x1b\\"
    );

    let mut emulator = local_emulator();
    assert_eq!(
        emulator.feed(&named(keys, path.to_str().unwrap())),
        b"\x1b_Gi=31;OK\x1b\\"
    );
    assert_eq!(
        emulator.feed(&named(keys, "/nonexistent/bezel/image")),
        b"\x1b_Gi=31;EBADF:Failed to read image file\x1b\\"
    );
    assert!(emulator.graphics().is_empty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_query_by_temporary_file_deletes_it_as_a_transmission_would() {
    let path = temp_file("tty-graphics-protocol-query", &pixel());
    let mut emulator = local_emulator();
    let reply = emulator.feed(&named("a=q,f=32,s=1,v=1,t=t,i=31", path.to_str().unwrap()));
    assert_eq!(reply, b"\x1b_Gi=31;OK\x1b\\");
    assert!(!path.exists());
}
