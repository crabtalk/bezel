use gpui::Modifiers;

use terminal::{emulator::Side, view::*};
use theme::Appearance;

fn mods() -> Modifiers {
    Modifiers::default()
}

#[test]
fn printables_prefer_key_char() {
    assert_eq!(
        keystroke_bytes("a", Some("a"), &mods(), false),
        Some(b"a".to_vec())
    );
    assert_eq!(
        keystroke_bytes(
            "a",
            Some("A"),
            &Modifiers {
                shift: true,
                ..mods()
            },
            false
        ),
        Some(b"A".to_vec())
    );
    // Multi-byte characters pass through as UTF-8.
    assert_eq!(
        keystroke_bytes("e", Some("é"), &mods(), false),
        Some("é".as_bytes().to_vec())
    );
    // Named single-char keys fall back to the key name.
    assert_eq!(
        keystroke_bytes("/", None, &mods(), false),
        Some(b"/".to_vec())
    );
    // Unknown multi-char keys are not ours.
    assert_eq!(keystroke_bytes("capslock", None, &mods(), false), None);
}

#[test]
fn control_keys_and_sequences() {
    assert_eq!(
        keystroke_bytes("enter", None, &mods(), false),
        Some(b"\r".to_vec())
    );
    assert_eq!(
        keystroke_bytes("backspace", None, &mods(), false),
        Some(vec![0x7f])
    );
    assert_eq!(
        keystroke_bytes("tab", None, &mods(), false),
        Some(b"\t".to_vec())
    );
    assert_eq!(
        keystroke_bytes(
            "tab",
            None,
            &Modifiers {
                shift: true,
                ..mods()
            },
            false
        ),
        Some(b"\x1b[Z".to_vec())
    );
    assert_eq!(
        keystroke_bytes("escape", None, &mods(), false),
        Some(vec![0x1b])
    );
    assert_eq!(
        keystroke_bytes("delete", None, &mods(), false),
        Some(b"\x1b[3~".to_vec())
    );
    assert_eq!(
        keystroke_bytes("pageup", None, &mods(), false),
        Some(b"\x1b[5~".to_vec())
    );
    assert_eq!(
        keystroke_bytes("f5", None, &mods(), false),
        Some(b"\x1b[15~".to_vec())
    );
}

#[test]
fn arrows_respect_app_cursor_mode() {
    assert_eq!(
        keystroke_bytes("up", None, &mods(), false),
        Some(b"\x1b[A".to_vec())
    );
    assert_eq!(
        keystroke_bytes("up", None, &mods(), true),
        Some(b"\x1bOA".to_vec())
    );
    assert_eq!(
        keystroke_bytes("home", None, &mods(), false),
        Some(b"\x1b[H".to_vec())
    );
    assert_eq!(
        keystroke_bytes("end", None, &mods(), true),
        Some(b"\x1bOF".to_vec())
    );
}

#[test]
fn ctrl_combos_map_to_control_bytes() {
    let ctrl = Modifiers {
        control: true,
        ..mods()
    };
    assert_eq!(
        keystroke_bytes("c", Some("c"), &ctrl, false),
        Some(vec![0x03])
    );
    assert_eq!(keystroke_bytes("z", None, &ctrl, false), Some(vec![0x1a]));
    assert_eq!(
        keystroke_bytes("space", None, &ctrl, false),
        Some(vec![0x00])
    );
    assert_eq!(keystroke_bytes("[", None, &ctrl, false), Some(vec![0x1b]));
    assert_eq!(keystroke_bytes("_", None, &ctrl, false), Some(vec![0x1f]));
    // Ctrl+1 has no caret encoding — not ours.
    assert_eq!(keystroke_bytes("1", Some("1"), &ctrl, false), None);
}

#[test]
fn alt_prefixes_escape() {
    let alt = Modifiers {
        alt: true,
        ..mods()
    };
    assert_eq!(
        keystroke_bytes("b", Some("b"), &alt, false),
        Some(vec![0x1b, b'b'])
    );
    let alt_ctrl = Modifiers {
        alt: true,
        control: true,
        ..mods()
    };
    assert_eq!(
        keystroke_bytes("c", None, &alt_ctrl, false),
        Some(vec![0x1b, 0x03])
    );
}

#[test]
fn platform_primary_combos_fall_through() {
    let cmd = Modifiers {
        platform: true,
        ..mods()
    };
    assert_eq!(keystroke_bytes("j", Some("j"), &cmd, false), None);
    assert_eq!(keystroke_bytes("enter", None, &cmd, false), None);
}

#[test]
fn paste_wraps_when_bracketed() {
    assert_eq!(paste_bytes("hi", false), b"hi".to_vec());
    assert_eq!(paste_bytes("hi", true), b"\x1b[200~hi\x1b[201~".to_vec());
    // Close-bracket injection is stripped.
    assert_eq!(
        paste_bytes("a\x1b[201~rm -rf", true),
        b"\x1b[200~arm -rf\x1b[201~".to_vec()
    );
}

