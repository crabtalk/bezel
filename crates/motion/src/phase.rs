//! Loader math — the pure phase functions behind the loading indicators.
//!
//! These are the curves and constants the gpui viewport animates with
//! (`bezel-motion`, `bezel::loaders`), kept pure so any
//! surface animates the *same* loaders rather than inventing its own spinner.
//! A loading indicator is a brand surface; two of them that disagree read as
//! two products.
//!
//! Everything is a pure function of a phase in `0..1`, so a caller can drive
//! it from a frame delta or from wall-clock elapsed time and get identical
//! output.

/// Pulse loader period.
pub const PULSE_MS: u64 = 2_400;
/// Gradient matrix spinner wave period.
pub const GRADIENT_SPIN_MS: u64 = 750;

/// Cells in the pulse wave loader.
pub const PULSE_CELLS: usize = 5;
/// Side length of the gradient spinner matrix.
pub const MATRIX_SIDE: usize = 3;

/// Pulse loader cells rest at this opacity between pulses.
pub const PULSE_MIN_OPACITY: f32 = 0.08;
/// …and at this scale.
pub const PULSE_MIN_SCALE: f32 = 0.9;
/// Per-cell stagger, as a fraction of the pulse period (0.15s of 2.4s).
pub const PULSE_STAGGER: f32 = 0.15 / 2.4;

/// Per-row tints of the gradient matrix spinner — a "sunrise" gradient
/// sampled at each row: cool blue at the top, through amber, to pink.
pub const GSPIN_ROW_TINTS: [u32; MATRIX_SIDE] = [0xB6D3EF, 0xEDB185, 0xF888A0];
/// Opacity a gradient-spinner cell rests at between pulses.
pub const GSPIN_DIM: f32 = 0.1;

/// Clockwise ring position of each `(row, col)` cell of the 2×3 mini spinner,
/// top-left first: (0,0) → (0,1) → (1,1) → (2,1) → (2,0) → (1,0). Every cell of
/// a 2×3 grid is on the ring, so the brightness chases around it.
pub const MINI_RING: [[usize; 2]; 3] = [[0, 1], [5, 2], [4, 3]];
/// Cells in the mini spinner's ring.
pub const MINI_RING_LEN: f32 = 6.0;

/// Orb period. Slower than either spinner: these breathe rather than tick, and
/// a tick at this size reads as impatience.
pub const ORB_MS: u64 = 2_000;

/// Orbs in the [`ORB_SEATS`] cluster.
pub const ORBS: usize = 3;
/// Where each cluster orb sits, as `(x, y)` fractions of the box — a triangle,
/// so the group reads as one object and still fills a square slot.
pub const ORB_SEATS: [(f32, f32); ORBS] = [(0.36, 0.36), (0.64, 0.32), (0.48, 0.66)];
/// A cluster orb's diameter at its smallest and largest, as fractions of the
/// box. The swing is the whole point: at the bottom an orb is nearly gone, so
/// the *count* you perceive changes as they trade places. A fixed size would
/// leave the silhouette constant and the thing would read as three dots
/// dimming.
pub const ORB_MIN_SIZE: f32 = 0.14;
pub const ORB_MAX_SIZE: f32 = 0.62;
/// How far a cluster orb wanders from its seat, as a fraction of the box. Wide
/// enough that neighbours overlap at one end of the walk and separate at the
/// other.
pub const ORB_DRIFT: f32 = 0.13;
/// An orb never goes out entirely — the cluster dims, it does not blink.
pub const ORB_MIN_OPACITY: f32 = 0.35;
/// Glow radius at rest and at full breath, as fractions of the box.
pub const ORB_GLOW_MIN: f32 = 0.10;
pub const ORB_GLOW_MAX: f32 = 0.40;

/// Dots in the [`orb_ring_seat`] ring.
pub const ORB_RING_DOTS: usize = 8;
/// The ring's radius and its dot diameter, as fractions of the box.
pub const ORB_RING_RADIUS: f32 = 0.34;
pub const ORB_RING_DOT: f32 = 0.16;

/// Rings in flight at once in the bloom.
pub const ORB_BLOOM_RINGS: usize = 3;
/// Where a bloom ring starts and ends, as fractions of the box.
pub const ORB_BLOOM_MIN: f32 = 0.16;
pub const ORB_BLOOM_MAX: f32 = 1.0;

// The swing has to be visible or the cluster is three dots dimming: an orb at
// the trough is at most a quarter the diameter of one at the crest. A compile
// error rather than a test, since both sides are constants.
const _: () = assert!(ORB_MAX_SIZE > ORB_MIN_SIZE * 4.0);

/// One cluster orb's opacity as it breathes: [`ORB_MIN_OPACITY`] → 1 → back.
pub fn orb_opacity(phase: f32) -> f32 {
    lerp(ORB_MIN_OPACITY, 1.0, pulse_wave(phase))
}

