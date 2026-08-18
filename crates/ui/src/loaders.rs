//! Loaders: the orb cluster, the pulse loader and the gradient matrix spinners.
//! All motion routes through `motion` pure helpers, so the math is
//! unit-tested and these elements are testable-by-compile.
//!
//! [`orb`] is bezel's own vocabulary — four shapes over one period — and is
//! what a thinking surface should reach for. The older three are grids of
//! cells.
//!
//! Rendering pattern: each cell is its own `with_animation` repeating element
//! sharing one period; per-cell offsets come from [`motion::staggered_phase`],
//! so all cells stay phase-locked (they start on the same frame) without a
//! shared clock. Cells animate inside fixed-size slots — opacity and inner size
//! are paint-local and never move surrounding layout. Reduced motion snaps every
//! cell to its rest state automatically (gpui `reduce_motion`).

use gpui::{App, EntityId, IntoElement, ParentElement, SharedString, Styled, div, px};

use motion::{self, GRADIENT_SPIN, ORB, PULSE, PULSE_STAGGER};
use theme::Theme;

pub use motion::phase::{GSPIN_DIM, GSPIN_ROW_TINTS, MATRIX_SIDE, PULSE_CELLS};

/// The pulse wave loader: a row of cells pulsing opacity 0.08→1 / scale 0.9→1
/// over 2.4s with a 0.15s stagger per cell.
///
/// `id` scopes the per-cell animation state — give each loader instance a
/// distinct id.
pub fn pulse_loader(
    _id: &'static str,
    theme: &Theme,
    cell_px: f32,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    let color = theme.text;
    let slot = cell_px;
    let delta = motion::pulse_delta(&PULSE, view, cx);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(slot / 2.0))
        .children((0..PULSE_CELLS).map(move |i| {
            // Fixed slot; the animated cell breathes inside it.
            div()
                .size(px(slot))
                .flex()
                .items_center()
                .justify_center()
                .child({
                    let phase = motion::staggered_phase(delta, i, PULSE_STAGGER);
                    div()
                        .rounded(px(slot / 4.0))
                        .bg(color)
                        .opacity(motion::pulse_opacity(phase))
                        .size(px(slot * motion::pulse_scale(phase)))
                })
        }))
}

/// The gradient matrix spinner (working indicator): a 3×3 grid of round cells
/// tinted per row from the sunrise gradient. Each cell pulses opacity once per
/// 750ms period; the per-cell phase follows the "arrow-up" pattern (the pulse
/// enters at the bottom edge and converges toward the top-center cell), so the
/// wave reads as travelling upward.
pub fn gradient_spinner(
    _id: &'static str,
    _theme: &Theme,
    cell_px: f32,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    let center = (MATRIX_SIDE as f32 - 1.0) / 2.0;
    let max = MATRIX_SIDE as f32 - 1.0 + center;
    let delta = motion::pulse_delta(&GRADIENT_SPIN, view, cx);
    div()
        .flex()
        .flex_col()
        .gap(px(cell_px / 2.0))
        .children((0..MATRIX_SIDE).map(move |row| {
            let tint: gpui::Hsla = gpui::rgb(GSPIN_ROW_TINTS[row]).into();
            div()
                .flex()
                .flex_row()
                .gap(px(cell_px / 2.0))
                .children((0..MATRIX_SIDE).map(move |col| {
                    // Distance of this cell from the wave origin, normalized
                    // into a phase offset (gradient-spin's `--gspin-phase`).
                    let d = MATRIX_SIDE as f32 - 1.0 - row as f32 + (col as f32 - center).abs();
                    let phase = if max == 0.0 { 0.0 } else { d / (max + 1.0) };
                    div()
                        .size(px(cell_px))
                        .rounded(px(cell_px / 2.0))
                        .bg(tint)
                        .opacity(motion::gspin_opacity(delta + phase, GSPIN_DIM))
                }))
        }))
}

