use js::value::{RegExp, TypeError, Value};
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