/// One cluster orb's diameter, as a fraction of the box.
pub fn orb_size(phase: f32) -> f32 {
    lerp(ORB_MIN_SIZE, ORB_MAX_SIZE, pulse_wave(phase))
}

/// One cluster orb's glow radius, as a fraction of the box. In step with the
/// opacity, because a glow that peaks off-beat reads as two lights rather than
/// one breathing.
pub fn orb_glow(phase: f32) -> f32 {
    lerp(ORB_GLOW_MIN, ORB_GLOW_MAX, pulse_wave(phase))
}

/// How far a cluster orb has drifted from its seat, as `(dx, dy)` fractions of
/// the box — a small circle walked once per period.
///
/// Being a circle, the drift returns exactly to zero every period, so nothing
/// accumulates however long it runs.
pub fn orb_drift(phase: f32) -> (f32, f32) {
    let angle = phase * std::f32::consts::TAU;
    (ORB_DRIFT * angle.cos(), ORB_DRIFT * angle.sin())
}

/// Where ring dot `index` sits on a circle of `radius`, as `(x, y)` fractions
/// of the box. Twelve o'clock first, going clockwise.
///
/// The radius is an argument because two shapes want the same circle: the ring
/// holds it still and the converge pulses it. Two functions here would be one
/// function and a number.
pub fn orb_ring_seat(index: usize, radius: f32) -> (f32, f32) {
    let angle =
        index as f32 / ORB_RING_DOTS as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    (0.5 + radius * angle.cos(), 0.5 + radius * angle.sin())
}

/// The converge's radius at `phase`: in to nothing, back out to the ring.
///
/// Every dot shares this one radius, so they arrive at the centre together and
/// stack into a single point — the frame that makes this read as a gathering
/// rather than as a ring that shrank.
pub fn orb_converge_radius(phase: f32) -> f32 {
    lerp(0.0, ORB_RING_RADIUS, pulse_wave(phase))
}

/// A bloom ring's radius, as a fraction of the box: out from
/// [`ORB_BLOOM_MIN`] to [`ORB_BLOOM_MAX`] once per period.
pub fn orb_bloom_radius(phase: f32) -> f32 {
    lerp(ORB_BLOOM_MIN, ORB_BLOOM_MAX, phase.rem_euclid(1.0))
}

/// A bloom ring's opacity: full as it leaves the centre, gone by the edge —
/// squared, so it holds its brightness through the middle of the travel and
/// then goes quickly, which is what keeps the ring from looking like a
/// dissolving circle.
pub fn orb_bloom_opacity(phase: f32) -> f32 {
    let t = 1.0 - phase.rem_euclid(1.0);
    t * t
}

/// Linear interpolation.
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// A cell's phase, given the loader's raw phase and the cell's index.
pub fn staggered_phase(raw_delta: f32, index: usize, stagger: f32) -> f32 {
    (raw_delta - index as f32 * stagger).rem_euclid(1.0)
}

/// Cosine pulse: 0 at phase 0, 1 at phase 0.5, back to 0 at phase 1.
pub fn pulse_wave(phase: f32) -> f32 {
    0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()
}

/// Pulse loader cell opacity for a phase: 0.08 → 1 → 0.08.
pub fn pulse_opacity(phase: f32) -> f32 {
    PULSE_MIN_OPACITY + (1.0 - PULSE_MIN_OPACITY) * pulse_wave(phase)
}

/// Pulse loader cell scale for a phase: 0.9 → 1 → 0.9.
pub fn pulse_scale(phase: f32) -> f32 {
    PULSE_MIN_SCALE + (1.0 - PULSE_MIN_SCALE) * pulse_wave(phase)
}

/// Gradient-spin cell opacity for a local phase `t` (0..1 of the period),
/// ported from the `gradient-spin-pulse` keyframes: full at the cycle
/// start, easing down to `dim` by 45%, resting at `dim` until 92%, then rising
/// back to full — the per-cell phase offset sweeps this pulse across the grid.
pub fn gspin_opacity(t: f32, dim: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    if t < 0.45 {
        lerp(1.0, dim, t / 0.45)
    } else if t < 0.92 {
        dim
    } else {
        lerp(dim, 1.0, (t - 0.92) / 0.08)
    }
}

/// The phase offset of a `(row, col)` cell in the 3×3 gradient spinner: the
/// pulse enters at the bottom edge and converges toward the top-centre cell, so
/// the wave reads as travelling upward.
pub fn gspin_cell_phase(row: usize, col: usize) -> f32 {
    let centre = (MATRIX_SIDE as f32 - 1.0) / 2.0;
    let max = MATRIX_SIDE as f32 - 1.0 + centre;
    let d = MATRIX_SIDE as f32 - 1.0 - row as f32 + (col as f32 - centre).abs();
    if max == 0.0 { 0.0 } else { d / (max + 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                (staggered_phase(0.0, index, stagger) - staggered_phase(0.0, 0, stagger)).abs()
                    > 0.2,
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
}
