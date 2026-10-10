use chart::{Column, Kind, Mark, mermaid};

#[test]
fn a_pie_becomes_arcs_coloured_by_label() {
    let chart = mermaid::import("pie title Pets\n  \"Dogs\" : 386\n  \"Cats\" : 85\n").unwrap();
    assert_eq!(chart.layers[0].mark, Mark::Arc);
    assert_eq!(chart.title.as_deref(), Some("Pets"));
    let Some(Column::Text(labels)) = chart.layers[0].data.column("label") else {
        panic!()
    };
    assert_eq!(&*labels.names, ["Dogs", "Cats"]);
    assert_eq!(
        chart.layers[0].encoding.color.clone().unwrap().kind,
        Kind::Nominal
    );
}

#[test]
fn an_xychart_of_bars_becomes_bars_by_category() {
    let source = "xychart-beta\n  title \"Sales\"\n  x-axis [jan, feb, mar]\n  y-axis \"Revenue\" 0 --> 100\n  bar [10, 20, 30]\n";
    let chart = mermaid::import(source).unwrap();
    assert_eq!(chart.layers[0].mark, Mark::Bar);
    assert_eq!(chart.layers[0].data.len(), 3);
    assert!(chart.layers[0].encoding.color.is_none());
    let Some(Column::Text(x)) = chart.layers[0].data.column("x") else {
        panic!()
    };
    assert_eq!(&*x.names, ["jan", "feb", "mar"]);
}

#[test]
fn several_lines_are_coloured_by_series() {
    let source = "xychart-beta\n  x-axis [a, b]\n  line [1, 2]\n  line [3, 4]\n";
    let chart = mermaid::import(source).unwrap();
    assert_eq!(chart.layers[0].mark, Mark::Line);
    assert_eq!(chart.layers[0].data.len(), 4);
    assert!(chart.layers[0].encoding.color.is_some());
}

#[test]
fn other_kinds_are_none() {
    assert!(mermaid::import("flowchart LR\n  A --> B\n").is_none());
    assert!(mermaid::import("pie\n").is_none());
}

#[test]
fn an_xychart_keeps_its_axis_titles_and_y_range() {
    let source = "xychart-beta\n  x-axis \"Month\" [jan, feb]\n  y-axis \"Revenue\" 0 --> 100\n  bar [10, 20]\n";
    let chart = mermaid::import(source).unwrap();
    let (x, y) = (
        chart.layers[0].encoding.x.clone().unwrap(),
        chart.layers[0].encoding.y.clone().unwrap(),
    );
    assert_eq!(x.title.as_deref(), Some("Month"));
    assert_eq!(y.title.as_deref(), Some("Revenue"));
    assert_eq!(y.scale.domain, Some([0.0, 100.0]));
}

#[test]
fn bars_and_lines_become_a_bar_layer_under_a_line_layer() {
    let chart =
        mermaid::import("xychart-beta\n  x-axis [a, b]\n  bar [1, 2]\n  line [3, 4]\n").unwrap();
    let marks: Vec<Mark> = chart.layers.iter().map(|layer| layer.mark).collect();
    assert_eq!(marks, [Mark::Bar, Mark::Line]);
    assert!(
        chart
            .layers
            .iter()
            .all(|layer| layer.encoding.color.is_none())
    );
}
