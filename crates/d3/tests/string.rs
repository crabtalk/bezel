use d3::string;
use std::cmp::Ordering;

#[test]
fn compare_orders_by_utf16_units() {
    assert_eq!(string::compare("B", "a"), Ordering::Less);
    assert_eq!(string::compare("10", "9"), Ordering::Less);
    // By code point U+1F600 is the larger; its first unit, 0xD83D, is not.
    assert_eq!(string::compare("\u{ff61}", "😀"), Ordering::Greater);
}

#[test]
fn replace_reads_dollar_patterns() {
    assert_eq!(string::replace("a-b-c", "-", "+"), "a+b-c");
    assert_eq!(string::replace("abc", "b", "[$&]"), "a[b]c");
    assert_eq!(string::replace("abc", "b", "$`$'$$"), "aac$c");
    assert_eq!(string::replace("abc", "b", "$1$<x>"), "a$1$<x>c");
    assert_eq!(string::replace("abc", "z", "y"), "abc");
}
