use bezel_motion::phase::*;

fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 1e-5, "{what}: {a} vs {b}");
}

#[test]
fn the_pulse_is_a_full_cosine_cycle() {
    close(pulse_wave(0.0), 0.0, "trough at 0");
    close(pulse_wave(0.5), 1.0, "crest at half");
    close(pulse_wave(1.0), 0.0, "trough at 1");
    // Opacity and scale ride the same wave between their own bounds.
    close(pulse_opacity(0.0), PULSE_MIN_OPACITY, "dim rest");
    close(pulse_opacity(0.5), 1.0, "full crest");
    close(pulse_scale(0.0), PULSE_MIN_SCALE, "small rest");
    close(pulse_scale(0.5), 1.0, "full scale");
}

#[test]
fn stagger_offsets_each_cell_and_wraps() {
    close(staggered_phase(0.0, 0, PULSE_STAGGER), 0.0, "cell 0");
    close(
        staggered_phase(0.0, 1, PULSE_STAGGER),
        1.0 - PULSE_STAGGER,
        "cell 1 trails into the previous cycle",
    );
    // Phase is periodic: a whole extra turn changes nothing.
    close(
        staggered_phase(0.3, 2, PULSE_STAGGER),
        staggered_phase(1.3, 2, PULSE_STAGGER),
        "wraps",
    );
    // Always inside the unit interval, for any input.
    for raw in [-4.2f32, -0.1, 0.0, 0.5, 7.9] {
        for index in 0..PULSE_CELLS {
            let phase = staggered_phase(raw, index, PULSE_STAGGER);
            assert!((0.0..1.0).contains(&phase), "{raw} {index} -> {phase}");
        }
    }
}

#[test]
fn an_orb_breathes_without_ever_going_out() {
    close(orb_opacity(0.0), ORB_MIN_OPACITY, "dimmest at the trough");
    close(orb_opacity(0.5), 1.0, "full at the crest");
    close(orb_glow(0.0), ORB_GLOW_MIN, "tightest at the trough");
    close(orb_glow(0.5), ORB_GLOW_MAX, "widest at the crest");
    // The floor is the point: a cluster that reached zero would blink, and
    // three blinking dots are a spinner again.
    for step in 0..200 {
        let value = orb_opacity(step as f32 / 100.0);
        assert!(
            (ORB_MIN_OPACITY..=1.0).contains(&value),
            "{step} -> {value}"
        );
    }
}

#[test]
fn the_cluster_changes_shape_and_not_just_brightness() {
    // The whole difference between a cluster and three dots dimming: an orb
    // at the trough is a quarter the diameter of one at the crest, so the
    // silhouette is different in every frame.
    close(orb_size(0.0), ORB_MIN_SIZE, "smallest at the trough");
    close(orb_size(0.5), ORB_MAX_SIZE, "largest at the crest");
    // And they never crest together — a third of a period apart each, so
    // one is always growing while another shrinks.
    let stagger = 1.0 / ORBS as f32;
    for index in 1..ORBS {
        assert!(
            (staggered_phase(0.0, index, stagger) - staggered_phase(0.0, 0, stagger)).abs() > 0.2,
            "orb {index} crests with orb 0"
        );
    }
}

#[test]
fn the_drift_is_a_closed_circle() {
    // Back exactly where it started every period — nothing accumulates,
    // however long the model thinks for.
    close(orb_drift(0.0).0, ORB_DRIFT, "starts right of the seat");
    close(orb_drift(0.0).1, 0.0, "…and level with it");
    close(orb_drift(1.0).0, orb_drift(0.0).0, "x returns");
    close(orb_drift(1.0).1, orb_drift(0.0).1, "y returns");
    close(orb_drift(0.5).0, -ORB_DRIFT, "opposite at half");
    // Never further from the seat than the drift radius, at any phase.
    for step in 0..200 {
        let (dx, dy) = orb_drift(step as f32 / 100.0);
        assert!(dx.hypot(dy) <= ORB_DRIFT + 1e-5, "{step} -> {dx},{dy}");
    }
}

#[test]
fn the_cluster_both_merges_and_separates() {
    // At the top of the walk neighbours overlap into one mass; at the
    // bottom they stand apart. A cluster that only ever did one of those
    // would be a blob or three dots, and the point is that it is both.
    let gap = ORB_SEATS
        .iter()
        .enumerate()
        .flat_map(|(i, a)| {
            ORB_SEATS
                .iter()
                .skip(i + 1)
                .map(move |b| (a.0 - b.0).hypot(a.1 - b.1))
        })
        .fold(f32::MAX, f32::min);
    assert!(
        gap < ORB_MAX_SIZE + 2.0 * ORB_DRIFT,
        "never touch: {gap} apart"
    );
    assert!(gap > ORB_MIN_SIZE, "never apart: {gap}");
}

