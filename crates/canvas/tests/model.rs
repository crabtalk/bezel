use canvas::{
    Canvas,
    model::{End, Side},
};

const SAMPLE: &str = r##"{
  "nodes": [
    {"id":"g","type":"group","x":-20,"y":-20,"width":400,"height":300,"label":"Ideas"},
    {"id":"t","type":"text","x":0,"y":0,"width":200,"height":60.4,"text":"# Hi","color":"4"},
    {"id":"f","type":"file","x":0,"y":100,"width":200,"height":60,"file":"notes/a.md","subpath":"#Top"},
    {"id":"s","type":"session","x":0,"y":200,"width":200,"height":60,"session":"1726000000000"}
  ],
  "edges": [
    {"id":"e","fromNode":"t","fromSide":"right","toNode":"f","toSide":"left","toEnd":"none","label":"see"}
  ],
  "app": {"zoom": 2}
}"##;

#[test]
fn reads_the_spec_and_what_it_does_not_name() {
    let canvas = Canvas::parse(SAMPLE).unwrap();
    assert_eq!(canvas.nodes.len(), 4);
    assert_eq!(canvas.nodes[1].height, 60);
    assert_eq!(canvas.nodes[2].subpath.as_deref(), Some("#Top"));
    assert_eq!(canvas.nodes[3].kind, "session");
    assert!(canvas.nodes[3].extra.contains_key("session"));
    assert_eq!(canvas.edges[0].from_side, Some(Side::Right));
    assert_eq!(canvas.edges[0].to_end, Some(End::None));
    assert!(canvas.extra.contains_key("app"));
}

#[test]
fn round_trips_to_a_fixed_point() {
    let once = Canvas::parse(SAMPLE).unwrap();
    let twice = Canvas::parse(&once.to_json()).unwrap();
    assert_eq!(once, twice);
    assert_eq!(once.to_json(), twice.to_json());
}

#[test]
fn minted_ids_are_free() {
    let mut canvas = Canvas::parse(SAMPLE).unwrap();
    for _ in 0..8 {
        let id = canvas.mint();
        assert!(canvas.node(&id).is_none() && canvas.edges.iter().all(|e| e.id != id));
        canvas.nodes.push(canvas.nodes[1].clone());
        canvas.nodes.last_mut().unwrap().id = id;
    }
}
