use js::math;

#[test]
fn round_keeps_negative_zero() {
    assert_eq!(math::round(2.5), 3.0);
    assert_eq!(math::round(-2.5), -2.0);
    assert!(math::round(-0.4).is_sign_negative());
    assert!(math::round(-0.0).is_sign_negative());
    assert!(math::round(0.4).is_sign_positive());
}

#[test]
fn max_and_min_order_signed_zeros() {
    assert!(math::max(-0.0, 0.0).is_sign_positive());
    assert!(math::max(0.0, -0.0).is_sign_positive());
    assert!(math::min(0.0, -0.0).is_sign_negative());
    assert!(math::min(-0.0, 0.0).is_sign_negative());
    assert!(math::max(1.0, f64::NAN).is_nan());
}

#[test]
fn hypot_takes_any_count() {
    assert_eq!(math::hypot(&[]), 0.0);
    assert_eq!(math::hypot(&[3.0, 4.0]), 5.0);
    assert_eq!(math::hypot(&[-3.0]), 3.0);
    assert_eq!(math::hypot(&[1.0, 2.0, 2.0]), 3.0);
    assert_eq!(math::hypot(&[f64::NAN, f64::NEG_INFINITY]), f64::INFINITY);
    assert!(math::hypot(&[f64::NAN, 1.0]).is_nan());
}

#[test]
fn pow_of_one_to_infinity_is_nan() {
    assert_eq!(math::pow(2.0, 10.0), 1024.0);
    assert_eq!(math::pow(f64::NAN, 0.0), 1.0);
    assert!(math::pow(1.0, f64::NAN).is_nan());
    assert!(math::pow(-1.0, f64::INFINITY).is_nan());
}
