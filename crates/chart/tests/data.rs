use chart::{Channel, Chart, Column, Data, Encode, Kind, Mark, Name};

use vega::value::Value;

fn sales() -> Data {
    Data::new()
        .text("month", ["Jan", "Feb", "Jan", "Mar", "Feb"])
        .number("sales", [3.0, 5.0, 2.0, 7.0, f64::NAN])
}

#[test]
fn text_is_interned_in_first_seen_order() {
    let data = sales();
    let Some(Column::Text(month)) = data.column("month") else {
        panic!("month is text");
    };
    assert_eq!(
        &*month.names,
        ["Jan", "Feb", "Mar"].map(|name| Name::Text(name.into()))
    );
    assert_eq!(&*month.codes, [0, 1, 0, 2, 1]);
    assert_eq!(month.get(3).map(AsRef::as_ref), Some("Mar"));
    assert_eq!(data.len(), 5);
}

#[test]
fn a_clone_shares_its_generation_and_a_change_takes_a_new_one() {
    let data = sales();
    assert_eq!(data.clone().generation(), data.generation());
    assert_ne!(sales().generation(), data.generation());

    let changed = data.clone().number("sales", [1.0; 5]);
    assert_ne!(changed.generation(), data.generation());
    let Some(Column::Number(sales)) = data.column("sales") else {
        panic!("sales is numeric");
    };
    assert_eq!(sales[0], 3.0, "the original is untouched");
}

#[test]
fn replacing_a_column_keeps_its_place() {
    let data = sales().text("month", ["a", "b", "c", "d", "e"]);
    let fields: Vec<_> = data.fields().map(|(name, _)| name.as_ref()).collect();
    assert_eq!(fields, ["month", "sales"]);
}

#[test]
fn the_only_column_may_be_replaced_at_another_length() {
    let data = Data::new().number("x", [1.0]).number("x", [1.0, 2.0]);
    assert_eq!(data.len(), 2);
}

#[test]
#[should_panic(expected = "not as long")]
fn a_column_of_another_length_panics() {
    let _ = sales().number("profit", [1.0]);
}

#[test]
fn builders_fill_the_encoding() {
    let chart = Chart::bar(sales())
        .x(Channel::ordinal("month"))
        .y(Channel::quantitative("sales"))
        .title("Sales");
    assert_eq!(chart.layers[0].mark, Mark::Bar);
    assert_eq!(
        chart.layers[0]
            .encoding
            .x
            .as_ref()
            .and_then(Encode::channel)
            .cloned()
            .unwrap()
            .field()
            .unwrap()
            .as_ref(),
        "month"
    );
    assert_eq!(
        chart.layers[0]
            .encoding
            .y
            .as_ref()
            .and_then(Encode::channel)
            .cloned()
            .unwrap()
            .kind,
        Kind::Quantitative
    );
    assert!(chart.layers[0].encoding.color.is_none());
}

#[test]
fn values_take_the_column_that_holds_them() {
    let column = |values: Vec<Value>| values.into_iter().collect::<Column>();
    let numbers = column(vec![Value::Number(1.0), Value::Null, Value::Undefined]);
    let Column::Number(numbers) = numbers else {
        panic!("numbers and nulls are numbers");
    };
    assert!(numbers[0] == 1.0 && numbers[1].is_nan() && numbers[2].is_nan());
    let Column::Text(text) = column(vec!["a".into(), Value::Null, "".into()]) else {
        panic!("strings and nulls are text");
    };
    assert_eq!(text.get(0).map(AsRef::as_ref), Some("a"));
    assert_eq!(text.get(1), None);
    assert_eq!(text.get(2).map(AsRef::as_ref), Some(""));
    let array: Value = vec![Value::Number(1.0), "b".into()].into();
    let Column::Value(values) = column(vec![array.clone(), Value::Bool(true)]) else {
        panic!("an array is a value");
    };
    assert_eq!(values[0], array);
}

#[test]
fn a_field_is_a_path_into_a_column_of_values() {
    let object = |size: f64, tags: &[&str]| {
        Value::Object(
            [
                ("size".into(), Value::Number(size)),
                (
                    "tags".into(),
                    tags.iter()
                        .map(|&tag| tag.into())
                        .collect::<Vec<Value>>()
                        .into(),
                ),
            ]
            .into(),
        )
    };
    let data = Data::new()
        .with(
            "info",
            [object(3.0, &["p", "q"]), object(5.0, &["r"]), Value::Null]
                .into_iter()
                .collect(),
        )
        .text("info.size", ["flat", "flat", "flat"]);
    let lengths = data.numbers("info[\"tags\"].length");
    assert_eq!(lengths[..2], [2.0, 1.0]);
    assert!(lengths[2].is_nan(), "past a null is undefined");
    let Some(Column::Text(tags)) = data.field("info.tags[1]").as_deref().cloned() else {
        panic!("strings are text");
    };
    assert_eq!(tags.get(0).map(AsRef::as_ref), Some("q"));
    assert_eq!(tags.get(1), None);
    // A column of the name itself comes first.
    assert!(matches!(
        data.field("info.size").as_deref(),
        Some(Column::Text(_))
    ));
    let keys = data.keys("info.size");
    assert!(keys[0] == keys[1] && keys[1] == keys[2]);
    assert_eq!(
        data.row(1).get(&"info".into()).unwrap().get(&"size".into()),
        Ok(Value::Number(5.0))
    );
}
