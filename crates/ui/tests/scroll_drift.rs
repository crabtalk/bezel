use gpui::{Pixels, px};

use ui::scroll::*;

/// A pane with room either side of both edge strips.
const START: Pixels = px(0.0);
const END: Pixels = px(600.0);

#[test]
fn the_middle_of_a_pane_is_still() {
    assert_eq!(drift_velocity(px(300.0), START, END), 0.0);
    // The edge strip is exclusive at its inner lip: a hair further in is
    // still, and the boundary itself is too.
    assert_eq!(drift_velocity(DRIFT_EDGE, START, END), 0.0);
    assert_eq!(drift_velocity(END - DRIFT_EDGE, START, END), 0.0);
}

#[test]
fn each_edge_travels_the_way_it_is_reached() {
    // gpui's offset goes negative as a pane scrolls on, so travelling back
    // toward the start is positive and travelling on is negative.
    assert_eq!(drift_velocity(START, START, END), DRIFT_SPEED);
    assert_eq!(drift_velocity(END, START, END), -DRIFT_SPEED);
}

#[test]
fn the_ramp_eases_with_proximity() {
    // Halfway into the strip, half the speed — easing toward the edge eases
    // the scroll rather than switching it on.
    let half = drift_velocity(DRIFT_EDGE / 2.0, START, END);
    assert!((half - DRIFT_SPEED / 2.0).abs() < 1.0, "{half}");
    let half = drift_velocity(END - DRIFT_EDGE / 2.0, START, END);
    assert!((half + DRIFT_SPEED / 2.0).abs() < 1.0, "{half}");
}

#[test]
fn past_the_edge_holds_full_speed() {
    // A card carried off the side of the board keeps it coming. Without this
    // the drift stops at the boundary, which is the one place a pointer
    // overshooting the pane always is.
    assert_eq!(drift_velocity(px(-400.0), START, END), DRIFT_SPEED);
    assert_eq!(drift_velocity(px(4000.0), START, END), -DRIFT_SPEED);
}

#[test]
fn a_pane_narrower_than_two_edges_still_has_a_middle() {
    // The strips would overlap, and a midpoint inside both is a midpoint
    // where the direction flips at half speed — the pane creeping under a
    // pointer that is nowhere near an edge.
    let (start, end) = (px(0.0), px(100.0));
    assert_eq!(drift_velocity(px(50.0), start, end), 0.0);
    assert_eq!(drift_velocity(px(0.0), start, end), DRIFT_SPEED);
    assert_eq!(drift_velocity(px(100.0), start, end), -DRIFT_SPEED);
}

#[test]
fn nothing_to_drift_in_is_never_drifting() {
    // The frame before layout has run, and a pane laid out backwards.
    assert_eq!(drift_velocity(px(0.0), px(0.0), px(0.0)), 0.0);
    assert_eq!(drift_velocity(px(10.0), px(600.0), px(0.0)), 0.0);
}
