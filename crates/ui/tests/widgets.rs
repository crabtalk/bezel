use gpui::px;

use ui::widgets::*;

use gpui::{Axis, Bounds, point, size};

fn box_at(left: f32, top: f32, width: f32, height: f32) -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(left), px(top)), size(px(width), px(height)))
}

#[test]
fn axis_fraction_measures_from_the_container_origin() {
    // A container that does not start at the window origin: the fraction is
    // of the container, not of the pointer's absolute position.
    let bounds = box_at(100.0, 40.0, 400.0, 200.0);
    assert_eq!(
        axis_fraction(point(px(300.0), px(60.0)), bounds, Axis::Horizontal, 0.0),
        0.5
    );
    assert_eq!(
        axis_fraction(point(px(200.0), px(60.0)), bounds, Axis::Horizontal, 0.0),
        0.25
    );
    // Vertical splits read y against the height.
    assert_eq!(
        axis_fraction(point(px(300.0), px(90.0)), bounds, Axis::Vertical, 0.0),
        0.25
    );
}

#[test]
fn axis_fraction_never_squeezes_a_pane_away() {
    let bounds = box_at(0.0, 0.0, 400.0, 200.0);
    // Dragged past either end, and even outside the container entirely.
    assert_eq!(
        axis_fraction(point(px(-500.0), px(0.0)), bounds, Axis::Horizontal, 0.2),
        0.2
    );
    assert_eq!(
        axis_fraction(point(px(900.0), px(0.0)), bounds, Axis::Horizontal, 0.2),
        0.8
    );
    // A nonsense minimum still leaves both panes on screen.
    assert_eq!(
        axis_fraction(point(px(0.0), px(0.0)), bounds, Axis::Horizontal, 4.0),
        0.5
    );
}

#[test]
fn a_takeover_follows_the_flag_until_it_is_touched() {
    let mut open = Takeover::default();
    assert!(!open.get(false));
    assert!(open.get(true), "the flag turning on opens it");
    // Touched while open: closed, and the flag is no longer consulted.
    open.toggle(true);
    assert!(!open.get(true));
    assert!(!open.get(false));
}

#[test]
fn the_first_press_flips_what_was_on_screen() {
    // The one thing easy to get backwards: with nothing stored yet, the
    // press flips `auto`, not the default. A header that shows "open"
    // because the work is streaming has to *close* on its first click —
    // flipping the stored `false` would open what is already open.
    let mut auto_open = Takeover::default();
    auto_open.toggle(true);
    assert!(!auto_open.get(true));

    let mut auto_closed = Takeover::default();
    auto_closed.toggle(false);
    assert!(auto_closed.get(false));
}

#[test]
fn a_manual_choice_outlasts_the_run_that_set_it() {
    // Opened by hand while nothing was running; the run starting and
    // finishing must not close it again.
    let mut open = Takeover::default();
    open.toggle(false);
    assert!(open.get(true));
    assert!(open.get(false));
}

#[test]
fn axis_fraction_survives_a_container_with_no_extent() {
    // The frame before layout has run — no divide by zero, no NaN.
    let empty = box_at(0.0, 0.0, 0.0, 0.0);
    assert_eq!(
        axis_fraction(point(px(10.0), px(10.0)), empty, Axis::Horizontal, 0.15),
        0.15
    );
}