#[test]
fn the_ring_is_a_circle_starting_at_noon() {
    let (x, y) = orb_ring_seat(0, ORB_RING_RADIUS);
    close(x, 0.5, "first dot is centred horizontally");
    close(y, 0.5 - ORB_RING_RADIUS, "…and at the top");
    // Quarter way round is three o'clock: clockwise, like a clock.
    let (x, y) = orb_ring_seat(ORB_RING_DOTS / 4, ORB_RING_RADIUS);
    close(x, 0.5 + ORB_RING_RADIUS, "quarter turn is to the right");
    close(y, 0.5, "…and level with the centre");
    // Every dot sits on the circle, and inside the box.
    for index in 0..ORB_RING_DOTS {
        let (x, y) = orb_ring_seat(index, ORB_RING_RADIUS);
        close((x - 0.5).hypot(y - 0.5), ORB_RING_RADIUS, "on the circle");
        assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y));
    }
}

#[test]
fn the_converge_gathers_to_a_single_point() {
    close(orb_converge_radius(0.0), 0.0, "collapsed at the trough");
    close(orb_converge_radius(0.5), ORB_RING_RADIUS, "out to the ring");
    // At the trough every dot is at the centre — the same place, which is
    // what makes the gathered frame one dot rather than eight touching.
    let gathered: Vec<_> = (0..ORB_RING_DOTS)
        .map(|index| orb_ring_seat(index, orb_converge_radius(0.0)))
        .collect();
    for (x, y) in &gathered {
        close(*x, 0.5, "gathered on the centre");
        close(*y, 0.5, "gathered on the centre");
    }
    // …and never further out than the ring it borrows.
    for step in 0..200 {
        let radius = orb_converge_radius(step as f32 / 100.0);
        assert!(
            (0.0..=ORB_RING_RADIUS).contains(&radius),
            "{step} -> {radius}"
        );
    }
}

#[test]
fn a_bloom_ring_leaves_the_centre_and_fades_by_the_edge() {
    close(orb_bloom_radius(0.0), ORB_BLOOM_MIN, "starts small");
    close(orb_bloom_radius(1.0), ORB_BLOOM_MIN, "and wraps back");
    close(orb_bloom_opacity(0.0), 1.0, "full as it leaves");
    close(orb_bloom_opacity(1.0), 1.0, "wraps to full");
    // Gone before it reaches the edge, or the ring would be cut off square
    // by the box rather than fading out of it.
    assert!(orb_bloom_opacity(0.95) < 0.01, "still visible at the rim");
    // Monotone outward: a ring never travels back toward the centre.
    for step in 0..99 {
        let (a, b) = (step as f32 / 100.0, (step + 1) as f32 / 100.0);
        assert!(
            orb_bloom_radius(a) < orb_bloom_radius(b),
            "{step} goes back"
        );
    }
}

#[test]
fn gradient_spin_holds_dim_then_snaps_back() {
    close(gspin_opacity(0.0, GSPIN_DIM), 1.0, "starts full");
    close(gspin_opacity(0.45, GSPIN_DIM), GSPIN_DIM, "down by 45%");
    close(gspin_opacity(0.7, GSPIN_DIM), GSPIN_DIM, "rests dim");
    close(gspin_opacity(1.0, GSPIN_DIM), 1.0, "back to full");
    // Never leaves its bounds, at any phase.
    for step in 0..200 {
        let value = gspin_opacity(step as f32 / 100.0, GSPIN_DIM);
        assert!((GSPIN_DIM..=1.0).contains(&value), "{step} -> {value}");
    }
}

#[test]
fn the_gradient_wave_travels_upward() {
    // The bottom row leads and the top-centre cell trails, which is what
    // makes the pulse read as rising.
    let bottom = gspin_cell_phase(MATRIX_SIDE - 1, 1);
    let top = gspin_cell_phase(0, 1);
    assert!(bottom < top, "bottom {bottom} should lead top {top}");
    // Symmetric about the centre column.
    close(gspin_cell_phase(1, 0), gspin_cell_phase(1, 2), "symmetry");
}

#[test]
fn the_mini_ring_visits_every_cell_once() {
    let mut seen: Vec<usize> = MINI_RING.iter().flatten().copied().collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..MINI_RING_LEN as usize).collect::<Vec<_>>());
}
