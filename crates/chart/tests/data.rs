use chart::{Channel, Chart, Column, Data, Kind, Mark};

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
    assert_eq!(&*month.names, ["Jan", "Feb", "Mar"]);
    assert_eq!(&*month.codes, [0, 1, 0, 2, 1]);
    assert_eq!(month.get(3), "Mar");
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
    assert_eq!(chart.layers[0].encoding.x.clone().unwrap().field, "month");
    assert_eq!(
        chart.layers[0].encoding.y.clone().unwrap().kind,
        Kind::Quantitative
    );
    assert!(chart.layers[0].encoding.color.is_none());
}
