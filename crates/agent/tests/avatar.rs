use agent::avatar::{Eyes, Motion, SAMPLES, Shape, seed};
use std::f32::consts::TAU;

fn sample(shape: &Shape) -> Vec<f32> {
    (0..SAMPLES)
        .map(|i| shape.radius(TAU * i as f32 / SAMPLES as f32))
        .collect()
}

/// A seed is a face, not a lookup: every one of them has to draw.
#[test]
fn every_seed_is_a_legal_outline() {
    for i in 0..20_000u64 {
        let shape = Shape::from(i.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for r in sample(&shape) {
            assert!(
                r.is_finite() && (0.44..=1.6).contains(&r),
                "seed {i} left the outline at r={r}"
            );
        }
    }
}

#[test]
fn presets_are_legal_too() {
    for (name, shape) in Shape::PRESETS {
        for r in sample(&shape) {
            assert!(
                r.is_finite() && (0.44..=1.6).contains(&r),
                "{name} at r={r}"
            );
        }
    }
}

/// The eye fit is solved against the body's own reach, so no eye may cross the
/// outline — including under the gaze drift motion is allowed to add.
#[test]
fn eyes_stay_inside_the_body() {
    let drift = Motion::ALIVE.drift;
    for i in 0..20_000u64 {
        let seed = i.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let shape = Shape::from(seed);
        for eye in Eyes::from(seed).place(&shape, drift) {
            let (cx, cy) = (eye.cx + drift, eye.cy + drift);
            let far = cx.hypot(cy) + eye.rx.hypot(eye.ry);
            assert!(
                far <= shape.reach(cx, cy),
                "seed {i} pushed an eye {far} past a reach of {}",
                shape.reach(cx, cy)
            );
        }
    }
}

/// A name is one route to a seed, so it keeps a face across a reload — and the
/// keyboard's shift key is not part of anyone's identity.
#[test]
fn a_name_is_one_seed() {
    assert_eq!(Shape::from("Sara"), Shape::from("Sara"));
    assert_ne!(Shape::from("Sara"), Shape::from("Dan"));
    assert_eq!(seed("  Mixed Case 9  "), seed("mixed case 9"));
    assert_eq!(seed("Grace\u{a0} Hopper"), seed("grace hopper"));
}

/// Motion is a function of `t` alone: the same instant paints the same face,
/// and a still avatar never moves at all.
#[test]
fn motion_is_pure() {
    let shape = Shape::BLOB;
    assert_eq!(
        Motion::ALIVE.shape(shape, 3.5),
        Motion::ALIVE.shape(shape, 3.5)
    );
    assert_ne!(
        Motion::ALIVE.shape(shape, 0.0),
        Motion::ALIVE.shape(shape, 2.0)
    );
    assert_eq!(Motion::STILL.shape(shape, 99.0), shape);
    assert_eq!(Motion::STILL.beat(99.0), Motion::STILL.beat(0.0));
}
