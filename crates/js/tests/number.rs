use js::number;

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
