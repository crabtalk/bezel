//! Coalescing is the whole design, so it is what these pin: a run of typing is
//! one step, and anything that breaks the run starts another.

use editor::{EditKind, History};
use markdown::{parse::parse, *};

fn body(block: usize, offset: usize) -> Selection {
    Selection::at(Cursor::new(block, Part::Body, offset))
}

/// Type `text` one character at a time, recording each keystroke the way the
/// editor does.
fn type_out(history: &mut History, doc: &mut Doc, mut at: Selection, text: &str) -> Selection {
    for ch in text.chars() {
        history.record(EditKind::Insert, doc, at);
        let head = doc.replace(at, Text::plain(ch.to_string()));
        at = Selection::at(head);
        history.landed(EditKind::Insert, at);
    }
    at
}

#[test]
fn a_run_of_typing_undoes_as_one_step() {
    let mut history = History::default();
    let mut doc = parse("");
    let at = type_out(&mut history, &mut doc, body(0, 0), "hello");
    assert_eq!(serialize(&doc), "hello");

    let (back, _) = history.undo(&doc, at).expect("one step to take");
    assert_eq!(serialize(&back), "", "the whole word came back at once");
    assert!(
        history.undo(&back, at).is_none(),
        "and it was the only step"
    );
}

#[test]
fn a_motion_between_two_runs_makes_two_steps() {
    let mut history = History::default();
    let mut doc = parse("");
    let at = type_out(&mut history, &mut doc, body(0, 0), "one");
    // Anything that is not an edit ends the group.
    history.interrupt();
    let at = type_out(&mut history, &mut doc, at, "two");
    assert_eq!(serialize(&doc), "onetwo");

    let (back, _) = history.undo(&doc, at).expect("the second run");
    assert_eq!(serialize(&back), "one");
    let (back, _) = history.undo(&back, at).expect("the first run");
    assert_eq!(serialize(&back), "");
}

#[test]
fn typing_somewhere_else_starts_a_new_step() {
    let mut history = History::default();
    let mut doc = parse("alpha\n\nbeta");
    let at = type_out(&mut history, &mut doc, body(0, 5), "X");
    // No interrupt — the caret simply is not where the last edit left it.
    let jumped = body(1, 4);
    let at2 = type_out(&mut history, &mut doc, jumped, "Y");
    assert_eq!(serialize(&doc), "alphaX\n\nbetaY");

    let (back, _) = history.undo(&doc, at2).expect("the second block");
    assert_eq!(serialize(&back), "alphaX\n\nbeta");
    let (back, _) = history.undo(&back, at).expect("the first");
    assert_eq!(serialize(&back), "alpha\n\nbeta");
}

#[test]
fn a_structural_edit_never_coalesces() {
    let mut history = History::default();
    let mut doc = parse("ab");
    for _ in 0..2 {
        history.record(EditKind::Structure, &doc, body(0, 1));
        doc.split(0, 1);
        history.landed(EditKind::Structure, body(0, 1));
    }
    assert!(history.undo(&doc, body(0, 1)).is_some());
    // The second step is still there, which a coalesced pair would not leave.
    assert!(history.undo(&doc, body(0, 1)).is_some());
}

#[test]
fn redo_replays_what_undo_took() {
    let mut history = History::default();
    let mut doc = parse("");
    let at = type_out(&mut history, &mut doc, body(0, 0), "text");

    let (back, back_at) = history.undo(&doc, at).unwrap();
    assert_eq!(serialize(&back), "");
    let (forward, _) = history.redo(&back, back_at).expect("a step to replay");
    assert_eq!(serialize(&forward), "text");
}

#[test]
fn a_fresh_edit_drops_the_redo_stack() {
    let mut history = History::default();
    let mut doc = parse("");
    let at = type_out(&mut history, &mut doc, body(0, 0), "one");
    let (back, back_at) = history.undo(&doc, at).unwrap();

    let mut doc = back;
    type_out(&mut history, &mut doc, back_at, "two");
    assert!(
        history.redo(&doc, back_at).is_none(),
        "redo cannot resurrect a branch the document has diverged from"
    );
}

#[test]
fn the_limit_bounds_what_is_kept() {
    let mut history = History::with_limit(3);
    let mut doc = parse("");
    let mut at = body(0, 0);
    for word in ["a", "b", "c", "d", "e"] {
        history.interrupt();
        at = type_out(&mut history, &mut doc, at, word);
    }
    let mut steps = 0;
    let mut state = doc.clone();
    while let Some((back, _)) = history.undo(&state, at) {
        state = back;
        steps += 1;
    }
    assert_eq!(steps, 3, "the oldest steps fall off the front");
}
