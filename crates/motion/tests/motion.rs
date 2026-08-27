use std::time::Duration;

use gpui::Rgba;
use motion::*;

/// A fade in a stand-in view. `EntityId: From<u64>`, so the pure store needs no
/// app to key on one.
fn fade(key: &'static str) -> Fade {
    Fade::new(gpui::EntityId::from(1).into(), key)
}
use web_time::Instant;

#[test]
fn eval_never_escapes_unit_interval_dense_sweep() {
    // Regression: f32 rounding produced 1.000000119 near the tail of
    // EASE_OUT_EXPO, tripping gpui's `delta ∈ [0,1]` assert (SIGABRT on
    // the user's machine). Sweep densely, including the values right
    // below 1.0 where Newton lands closest to the endpoint.
    for curve in [EASE_OUT_EXPO, EASE_OUT, EASE, EASE_RESORT, EASE_IN_OUT] {
        for i in 0..=100_000u32 {
            let x = i as f32 / 100_000.0;
            let y = curve.eval(x);
            assert!((0.0..=1.0).contains(&y), "eval({x}) = {y} escaped [0,1]");
        }
        for x in [0.999_999f32, 0.999_999_9, 1.0 - f32::EPSILON] {
            let y = curve.eval(x);
            assert!((0.0..=1.0).contains(&y), "eval({x}) = {y} escaped [0,1]");
        }
    }
}

fn assert_close(actual: f32, expected: f32, tol: f32, ctx: &str) {
    assert!(
        (actual - expected).abs() <= tol,
        "{ctx}: got {actual}, expected {expected} ±{tol}"
    );
}

#[test]
fn bezier_linear_is_identity() {
    let linear = CubicBezier::new(0.0, 0.0, 1.0, 1.0);
    for x in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
        assert_close(linear.eval(x), x, 1e-4, "linear");
    }
}

#[test]
fn bezier_known_values() {
    // References computed independently with 80-step bisection.
    let cases: [(&str, CubicBezier, [f32; 5]); 3] = [
        (
            "expo",
            EASE_OUT_EXPO,
            [0.494391, 0.825622, 0.971779, 0.997677, 0.999878],
        ),
        (
            "ease-out",
            EASE_OUT,
            [0.160572, 0.378138, 0.684643, 0.906535, 0.982973],
        ),
        (
            "ease",
            EASE,
            [0.094796, 0.408511, 0.802403, 0.960459, 0.994316],
        ),
    ];
    for (name, curve, expected) in cases {
        for (x, want) in [0.1, 0.25, 0.5, 0.75, 0.9].into_iter().zip(expected) {
            assert_close(curve.eval(x), want, 1e-3, name);
        }
    }
}

#[test]
fn bezier_endpoints_and_clamping() {
    for curve in [EASE_OUT_EXPO, EASE_OUT, EASE, EASE_RESORT, EASE_IN_OUT] {
        assert_eq!(curve.eval(0.0), 0.0);
        assert_eq!(curve.eval(1.0), 1.0);
        assert_eq!(curve.eval(-0.5), 0.0);
        assert_eq!(curve.eval(1.5), 1.0);
    }
}

#[test]
fn bezier_is_monotonic_for_catalog_curves() {
    for curve in [EASE_OUT_EXPO, EASE_OUT, EASE, EASE_RESORT, EASE_IN_OUT] {
        let mut last = 0.0;
        for i in 0..=100 {
            let y = curve.eval(i as f32 / 100.0);
            assert!(y >= last - 1e-4, "monotonicity violated at {i}");
            last = y;
        }
    }
}

#[test]
fn spec_delay_holds_then_runs() {
    // SPLASH_OUT: 150ms delay + 500ms run = 650ms total.
    assert_eq!(SPLASH_OUT.total(), Duration::from_millis(650));
    assert_eq!(SPLASH_OUT.progress(0.0), 0.0);
    // Still inside the delay window at raw 0.2 (130ms < 150ms).
    assert_eq!(SPLASH_OUT.progress(0.2), 0.0);
    // Fully done at the end; clamped beyond.
    assert_eq!(SPLASH_OUT.progress(1.0), 1.0);
    assert_eq!(SPLASH_OUT.progress(2.0), 1.0);
    // Midway through the run: raw 0.65 → 272.5ms into the 500ms run.
    let mid = SPLASH_OUT.progress(0.65);
    assert!(mid > 0.0 && mid < 1.0);
    // No-delay specs pass straight through the curve.
    assert_close(
        FADE_IN.progress(0.5),
        EASE_OUT_EXPO.eval(0.5),
        1e-6,
        "no-delay",
    );
}

