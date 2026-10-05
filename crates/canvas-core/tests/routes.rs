use canvas_core::{
    Canvas, Change, change,
    model::{End, Shape},
    path::{self, Ends, Rect},
};
use gpui::point;

const ROUTED: &str = r#"{
  "nodes": [
    {"id":"a","type":"text","x":0,"y":0,"width":100,"height":40},
    {"id":"b","type":"text","x":0,"y":200,"width":100,"height":40},
    {"id":"c","type":"text","x":300,"y":0,"width":100,"height":40}
  ],
  "edges": [
    {"id":"ab","fromNode":"a","toNode":"b","points":[[50,40],[80,120],[50,200]]},
    {"id":"ac","fromNode":"a","toNode":"c","points":[[100,20],[300,20]]},
    {"id":"bc","fromNode":"b","toNode":"c","points":[[100,220],[300,40]]}
  ]
}"#;

fn rect(x: f32, y: f32) -> Rect {
    Rect {
        x,
        y,
        w: 100.0,
        h: 40.0,
    }
}

#[test]
fn a_route_runs_through_its_points_and_its_ends_point_back_along_it() {
    let ends = Ends {
        from_end: End::Circle,
        ..Ends::between(rect(0.0, 0.0), rect(0.0, 200.0))
    };
    let path = path::route(&ends, &[[50, 40], [50, 120], [50, 200]]).unwrap();
    assert_eq!(path.segments.len(), 2);
    assert_eq!(
        (path.start(), path.end()),
        (point(50.0, 40.0), point(50.0, 200.0))
    );
    // Out of the start is down the line; out of the end, back up it.
    assert_eq!(
        (path.from_out, path.to_out),
        (point(0.0, 1.0), point(0.0, -1.0))
    );
    assert_eq!((path.from_end, path.to_end), (End::Circle, End::Arrow));
    assert!(path::route(&ends, &[[0, 0]]).is_none());
}

#[test]
fn moving_a_node_drops_the_routes_it_touches_and_undo_brings_them_back() {
    let before = Canvas::parse(ROUTED).unwrap();
    let mut canvas = before.clone();
    let undo = change::apply_all(
        &mut canvas,
        &[Change::MoveNodes {
            moves: vec![("b".into(), (0, 300))],
        }],
    );
    let routed = |canvas: &Canvas, id: &str| !canvas.edge(id).unwrap().points.is_empty();
    assert!(!routed(&canvas, "ab") && !routed(&canvas, "bc"));
    assert!(
        routed(&canvas, "ac"),
        "an edge away from the move keeps its route"
    );

    change::apply_all(&mut canvas, &undo);
    assert_eq!(canvas, before);
}

#[test]
fn resizing_to_the_same_size_keeps_routes() {
    let mut canvas = Canvas::parse(ROUTED).unwrap();
    change::apply(
        &mut canvas,
        &Change::Resize {
            id: "a".into(),
            size: (100, 40),
        },
    );
    assert!(!canvas.edge("ab").unwrap().points.is_empty());
    assert_eq!(canvas.nodes[0].shape.unwrap_or_default(), Shape::Rect);
}
