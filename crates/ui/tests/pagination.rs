use bezel_ui::pagination::*;

fn pages(slots: &[Slot]) -> String {
    slots
        .iter()
        .map(|slot| match slot {
            Slot::Page(page) => page.to_string(),
            Slot::Gap => "…".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_short_run_shows_every_page() {
    assert_eq!(pages(&window(3, 5, 2)), "1 2 3 4 5");
    assert_eq!(pages(&window(1, 3, 2)), "1 2 3");
}

#[test]
fn a_long_run_gaps_both_sides() {
    assert_eq!(pages(&window(6, 20, 2)), "1 … 4 5 6 7 8 … 20");
}

#[test]
fn a_gap_hiding_one_page_shows_the_page_instead() {
    // The window is 3..=7, so page 2 alone sits outside it on the left —
    // "…" would take the same room and say less.
    assert_eq!(pages(&window(5, 20, 2)), "1 2 3 4 5 6 7 … 20");
    // And the mirror, at the other end.
    assert_eq!(pages(&window(16, 20, 2)), "1 … 14 15 16 17 18 19 20");
}

#[test]
fn the_window_slides_at_the_ends_rather_than_shrinking() {
    // Five pages of window at both extremes, not three.
    let first = window(1, 20, 2);
    assert_eq!(pages(&first), "1 2 3 4 5 … 20");
    let last = window(20, 20, 2);
    assert_eq!(pages(&last), "1 … 16 17 18 19 20");
    let middle = window(10, 20, 2);
    assert_eq!(
        window_width(&first),
        window_width(&middle),
        "the run of pages around the current one keeps its width"
    );
    assert_eq!(window_width(&last), window_width(&middle));
}

/// Longest run of consecutive pages — the window, whichever end it is at.
fn window_width(slots: &[Slot]) -> usize {
    let mut best = 0;
    let mut run = 0;
    let mut previous = None;
    for slot in slots {
        match slot {
            Slot::Page(page) if previous == Some(page.saturating_sub(1)) => run += 1,
            Slot::Page(_) => run = 1,
            Slot::Gap => run = 0,
        }
        previous = match slot {
            Slot::Page(page) => Some(*page),
            Slot::Gap => None,
        };
        best = best.max(run);
    }
    best
}

#[test]
fn the_degenerate_runs_answer_something_sane() {
    assert!(window(1, 0, 2).is_empty(), "nothing to page through");
    assert_eq!(pages(&window(1, 1, 2)), "1");
    // `around` of zero still shows where you are and both ends.
    assert_eq!(pages(&window(6, 20, 0)), "1 … 6 … 20");
}

#[test]
fn a_current_page_out_of_range_is_clamped_not_trusted() {
    assert_eq!(pages(&window(0, 20, 2)), pages(&window(1, 20, 2)));
    assert_eq!(pages(&window(999, 20, 2)), pages(&window(20, 20, 2)));
}
