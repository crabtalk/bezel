use gpui::{Pixels, px};

use ui::scroll::*;

/// A 400px window onto 1600px of content: 1200 of overflow.
const VIEWPORT: Pixels = px(400.0);
const MAX: Pixels = px(1200.0);

#[test]
fn a_thumb_is_as_long_as_the_visible_share() {
    // 400 of 1600 visible — a quarter of the track.
    let range = thumb(VIEWPORT, MAX, px(0.0), MIN_THUMB).expect("scrollable");
    assert_eq!(range.start, px(0.0));
    assert_eq!(range.end - range.start, px(100.0));
}

#[test]
fn the_thumb_travels_the_track_and_no_further() {
    let size = px(100.0);
    // Scrolled to the end: the thumb's END meets the track's, so its top
    // stops a thumb's length short.
    let range = thumb(VIEWPORT, MAX, -MAX, MIN_THUMB).expect("scrollable");
    assert_eq!(range.start, VIEWPORT - size);
    assert_eq!(range.end, VIEWPORT);
    // Halfway.
    let range = thumb(VIEWPORT, MAX, -(MAX / 2.0), MIN_THUMB).expect("scrollable");
    assert_eq!(range.start, (VIEWPORT - size) / 2.0);
}

#[test]
fn an_overshot_offset_is_clamped_rather_than_escaping() {
    // A wheel can push past either end before the container clamps.
    let past = thumb(VIEWPORT, MAX, px(-9000.0), MIN_THUMB).expect("scrollable");
    assert_eq!(
        past,
        thumb(VIEWPORT, MAX, -MAX, MIN_THUMB).expect("scrollable")
    );
    let before = thumb(VIEWPORT, MAX, px(500.0), MIN_THUMB).expect("scrollable");
    assert_eq!(before.start, px(0.0));
}

#[test]
fn nothing_to_scroll_means_no_bar() {
    assert!(thumb(VIEWPORT, px(0.0), px(0.0), MIN_THUMB).is_none());
    // The frame before layout has run.
    assert!(thumb(px(0.0), MAX, px(0.0), MIN_THUMB).is_none());
    // A viewport shorter than the minimum thumb: all thumb, no travel.
    assert!(thumb(px(20.0), MAX, px(0.0), MIN_THUMB).is_none());
}

#[test]
fn dragging_the_thumb_back_gives_the_offset_that_drew_it() {
    // The round trip both directions have to agree on, or the thumb slides
    // out from under the pointer.
    for offset in [px(0.0), px(-1.0), px(-600.0), -MAX] {
        let range = thumb(VIEWPORT, MAX, offset, MIN_THUMB).expect("scrollable");
        let size = range.end - range.start;
        assert_eq!(
            offset_for_thumb(range.start, VIEWPORT, MAX, size),
            offset,
            "round trip at {offset:?}"
        );
    }
}

#[test]
fn a_clamped_thumb_still_reaches_the_end() {
    // 400px onto 100_000: the geometric thumb would be 1.6px, so the
    // minimum takes over — and the round trip has to survive that, since
    // travel is now measured against the clamped size and not the real one.
    let max = px(99_600.0);
    let range = thumb(VIEWPORT, max, -max, MIN_THUMB).expect("scrollable");
    assert_eq!(range.end - range.start, MIN_THUMB);
    assert_eq!(range.end, VIEWPORT);
    assert_eq!(
        offset_for_thumb(range.start, VIEWPORT, max, MIN_THUMB),
        -max
    );
}

#[test]
fn following_means_within_slack_of_the_end() {
    assert!(at_bottom(MAX, -MAX, FOLLOW_SLACK), "exactly at the end");
    assert!(at_bottom(MAX, px(-1197.0), FOLLOW_SLACK), "3px short");
    assert!(!at_bottom(MAX, px(-1190.0), FOLLOW_SLACK), "10px short");
    assert!(!at_bottom(MAX, px(0.0), FOLLOW_SLACK), "at the top");
}

#[test]
fn content_that_fits_is_always_at_the_bottom() {
    // Nowhere else to be. Answering `false` would unpin an empty log and it
    // would never re-pin, since it can never reach an end that isn't there.
    assert!(at_bottom(px(0.0), px(0.0), FOLLOW_SLACK));
}

#[test]
fn an_overshot_offset_still_reads_as_the_end_it_overshot() {
    // A wheel outruns the clamp for a frame; both ends have to survive it.
    assert!(at_bottom(MAX, px(-9000.0), FOLLOW_SLACK));
    assert!(!at_bottom(MAX, px(500.0), FOLLOW_SLACK));
}

#[test]
fn a_fresh_follow_state_is_pinned() {
    // A transcript opens on its newest line, not its oldest.
    let state = FollowState::new();
    assert!(state.following());
}

#[test]
fn a_drag_past_either_end_stops_there() {
    assert_eq!(
        offset_for_thumb(px(10_000.0), VIEWPORT, MAX, px(100.0)),
        -MAX
    );
    assert_eq!(
        offset_for_thumb(px(-10_000.0), VIEWPORT, MAX, px(100.0)),
        px(0.0)
    );
    // Degenerate: a thumb filling its track has nowhere to go.
    assert_eq!(offset_for_thumb(px(50.0), VIEWPORT, MAX, VIEWPORT), px(0.0));
}