#[test]
fn coalescer_schedules_once_per_burst() {
    let mut c = InputCoalescer::default();
    assert!(c.is_empty());
    assert!(c.push(b"a"), "first push schedules the flush");
    assert!(!c.push(b"b"), "subsequent pushes ride the pending flush");
    assert!(!c.push(b"c"));
    assert_eq!(c.take(), b"abc".to_vec());
    assert!(c.is_empty());
    // Next burst schedules again.
    assert!(c.push(b"d"));
    // Empty pushes never schedule.
    let mut c = InputCoalescer::default();
    assert!(!c.push(b""));
}

/// Relative luminance, for contrast assertions (WCAG 2.x definition).
fn luminance(r: u8, g: u8, b: u8) -> f32 {
    let f = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}

fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let (la, lb) = (luminance(a.0, a.1, a.2), luminance(b.0, b.1, b.2));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn cube_is_appearance_independent() {
    for appearance in [Appearance::Dark, Appearance::Light] {
        // 16 = cube origin (0,0,0); 231 = cube max (255,255,255).
        assert_eq!(indexed_rgb(appearance, 16), (0, 0, 0));
        assert_eq!(indexed_rgb(appearance, 231), (255, 255, 255));
        // 196 = pure red corner: 16 + 36*5.
        assert_eq!(indexed_rgb(appearance, 196), (255, 0, 0));
        // 21 = pure blue corner.
        assert_eq!(indexed_rgb(appearance, 21), (0, 0, 255));
    }
}

#[test]
fn grayscale_ramp_mirrors_in_light() {
    // Dark: 232 → 8 (faintest), 255 → 238 (strongest).
    assert_eq!(indexed_rgb(Appearance::Dark, 232), (8, 8, 8));
    assert_eq!(indexed_rgb(Appearance::Dark, 255), (238, 238, 238));
    // Light reverses it so the faint end stays faint against white.
    assert_eq!(indexed_rgb(Appearance::Light, 232), (238, 238, 238));
    assert_eq!(indexed_rgb(Appearance::Light, 255), (8, 8, 8));
}

#[test]
fn ansi_range_hits_the_appearance_table() {
    assert_eq!(indexed_rgb(Appearance::Dark, 1), (0xf8, 0x71, 0x71));
    assert_eq!(indexed_rgb(Appearance::Light, 1), (0xdc, 0x26, 0x26));
}

#[test]
fn terminal_bg_tracks_the_appearance() {
    let dark = terminal_bg_for(Appearance::Dark);
    assert_eq!(dark.s, 0.0);
    assert!((dark.l - 9.0 / 255.0).abs() < 1e-4);

    let light = terminal_bg_for(Appearance::Light);
    assert_eq!(light.s, 0.0);
    assert!((light.l - 250.0 / 255.0).abs() < 1e-4);
}

/// Slot 0 is "black" — the one slot whose job is to sit *at* the dark end
/// of the scale, not to be readable text. It draws box edges and shaded
/// blocks, so it gets the grey floor below rather than the text floor.
const ANSI_BLACK: usize = 0;

/// The regression the light table exists to prevent: every ANSI slot has to be
/// readable on its *own* background. The dark table on the light background is
/// what the bug looked like.
///
/// Two floors, because the table holds two kinds of thing. The twelve
/// chromatic slots carry text and clear AA for the 13px mono the grid paints
/// at. The black/bright-black pair are structural greys — box drawing and dim
/// hints — and the *dark* palette, which shipped tuned by eye and is not what
/// this change touches, puts bright-black at 2.58:1. Holding them to the text
/// floor would fail the palette that was already correct, so they only have to
/// separate from the background.
#[test]
fn every_ansi_slot_is_legible_on_its_background() {
    const MIN_TEXT: f32 = 3.0;
    const MIN_GREY: f32 = 1.25;
    let cases = [
        (Appearance::Dark, (0x09, 0x09, 0x09)),
        (Appearance::Light, (0xfa, 0xfa, 0xfa)),
    ];
    for (appearance, bg) in cases {
        for ix in 0..16u8 {
            let color = indexed_rgb(appearance, ix);
            let grey = ix as usize % 8 == ANSI_BLACK;
            let min = if grey { MIN_GREY } else { MIN_TEXT };
            let ratio = contrast(color, bg);
            assert!(
                ratio >= min,
                "{appearance:?} ANSI {ix} {color:?} is {ratio:.2}:1 on {bg:?}, want {min}:1"
            );
        }
    }
}