/// A 2×3 miniature of [`gradient_spinner`] sized for a status-dot slot: same
/// row tints and pulse timing, but the brightness SNAKES around the grid's
/// perimeter (every cell of a 2×3 grid is on the ring) instead of sweeping as
/// a vertical wave — a tiny radial chase. ~6×10px footprint at the default
/// 2.5px cells.
pub fn mini_gradient_spinner(
    key: impl Into<SharedString>,
    cell_px: f32,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    const COLS: usize = 2;
    const ROWS: usize = 3;
    /// Clockwise ring position of each `(row, col)` cell, top-left first:
    /// (0,0) → (0,1) → (1,1) → (2,1) → (2,0) → (1,0).
    const RING: [[usize; COLS]; ROWS] = [[0, 1], [5, 2], [4, 3]];
    const RING_LEN: f32 = (COLS * ROWS) as f32;
    let _key = key.into();
    let delta = motion::pulse_delta(&GRADIENT_SPIN, view, cx);
    div()
        .flex()
        .flex_col()
        .gap(px(cell_px / 2.0))
        .children((0..ROWS).map(move |row| {
            let tint: gpui::Hsla = gpui::rgb(GSPIN_ROW_TINTS[row]).into();
            div()
                .flex()
                .flex_row()
                .gap(px(cell_px / 2.0))
                .children((0..COLS).map(move |col| {
                    let phase = RING[row][col] as f32 / RING_LEN;
                    div()
                        .size(px(cell_px))
                        .rounded(px(cell_px / 2.0))
                        .bg(tint)
                        .opacity(motion::gspin_opacity(delta + phase, GSPIN_DIM))
                }))
        }))
}

/// Which orb to draw. All four share one period, one tint and one box, and
/// differ only in how the circles are arranged and what the phase moves.
///
/// A parameter rather than four functions, for the reason [`crate::input::Shape`]
/// is one: they are the same operation, and the thing that differs is an
/// argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orb {
    /// Blobs whose sizes swing so the count you perceive changes, drifting far
    /// enough to merge and separate. The one a thinking surface should reach
    /// for.
    Cluster,
    /// Dots on a circle, brightness chasing round — the classic, with a real
    /// circle instead of the mini spinner's 2×3 grid.
    Ring,
    /// Dots gathering to a single point and opening back out to the ring.
    Converge,
    /// Rings leaving the centre and fading before the edge. The only one that
    /// travels outward, which is what makes it read as a signal rather than a
    /// wait.
    Bloom,
}

