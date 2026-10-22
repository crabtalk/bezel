use d3::format::number;

#[test]
fn parse_int_reads_every_radix() {
    assert_eq!(number::parse_int("42px", 0), 42.0);
    assert_eq!(number::parse_int("-0x1A", 0), -26.0);
    assert_eq!(number::parse_int("0x1A", 16), 26.0);
    assert_eq!(number::parse_int("0x1A", 10), 0.0);
    assert_eq!(number::parse_int("101", 2), 5.0);
    assert_eq!(number::parse_int("z", 36), 35.0);
    assert_eq!(number::parse_int("  3.9", 10), 3.0);
    assert!(number::parse_int("z", 37).is_nan());
    assert!(number::parse_int("1", 1).is_nan());
    assert!(number::parse_int("", 0).is_nan());
}

#[test]
fn to_integer_truncates_and_zeroes_nan() {
    assert_eq!(number::to_integer(-2.7), -2.0);
    assert_eq!(number::to_integer(f64::NAN), 0.0);
    assert!(number::to_integer(-0.5).is_sign_positive());
    assert_eq!(number::to_integer(f64::INFINITY), f64::INFINITY);
}

#[test]
fn decimal_prefix_is_the_literal_parse_float_reads() {
    assert_eq!(number::decimal_prefix("1.5e3x"), 5);
    assert_eq!(number::decimal_prefix("1e"), 1);
    assert_eq!(number::decimal_prefix(".5"), 2);
    assert_eq!(number::decimal_prefix("-Infinityx"), 9);
    assert_eq!(number::decimal_prefix("."), 0);
}

#[test]
fn round_keeps_negative_zero() {
    assert_eq!(number::round(2.5), 3.0);
    assert_eq!(number::round(-2.5), -2.0);
    assert!(number::round(-0.4).is_sign_negative());
    assert!(number::round(-0.0).is_sign_negative());
    assert!(number::round(0.4).is_sign_positive());
}

#[test]
fn max_and_min_order_signed_zeros() {
    assert!(number::max(-0.0, 0.0).is_sign_positive());
    assert!(number::max(0.0, -0.0).is_sign_positive());
    assert!(number::min(0.0, -0.0).is_sign_negative());
    assert!(number::min(-0.0, 0.0).is_sign_negative());
    assert!(number::max(1.0, f64::NAN).is_nan());
}

#[test]
fn hypot_takes_any_count() {
    assert_eq!(number::hypot(&[]), 0.0);
    assert_eq!(number::hypot(&[3.0, 4.0]), 5.0);
    assert_eq!(number::hypot(&[-3.0]), 3.0);
    assert_eq!(number::hypot(&[1.0, 2.0, 2.0]), 3.0);
    assert_eq!(number::hypot(&[f64::NAN, f64::NEG_INFINITY]), f64::INFINITY);
    assert!(number::hypot(&[f64::NAN, 1.0]).is_nan());
}

#[test]
fn pow_of_one_to_infinity_is_nan() {
    assert_eq!(number::pow(2.0, 10.0), 1024.0);
    assert_eq!(number::pow(f64::NAN, 0.0), 1.0);
    assert!(number::pow(1.0, f64::NAN).is_nan());
    assert!(number::pow(-1.0, f64::INFINITY).is_nan());
}
