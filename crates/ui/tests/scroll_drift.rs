use gpui::{Bounds, Pixels, Point, point, px, size};

use ui::scroll::*;

/// A pane with room either side of both edge strips, and the window's left
/// half when one has a neighbour.
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

// ---------------------------------------------------------------------------
// What the pane's edge gives onto
// ---------------------------------------------------------------------------

/// A pane occupying the left half of a window, with another beside it.
fn pane() -> Bounds<Pixels> {
    Bounds {
        origin: point(START, px(0.0)),
        size: size(END - START, px(400.0)),
    }
}

/// Along the pane's own row, `into` pixels past its right edge.
fn past(into: f32) -> Point<Pixels> {
    point(END + px(into), px(200.0))
}

#[test]
fn a_pointer_past_the_edge_holds_a_pane_that_owns_it() {
    let velocity = pane_velocity(pane(), past(40.0), Axes::Horizontal, Beyond::Nothing);
    assert_eq!(velocity.x, -DRIFT_SPEED);
}

#[test]
fn a_pointer_that_has_left_for_a_neighbour_drifts_nothing() {
    // The pane would otherwise scroll for the rest of the gesture: past the
    // edge is full speed, and the cross-axis guard cannot help — a left/right
    // split puts the neighbour at the same y.
    let velocity = pane_velocity(pane(), past(40.0), Axes::Horizontal, Beyond::Neighbour);
    assert_eq!(velocity, point(0.0, 0.0));
}

#[test]
fn the_edge_strip_itself_reads_the_same_either_way() {
    // Only the pointer that has actually left is in question; inside the pane
    // the two agree, so the hand-over costs no drift on the way out.
    let inside = point(END - DRIFT_EDGE / 2.0, px(200.0));
    assert_eq!(
        pane_velocity(pane(), inside, Axes::Horizontal, Beyond::Neighbour),
        pane_velocity(pane(), inside, Axes::Horizontal, Beyond::Nothing)
    );
}

#[test]
fn a_pointer_in_another_row_is_not_aiming_here() {
    // Across the axis, with no neighbour anywhere: every lane of a board would
    // drift together on a drag that is only near one of them.
    let below = point(START + px(10.0), px(900.0));
    let velocity = pane_velocity(pane(), below, Axes::Horizontal, Beyond::Nothing);
    assert_eq!(velocity, point(0.0, 0.0));
}
