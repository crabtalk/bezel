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

#[test]
fn asleep_holds_the_eyes_shut() {
    for i in 0..400 {
        let t = i as f32 * 0.05;
        assert_eq!(Motion::ASLEEP.beat(t).lid, 1.0);
    }
}

#[test]
fn a_resting_lid_is_a_floor_under_the_blink() {
    let half = Motion {
        lid: 0.5,
        ..Motion::ALIVE
    };
    for i in 0..400 {
        let t = i as f32 * 0.05;
        let lid = half.beat(t).lid;
        assert!((0.5..=1.0).contains(&lid));
        assert!(lid >= Motion::ALIVE.beat(t).lid);
    }
}

/// A busy gaze crosses to both sides within its scan, and the fit spends the
/// sweep, so an eye at either end stays inside the body.
#[test]
fn a_busy_gaze_sweeps_inside_the_body() {
    let busy = Motion::BUSY;
    let sweeps: Vec<f32> = (0..240).map(|i| busy.sweep(i as f32 * 0.01)).collect();
    assert!(sweeps.iter().any(|x| *x > busy.scan * 0.9));
    assert!(sweeps.iter().any(|x| *x < -busy.scan * 0.9));
    assert!(sweeps.iter().all(|x| x.abs() <= busy.scan));
    for i in 0..2_000u64 {
        let seed = i.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let shape = Shape::from(seed);
        for eye in Eyes::from(seed).place(&shape, busy.reach()) {
            for side in [-busy.reach(), busy.reach()] {
                let (cx, cy) = (eye.cx + side, eye.cy);
                let far = cx.hypot(cy) + eye.rx.hypot(eye.ry);
                assert!(far <= shape.reach(cx, cy), "seed {i} swept an eye out");
            }
        }
    }
}

#[test]
fn only_a_busy_face_sweeps() {
    for motion in [Motion::STILL, Motion::ALIVE, Motion::ASLEEP] {
        assert_eq!(motion.sweep(0.3), 0.0);
    }
}
