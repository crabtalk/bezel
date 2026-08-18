//! Loader math — the pure phase functions behind the loading indicators.
//!
//! These are the curves and constants the gpui viewport animates with
//! (`motion`, `bezel::loaders`), kept pure so any
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
