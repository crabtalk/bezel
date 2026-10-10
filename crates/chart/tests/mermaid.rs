use chart::{Column, Kind, Mark, mermaid};

#[test]
fn a_pie_becomes_arcs_coloured_by_label() {
    let chart = mermaid::import("pie title Pets\n  \"Dogs\" : 386\n  \"Cats\" : 85\n").unwrap();
    assert_eq!(chart.mark, Mark::Arc);
    assert_eq!(chart.title.as_deref(), Some("Pets"));
    let Some(Column::Text(labels)) = chart.data.column("label") else {
        panic!()
    };
    assert_eq!(&*labels.names, ["Dogs", "Cats"]);
    assert_eq!(chart.encoding.color.unwrap().kind, Kind::Nominal);
}

#[test]
fn an_xychart_of_bars_becomes_bars_by_category() {
    let source = "xychart-beta\n  title \"Sales\"\n  x-axis [jan, feb, mar]\n  y-axis \"Revenue\" 0 --> 100\n  bar [10, 20, 30]\n";
    let chart = mermaid::import(source).unwrap();
    assert_eq!(chart.mark, Mark::Bar);
    assert_eq!(chart.data.len(), 3);
    assert!(chart.encoding.color.is_none());
    let Some(Column::Text(x)) = chart.data.column("x") else {
        panic!()
    };
    assert_eq!(&*x.names, ["jan", "feb", "mar"]);
}

#[test]
fn several_lines_are_coloured_by_series() {
    let source = "xychart-beta\n  x-axis [a, b]\n  line [1, 2]\n  line [3, 4]\n";
    let chart = mermaid::import(source).unwrap();
    assert_eq!(chart.mark, Mark::Line);
    assert_eq!(chart.data.len(), 4);
    assert!(chart.encoding.color.is_some());
}

#[test]
fn mixed_series_and_other_kinds_are_none() {
    assert!(
        mermaid::import("xychart-beta\n  x-axis [a, b]\n  bar [1, 2]\n  line [3, 4]\n").is_none()
    );
    assert!(mermaid::import("flowchart LR\n  A --> B\n").is_none());
    assert!(mermaid::import("pie\n").is_none());
}
