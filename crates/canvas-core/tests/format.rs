use canvas_core::{
    Canvas, json_canvas,
    model::{End, Shape, Stroke, VERSION},
};

const OURS: &str = r#"{
  "nodes": [
    {"id":"a","type":"text","x":0,"y":0,"width":120,"height":60,"text":"Ready?","shape":"diamond"},
    {"id":"b","type":"text","x":0,"y":200,"width":120,"height":60,"text":"Ship"}
  ],
  "edges": [
    {"id":"e","fromNode":"a","toNode":"b","toEnd":"circle","points":[[60,60],[60.4,130],[60,200]],"style":"dashed"}
  ]
}"#;

#[test]
fn reads_shapes_routes_strokes_and_ends() {
    let canvas = Canvas::parse(OURS).unwrap();
    assert_eq!(canvas.nodes[0].shape, Some(Shape::Diamond));
    assert_eq!(canvas.nodes[1].shape, None);
    let edge = &canvas.edges[0];
    assert_eq!(edge.points, vec![[60, 60], [60, 130], [60, 200]]);
    assert_eq!(edge.style, Some(Stroke::Dashed));
    assert_eq!(edge.to_end, Some(End::Circle));
}

#[test]
fn a_save_is_stamped_with_the_release_that_wrote_it() {
    let saved = Canvas::parse(&Canvas::parse(OURS).unwrap().to_json()).unwrap();
    assert_eq!(saved.version.as_deref(), Some(VERSION));
}

#[test]
fn json_canvas_out_drops_what_the_spec_cannot_say() {
    let canvas = Canvas::parse(OURS).unwrap();
    let spec: serde_json::Value = serde_json::from_str(&json_canvas::export(&canvas)).unwrap();
    assert!(spec.get("version").is_none());
    assert!(spec["nodes"][0].get("shape").is_none());
    let edge = &spec["edges"][0];
    assert!(edge.get("points").is_none() && edge.get("style").is_none());
    assert_eq!(edge["toEnd"], "arrow");
    assert_eq!(edge["fromNode"], "a");
}

#[test]
fn json_canvas_in_names_no_version() {
    let stamped = Canvas::parse(OURS).unwrap().to_json();
    let canvas = json_canvas::import(&stamped).unwrap();
    assert_eq!(canvas.version, None);
    assert_eq!(canvas.nodes.len(), 2);
}