#[test]
fn catalog_timings_match_the_source() {
    assert_eq!(FADE_IN.duration_ms, 500);
    assert_eq!(FADE_QUICK.duration_ms, 150);
    assert_eq!(MENU_IN.duration_ms, 140);
    assert_eq!(DIALOG_IN.duration_ms, 180);
    assert_eq!((SPLASH_OUT.duration_ms, SPLASH_OUT.delay_ms), (500, 150));
    assert_eq!(RESIZE.duration_ms, 200);
    assert_eq!(TAB_SLIDE.duration_ms, 150);
    assert_eq!(COLLAPSE.duration_ms, 180);
    assert_eq!(CHEVRON.duration_ms, 200);
    assert_eq!(PULSE.duration_ms, 2400);
    assert_eq!(GRADIENT_SPIN.duration_ms, 750);
    assert_eq!(EASE_OUT_EXPO, CubicBezier::new(0.16, 1.0, 0.3, 1.0));
}

#[test]
fn pulse_wave_endpoints() {
    assert_close(pulse_wave(0.0), 0.0, 1e-6, "wave start");
    assert_close(pulse_wave(0.5), 1.0, 1e-6, "wave peak");
    assert_close(pulse_wave(1.0), 0.0, 1e-6, "wave end");
    assert_close(pulse_opacity(0.0), 0.08, 1e-6, "opacity floor");
    assert_close(pulse_opacity(0.5), 1.0, 1e-6, "opacity peak");
    assert_close(pulse_scale(0.0), 0.9, 1e-6, "scale floor");
    assert_close(pulse_scale(0.5), 1.0, 1e-6, "scale peak");
}

#[test]
fn stagger_wraps_and_orders_cells() {
    // Cell 0 at delta 0 is at phase 0; later cells lag by the stagger.
    assert_close(staggered_phase(0.0, 0, PULSE_STAGGER), 0.0, 1e-6, "cell 0");
    assert_close(
        staggered_phase(0.0, 1, PULSE_STAGGER),
        1.0 - PULSE_STAGGER,
        1e-5,
        "cell 1 wraps",
    );
    // A full period later the phase is identical.
    assert_close(
        staggered_phase(0.3, 2, PULSE_STAGGER),
        staggered_phase(0.3 + 1.0, 2, PULSE_STAGGER),
        2e-6,
        "periodic",
    );
    // Matrix wave peaks travel: diagonal k peaks when the front reaches it.
    let peak0 = matrix_wave(0.5, 0, 5);
    assert_close(peak0, 1.0, 1e-5, "diag 0 peak at half period");
}

#[test]
fn lerp_basics() {
    assert_eq!(lerp(208.0, 400.0, 0.0), 208.0);
    assert_eq!(lerp(208.0, 400.0, 1.0), 400.0);
    assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
}

#[test]
fn hover_fade_ramps_and_reverses_continuously() {
    let _guard = lock_speed();
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    let ms = |m: u64| t0 + Duration::from_millis(m);

    // Enter: 0 at the flip, mid-flight strictly between, 1 at 150ms.
    fades.set_at(&fade("pill"), true, false, t0);
    assert_eq!(fades.value_at(&fade("pill"), t0), 0.0);
    let mid = fades.value_at(&fade("pill"), ms(75));
    assert!(mid > 0.0 && mid < 1.0, "mid-flight enter: {mid}");
    assert_eq!(fades.value_at(&fade("pill"), ms(150)), 1.0);
    assert_eq!(
        fades.value_at(&fade("pill"), ms(400)),
        1.0,
        "clamps past the end"
    );

    // Leave mid-flight re-anchors at the current value — no jump.
    fades.set_at(&fade("pill"), true, false, t0);
    let at_flip = fades.value_at(&fade("pill"), ms(75));
    fades.set_at(&fade("pill"), false, false, ms(75));
    let after_flip = fades.value_at(&fade("pill"), ms(75));
    assert!(
        (after_flip - at_flip).abs() < 1e-4,
        "continuity: {at_flip} vs {after_flip}"
    );
    let falling = fades.value_at(&fade("pill"), ms(140));
    assert!(falling < after_flip, "fades back down");
    assert_eq!(fades.value_at(&fade("pill"), ms(225)), 0.0, "lands at rest");
}

#[test]
fn hover_fade_reduced_motion_snaps() {
    let _guard = lock_speed();
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    fades.set_at(&fade("row"), true, true, t0);
    assert_eq!(fades.value_at(&fade("row"), t0), 1.0, "enter snaps to 1");
    fades.set_at(&fade("row"), false, true, t0);
    assert_eq!(fades.value_at(&fade("row"), t0), 0.0, "leave snaps to 0");
}

#[test]
fn hover_fade_leave_without_enter_is_inert() {
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    fades.set_at(&fade("ghost"), false, false, t0);
    assert!(fades.entries.is_empty(), "no entry for a leave-only key");
    assert_eq!(fades.value_at(&fade("ghost"), t0), 0.0);
}