/// "Bright" means *more* prominent in both appearances — which is darker on a
/// light field, not lighter. A literal copy of the dark table would fail this
/// in all seven checked slots.
///
/// The black pair is the standing exception: "bright black" is the dim-text
/// grey in every terminal palette, so it steps *lighter* than black in both
/// appearances. Inverting it on light would put the most common dim-text color
/// below `#1f1f1f`, i.e. heavier than body text — the opposite of dim.
#[test]
fn bright_slots_gain_emphasis_in_both_appearances() {
    let lum = |c: (u8, u8, u8)| luminance(c.0, c.1, c.2);
    for ix in 0..8u8 {
        if ix as usize == ANSI_BLACK {
            continue;
        }
        assert!(
            lum(indexed_rgb(Appearance::Dark, ix + 8)) > lum(indexed_rgb(Appearance::Dark, ix)),
            "dark ANSI {ix}: bright should be lighter on near-black"
        );
        assert!(
            lum(indexed_rgb(Appearance::Light, ix + 8)) < lum(indexed_rgb(Appearance::Light, ix)),
            "light ANSI {ix}: bright should be darker on near-white"
        );
    }
    // The exception, asserted rather than merely skipped.
    assert!(
        lum(indexed_rgb(Appearance::Dark, 8))
            > lum(indexed_rgb(Appearance::Dark, ANSI_BLACK as u8))
    );
    assert!(
        lum(indexed_rgb(Appearance::Light, 8))
            > lum(indexed_rgb(Appearance::Light, ANSI_BLACK as u8))
    );
}

#[test]
fn timing_constants_match_spec() {
    assert_eq!(COALESCE_MS, 12);
    assert_eq!(RESIZE_DEBOUNCE_MS, 80);
}

// ---- pointer → cell ----

/// 10x20 cells, an 8x4 grid: cols 0..7, rows 0..3.
fn hit(x: f32, y: f32) -> CellHit {
    cell_at(x, y, 10.0, 20.0, 8, 4)
}

#[test]
fn pointer_maps_to_the_cell_it_is_over() {
    assert_eq!(
        hit(0.0, 0.0),
        CellHit {
            row: 0,
            col: 0,
            side: Side::Left
        }
    );
    assert_eq!(
        hit(25.0, 45.0),
        CellHit {
            row: 2,
            col: 2,
            side: Side::Left
        }
    );
    // Last cell, exactly.
    assert_eq!(
        hit(70.0, 60.0),
        CellHit {
            row: 3,
            col: 7,
            side: Side::Left
        }
    );
}

#[test]
fn side_splits_the_cell_at_its_midpoint() {
    // Cell 2 spans x 20..30, so the midpoint is 25.
    assert_eq!(hit(21.0, 0.0).side, Side::Left);
    assert_eq!(
        hit(25.0, 0.0).side,
        Side::Left,
        "the midpoint itself is left"
    );
    assert_eq!(hit(26.0, 0.0).side, Side::Right);
    // The cell is unaffected by which half.
    assert_eq!(hit(21.0, 0.0).col, 2);
    assert_eq!(hit(29.0, 0.0).col, 2);
}

/// Dragging out of the panel must extend to the edge it left through, not
/// freeze at the last sample inside.
#[test]
fn overshoot_clamps_into_the_grid() {
    assert_eq!(hit(9_999.0, 0.0).col, 7);
    assert_eq!(hit(0.0, 9_999.0).row, 3);
    assert_eq!(hit(-50.0, 0.0).col, 0);
    assert_eq!(hit(0.0, -50.0).row, 0);
}

/// The part clamping alone does not give you: past the right or bottom edge the
/// side is forced Right so the last cell is *included*, and above the top it is
/// forced Left. Dragging below-and-left must still take the bottom row whole.
#[test]
fn overshoot_forces_the_side_to_the_edge() {
    assert_eq!(hit(9_999.0, 10.0).side, Side::Right);
    // x sits in cell 0's left half, but the row overshot — Right wins.
    assert_eq!(hit(1.0, 9_999.0).side, Side::Right);
    assert_eq!(hit(1.0, 9_999.0).col, 0);
    // Above the top, mirrored.
    assert_eq!(hit(75.0, -50.0).side, Side::Left);
}

#[test]
fn degenerate_metrics_do_not_panic() {
    assert_eq!(
        cell_at(5.0, 5.0, 0.0, 20.0, 8, 4),
        CellHit {
            row: 0,
            col: 0,
            side: Side::Left
        }
    );
    assert_eq!(
        cell_at(5.0, 5.0, 10.0, 20.0, 0, 0),
        CellHit {
            row: 0,
            col: 0,
            side: Side::Left
        }
    );
    assert_eq!(cell_at(f32::NAN, f32::INFINITY, 10.0, 20.0, 8, 4).col, 0);
}

#[test]
fn drag_threshold_matches_the_gpui_default() {
    assert_eq!(SELECTION_DRAG_THRESHOLD, 2.0);
}

/// The selection veil must stay achromatic, or it tints the ANSI text it
/// covers instead of just lifting it.
#[test]
fn selection_wash_is_neutral_and_translucent() {
    for appearance in [Appearance::Dark, Appearance::Light] {
        let wash = terminal_selection_for(appearance);
        assert_eq!(wash.s, 0.0, "{appearance:?} selection must have no hue");
        assert!(
            wash.a > 0.0 && wash.a < 0.5,
            "{appearance:?} selection must veil the cell, not replace it"
        );
    }
    // Opposite directions: lighten the dark grid, darken the light one.
    assert_eq!(terminal_selection_for(Appearance::Dark).l, 1.0);
    assert_eq!(terminal_selection_for(Appearance::Light).l, 0.0);
}
