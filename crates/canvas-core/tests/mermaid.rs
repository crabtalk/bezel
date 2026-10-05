use canvas_core::{
    mermaid,
    model::{End, GROUP, Shape, Stroke},
};

const FLOW: &str = "flowchart TD
  subgraph build
    a[Parse] --> b{Valid?}
  end
  b -->|yes| c([Paint])
  b -.-> d((Retry))
  c --o e[(Store)]";

#[test]
fn a_flowchart_becomes_shaped_nodes_routed_edges_and_groups() {
    let canvas = mermaid::import(FLOW, 12.0).unwrap();
    let node = |id: &str| canvas.node(id).unwrap();
    assert_eq!(node("a").text.as_deref(), Some("Parse"));
    assert_eq!(node("b").shape, Some(Shape::Diamond));
    assert_eq!(node("c").shape, Some(Shape::Stadium));
    assert_eq!(node("d").shape, Some(Shape::Circle));
    assert_eq!(node("e").shape, Some(Shape::Cylinder));
    let group = canvas.nodes.iter().find(|node| node.kind == GROUP).unwrap();
    assert_eq!(group.label.as_deref(), Some("build"));

    let edge = |from: &str, to: &str| {
        canvas
            .edges
            .iter()
            .find(|edge| edge.from_node == from && edge.to_node == to)
            .unwrap()
    };
    assert_eq!(edge("b", "c").label.as_deref(), Some("yes"));
    assert!(edge("a", "b").points.len() >= 2);
    assert_eq!(edge("a", "b").to_end, Some(End::Arrow));
    assert_eq!(edge("b", "d").style, Some(Stroke::Dotted));
    assert_eq!(edge("c", "e").to_end, Some(End::Circle));
}

#[test]
fn what_is_not_a_graph_or_does_not_parse_answers_none() {
    assert!(mermaid::import("pie title Pets\n  \"Dogs\" : 3\n  \"Cats\" : 2", 12.0).is_none());
    assert!(mermaid::import("flowchart LR\n  A -->", 12.0).is_none());
}