#[test]
fn hover_tick_reports_flight_and_prunes() {
    let _guard = lock_speed();
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    let ms = |m: u64| t0 + Duration::from_millis(m);

    fades.set_at(&fade("a"), true, false, t0);
    // Mid-flight: active, frames must keep coming (read each frame).
    assert!(fades.tick_at(ms(50)));
    fades.value_at(&fade("a"), ms(50));
    assert!(fades.tick_at(ms(100)));
    fades.value_at(&fade("a"), ms(100));
    // Settled hovered (still read): no more frames needed, entry kept.
    assert!(!fades.tick_at(ms(200)));
    fades.value_at(&fade("a"), ms(200));
    assert_eq!(fades.value_at(&fade("a"), ms(250)), 1.0);

    // Leave → fades → settles at rest → entry evicted.
    fades.set_at(&fade("a"), false, false, ms(250));
    assert!(fades.tick_at(ms(300)));
    fades.value_at(&fade("a"), ms(300));
    assert!(!fades.tick_at(ms(500)), "settled at rest");
    assert!(fades.entries.is_empty(), "rest entries are pruned");
}

#[test]
fn hover_tick_evicts_unread_entries() {
    // An element that unmounts mid-hover never sends its leave — a full
    // frame without a read drops the entry so a remount starts clean.
    let _guard = lock_speed();
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    let ms = |m: u64| t0 + Duration::from_millis(m);
    fades.set_at(&fade("menu-row"), true, false, t0);
    fades.tick_at(ms(16));
    fades.value_at(&fade("menu-row"), ms(16)); // frame 1: mounted, read
    fades.tick_at(ms(32)); // frame 2: unmounted — no read
    fades.tick_at(ms(48)); // frame 3: a full unread frame has passed
    assert!(fades.entries.is_empty(), "unread entry evicted");
    assert_eq!(fades.value_at(&fade("menu-row"), ms(64)), 0.0);
}

#[test]
fn mix_endpoints_and_transparent_blend() {
    let rest = theme::neutral(0.235);
    let hover = theme::neutral(0.29);
    assert_eq!(mix(rest, hover, 0.0), rest);
    assert_eq!(mix(rest, hover, 1.0), hover);
    assert_eq!(mix(rest, hover, -1.0), rest, "t clamps low");
    assert_eq!(mix(rest, hover, 2.0), hover, "t clamps high");

    // Opaque blend: lightness moves monotonically between the endpoints.
    let mid = mix(rest, hover, 0.5);
    assert!(mid.l > rest.l && mid.l < hover.l, "mid lightness {}", mid.l);

    // Transparent → wash: alpha ramps, hue stays the wash's (premultiplied
    // — never a darkened grey mid-fade). `ink` reads the process-wide
    // appearance, which theme's tests flip — hold the lock so this test
    // never observes a mid-flip Light palette.
    let _guard = theme::lock_appearance();
    let wash = theme::ink(0.06);
    let half = mix(gpui::transparent_black(), wash, 0.5);
    assert!((half.a - 0.03).abs() < 1e-4, "alpha midpoint {}", half.a);
    let half_rgba = Rgba::from(half);
    assert!(
        half_rgba.r > 0.99 && half_rgba.g > 0.99 && half_rgba.b > 0.99,
        "white wash keeps its hue: {half_rgba:?}"
    );
}

#[test]
fn hover_spec_matches_tailwind_transition_colors() {
    assert_eq!(HOVER_FADE.duration_ms, 150);
    assert_eq!(HOVER_FADE.delay_ms, 0);
    assert_eq!(EASE_TAILWIND, CubicBezier::new(0.4, 0.0, 0.2, 1.0));
}

#[test]
fn speed_is_set_in_code_and_cannot_be_set_to_nonsense() {
    let _guard = lock_speed();
    assert_eq!(speed_scale(), 1.0, "the designed speed is the default");

    set_speed(10.0);
    assert_eq!(speed_scale(), 10.0);
    // It is what every timeline is measured in, so it reaches the clocks.
    assert_eq!(
        HoverFades::duration(),
        HOVER_FADE.total().mul_f32(10.0),
        "the fade clock reads the same knob"
    );

    // A zero would make every duration instantaneous and a negative one
    // would run time backwards; NaN would poison every duration silently.
    set_speed(0.0);
    assert_eq!(speed_scale(), 0.01);
    set_speed(1e9);
    assert_eq!(speed_scale(), 100.0);
    set_speed(f32::NAN);
    assert_eq!(speed_scale(), 1.0);

    set_speed(1.0);
}

#[test]
fn gspin_pulse_shape() {
    // Full at the cycle start, dim through the rest band, rising at the tail.
    assert_close(gspin_opacity(0.0, 0.1), 1.0, 1e-6, "cycle start");
    assert_close(gspin_opacity(0.45, 0.1), 0.1, 1e-6, "fully dim");
    assert_close(gspin_opacity(0.9, 0.1), 0.1, 1e-6, "rest band");
    assert_close(gspin_opacity(1.0, 0.1), 1.0, 1e-6, "wraps to full");
    let mid_fall = gspin_opacity(0.2, 0.1);
    assert!(mid_fall > 0.1 && mid_fall < 1.0, "eases down");
    let mid_rise = gspin_opacity(0.96, 0.1);
    assert!(mid_rise > 0.1 && mid_rise < 1.0, "eases up");
}