/// The orbs — **bezel's own loaders**, and the only ones here that are.
///
/// The three older loaders in this module are all grids of cells: a pulse row,
/// a 3×3 matrix, a 2×3 mini. Three variations on one arrangement is a narrow
/// vocabulary for the surface a library gets looked at through, and `phase.rs`
/// makes the point itself: *a loading indicator is a brand surface.*
///
/// Everything is circles, because that is the whole vocabulary gpui gives at
/// the pinned rev: no rotation transform, no conic gradient, and no blur filter
/// on an element ([`crate::material`]'s backdrop blur blurs what is *behind* a
/// surface and cannot soften the surface itself). So the glow is a `BoxShadow`,
/// the ring is eight positioned dots rather than a swept arc, and every
/// position is arithmetic — all of it pure and unit-tested in
/// [`motion::phase`].
///
/// One tint, from the theme's accent. In three hues this would be the gradient
/// spinner wearing a different shape.
pub fn orb(
    shape: Orb,
    key: impl Into<SharedString>,
    size_px: f32,
    theme: &Theme,
    view: EntityId,
    cx: &mut App,
) -> impl IntoElement {
    let _key = key.into();
    let delta = motion::pulse_delta(&ORB, view, cx);
    let accent = theme.accent;

    // A circle placed by its centre, since every seat below is a centre.
    let dot = move |cx: f32, cy: f32, size: f32, opacity: f32, glow: f32| {
        div()
            .absolute()
            .left(px(cx - size / 2.0))
            .top(px(cy - size / 2.0))
            .size(px(size))
            .rounded_full()
            .bg(accent.opacity(opacity))
            .shadow(vec![gpui::BoxShadow {
                color: accent.opacity(opacity * 0.55),
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(glow),
                spread_radius: px(0.0),
                inset: false,
            }])
    };

    let cells: Vec<gpui::Div> = match shape {
        Orb::Cluster => (0..motion::ORBS)
            .map(|index| {
                // A third of a period apart, so one is always swelling while
                // another shrinks and the silhouette never repeats.
                let phase = motion::staggered_phase(delta, index, 1.0 / motion::ORBS as f32);
                let (seat_x, seat_y) = motion::ORB_SEATS[index];
                let (drift_x, drift_y) = motion::orb_drift(phase);
                dot(
                    size_px * (seat_x + drift_x),
                    size_px * (seat_y + drift_y),
                    size_px * motion::orb_size(phase),
                    motion::orb_opacity(phase),
                    size_px * motion::orb_glow(phase),
                )
            })
            .collect(),
        Orb::Ring => (0..motion::ORB_RING_DOTS)
            .map(|index| {
                let phase =
                    motion::staggered_phase(delta, index, 1.0 / motion::ORB_RING_DOTS as f32);
                let (seat_x, seat_y) = motion::orb_ring_seat(index, motion::ORB_RING_RADIUS);
                dot(
                    size_px * seat_x,
                    size_px * seat_y,
                    size_px * motion::ORB_RING_DOT,
                    motion::orb_opacity(phase),
                    size_px * motion::orb_glow(phase) * 0.5,
                )
            })
            .collect(),
        Orb::Converge => {
            // One phase for every dot, unlike the ring: they travel together,
            // so the gathered frame is a single point rather than a queue.
            let radius = motion::orb_converge_radius(delta);
            (0..motion::ORB_RING_DOTS)
                .map(|index| {
                    let (seat_x, seat_y) = motion::orb_ring_seat(index, radius);
                    dot(
                        size_px * seat_x,
                        size_px * seat_y,
                        size_px * motion::ORB_RING_DOT,
                        motion::orb_opacity(delta),
                        size_px * motion::orb_glow(delta) * 0.5,
                    )
                })
                .collect()
        }
        Orb::Bloom => (0..motion::ORB_BLOOM_RINGS)
            .map(|index| {
                let phase =
                    motion::staggered_phase(delta, index, 1.0 / motion::ORB_BLOOM_RINGS as f32);
                let diameter = size_px * motion::orb_bloom_radius(phase);
                let opacity = motion::orb_bloom_opacity(phase);
                // A ring, not a disc: the border is the whole element, so this
                // one is the odd shape out and cannot go through `dot`.
                div()
                    .absolute()
                    .left(px((size_px - diameter) / 2.0))
                    .top(px((size_px - diameter) / 2.0))
                    .size(px(diameter))
                    .rounded_full()
                    .border(px((size_px * 0.05).max(1.0)))
                    .border_color(accent.opacity(opacity))
            })
            .collect(),
    };

    div().relative().size(px(size_px)).children(cells)
}

/// "L O A D I N G" — `text-[11px] uppercase tracking-[0.32em]`; tracking
/// approximated with thin spaces (gpui has no letter-spacing at the pinned
/// rev).
pub fn loading_word(theme: &Theme) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .text_color(theme.text_muted.opacity(0.7))
        .child(SharedString::from(
            "L\u{2009}O\u{2009}A\u{2009}D\u{2009}I\u{2009}N\u{2009}G",
        ))
}

// Compile-time proof the specs referenced here stay wired to the catalog.
const _: () = {
    assert!(PULSE.duration_ms == 2400);
    assert!(GRADIENT_SPIN.duration_ms == 750);
};
