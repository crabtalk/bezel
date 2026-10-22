use chart::value::{TypeError, Value, regexp::RegExp};
use std::cmp::Ordering;

fn array(items: &[Value]) -> Value {
    items.to_vec().into()
}

#[test]
fn conversions_follow_javascript() {
    assert_eq!(Value::Null.to_number(), 0.0);
    assert!(Value::Undefined.to_number().is_nan());
    assert_eq!(Value::from(" 12 ").to_number(), 12.0);
    assert_eq!(array(&[5.0.into()]).to_number(), 5.0);
    assert_eq!(
        array(&[1.0.into(), Value::Null, "a".into()]).to_string(),
        "1,,a"
    );
    assert_eq!(Value::Object([].into()).to_string(), "[object Object]");
    assert!(!Value::from("").truthy() && array(&[]).truthy());
}

#[test]
fn equality_is_loose_strict_or_structural() {
    let items = array(&[1.0.into()]);
    assert!(items.strict_eq(&items.clone()));
    assert!(!items.strict_eq(&array(&[1.0.into()])));
    assert_eq!(items, array(&[1.0.into()]));
    assert!(Value::Null.loose_eq(&Value::Undefined));
    assert!(Value::from("1").loose_eq(&true.into()));
    assert!(items.loose_eq(&"1".into()));
    assert_eq!(
        Value::from("b").compare(&"a".into()),
        Some(Ordering::Greater)
    );
    assert_eq!(Value::from(1.0).compare(&f64::NAN.into()), None);
    assert_eq!(&Value::from(1.0) + &"2".into(), Value::from("12"));
}

#[test]
fn properties_read_as_javascript_reads_them() {
    let object = Value::Object([("a".into(), array(&["x".into(), "y".into()]))].into());
    let tags = object.get(&"a".into()).unwrap();
    assert_eq!(tags.get(&1.0.into()), Ok("y".into()));
    assert_eq!(tags.get(&"length".into()), Ok(2.0.into()));
    assert_eq!(object.get(&"b".into()), Ok(Value::Undefined));
    assert_eq!(
        Value::Null.get(&"a".into()),
        Err(TypeError(
            "Cannot read properties of null (reading 'a')".into()
        ))
    );
}

#[test]
fn a_regexp_prints_as_its_literal() {
    assert_eq!(RegExp::new("a+", "gi").unwrap().to_string(), "/a+/gi");
    assert_eq!(RegExp::new("", "").unwrap().to_string(), "/(?:)/");
    assert!(RegExp::new("(", "").is_err());
}

#[test]
fn a_regexp_matches_as_javascript_does() {
    let test =
        |source: &str, flags: &str, text: &str| RegExp::new(source, flags).unwrap().test(text);
    assert!(!test(r"^\d$", "", "٣"));
    assert!(!test(r"^\w$", "", "é"));
    assert!(test(r"^\s$", "", "\u{feff}"));
    assert!(!test(r"^.$", "", "\u{2028}"));
    assert!(test(r"^.$", "s", "\u{2028}"));
    assert!(test(r"^a{2}$", "", "aa"));
    assert!(test(r"^a{x}$", "", "a{x}"));
    assert!(test(r"^é\x41$", "", "éA"));
    assert!(test(r"^😀$", "", "😀"));
    assert!(test(r"^[^]$", "", "\n"));
    assert!(!test(r"[]", "", "a"));
    assert!(test(r"\bcat\b", "", "a cat"));
    assert!(test(r"^b", "y", "bc"));
    assert!(!test(r"b", "y", "abc"));
    assert!(test(r"(?<year>\d{4})", "", "in 2024"));
}

#[test]
fn a_regexp_without_a_regex_crate_form_is_a_syntax_error() {
    assert!(RegExp::new("a(?=b)", "").is_err());
    assert!(RegExp::new("(?<!a)b", "").is_err());
    assert!(RegExp::new(r"(a)\1", "").is_err());
    assert!(RegExp::new("a", "gg").is_err());
    assert!(RegExp::new("[a", "").is_err());
}

#[test]
fn a_regexp_replaces_and_splits_as_javascript_does() {
    let re = |source: &str, flags: &str| RegExp::new(source, flags).unwrap();
    assert_eq!(re("", "g").replace("ab", "-"), "-a-b-");
    assert_eq!(re("x*", "g").replace("axb", "-"), "-a--b-");
    assert_eq!(re("b", "").replace("abcb", "[$&]"), "a[b]cb");
    assert_eq!(re(r"(?<x>b)", "g").replace("abcb", "$<x>$<x>"), "abbcbb");
    let split = |re: RegExp, text: &str| -> Vec<Option<String>> {
        re.split(text)
            .into_iter()
            .map(|piece| piece.map(str::to_string))
            .collect()
    };
    let some = |pieces: &[&str]| -> Vec<Option<String>> {
        pieces.iter().map(|piece| Some(piece.to_string())).collect()
    };
    assert_eq!(split(re("", ""), "abc"), some(&["a", "b", "c"]));
    assert_eq!(split(re(",", ""), "a,b,"), some(&["a", "b", ""]));
    assert_eq!(split(re("x", ""), ""), some(&[""]));
    assert_eq!(split(re("", ""), ""), Vec::<Option<String>>::new());
    assert_eq!(
        split(re("(-)|(\\+)", ""), "a-b"),
        vec![Some("a".into()), Some("-".into()), None, Some("b".into())]
    );
}
