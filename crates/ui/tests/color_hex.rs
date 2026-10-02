use ui::color::{parse_hex, to_hex};

#[test]
fn hex_round_trips_six_and_eight_digits() {
    for text in ["#0a84ff", "#ff3b30", "#000000", "#ffffff"] {
        assert_eq!(to_hex(parse_hex(text).unwrap(), false), text);
    }
    assert_eq!(to_hex(parse_hex("#0a84ff80").unwrap(), true), "#0a84ff80");
    assert_eq!(to_hex(parse_hex("0A84FF").unwrap(), true), "#0a84ffff");
}

#[test]
fn hex_rejects_other_lengths_and_digits() {
    for text in ["", "#fff", "#0a84f", "#0a84ffa", "#gggggg", "#0a84ff800"] {
        assert!(parse_hex(text).is_none(), "{text}");
    }
}
