//! What an edge is, by its `type`: where it runs, how near a press picks it,
//! and what its label edits.

use canvas::{
    Canvas, CanvasView, EdgeKinds,
    edge::{self, EdgeField},
    layout,
    model::{Edge, Side},
    path::{self, Ends, Rect},
};
use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
    size,
};

/// Two boxes side by side, joined left to right.
fn ends() -> Ends {
    Ends::between(
        Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 40.0,
        },
        Rect {
            x: 400.0,
            y: 0.0,
            w: 200.0,
            h: 40.0,
        },
    )
}

#[test]
fn a_curve_bends_and_a_line_does_not() {
    let ends = ends();
    let curve = path::curve(&ends);
    let line = path::line(&ends);
    // Both leave the right side and arrive at the left.
    assert_eq!(curve.start(), point(200.0, 20.0));
    assert_eq!(curve.end(), point(400.0, 20.0));
    assert_eq!(line.start(), curve.start());
    assert_eq!(line.end(), curve.end());
    assert_eq!((curve.segments.len(), line.segments.len()), (2, 1));
    // Halfway along, both sit between the boxes.
    assert_eq!(curve.middle(), point(300.0, 20.0));
    assert_eq!(line.middle(), point(300.0, 20.0));
    assert!(curve.to_arrow && !curve.from_arrow);
}

#[test]
fn a_press_is_measured_from_the_path() {
    let line = path::line(&ends());
    assert!(line.distance(point(300.0, 20.0)) < 0.01);
    assert!((line.distance(point(300.0, 40.0)) - 20.0).abs() < 0.01);
}

#[test]
fn an_edge_takes_the_kind_its_type_names() {
    let kinds = EdgeKinds::new().with("straight", edge::line().weight(3.0));
    let mut edge = Edge::new("e", "a", "b");
    assert_eq!(kinds.get(&edge).weight, 1.0);
    edge.kind = Some("straight".into());
    assert_eq!(kinds.get(&edge).weight, 3.0);
    // A type nothing names is the plain edge.
    edge.kind = Some("rope".into());
    assert_eq!(kinds.get(&edge).weight, 1.0);
}

#[test]
fn an_edge_type_is_written_back() {
    let json = r#"{"nodes":[],"edges":[{"id":"e","type":"straight","fromNode":"a","toNode":"b"}]}"#;
    let canvas = Canvas::parse(json).unwrap();
    assert_eq!(canvas.edges[0].kind.as_deref(), Some("straight"));
    assert!(canvas.to_json().contains(r#""type": "straight""#));
}

/// One straight edge, which the curve would bow away from.
const PAIR: &str = r#"{
  "nodes": [
    {"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"},
    {"id":"b","type":"link","x":400,"y":200,"width":200,"height":40,"url":"b"}
  ],
  "edges": [{"id":"e","type":"straight","fromNode":"a","toNode":"b"}]
}"#;

fn open(cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let doc = Canvas::parse(PAIR).unwrap();
    let kinds = EdgeKinds::new().with(
        "straight",
        edge::line().edit(Some(EdgeField::new(
            |edge| {
                edge.extra
                    .get("note")
                    .and_then(|note| note.as_str())
                    .unwrap_or_default()
                    .to_owned()
            },
            |edge, note| drop(edge.extra.insert("note".into(), note.into())),
        ))),
    );
    let window =
        cx.add_window(move |_, cx| CanvasView::new(doc, layout::FREE, cx).with_edge_kinds(kinds));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    (view, cx)
}

fn screen(view: &Entity<CanvasView>, at: (f32, f32), cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.update(|_, cx| {
        let view = view.read(cx);
        let (pan, zoom) = (view.editor().pan(), view.editor().zoom());
        view.bounds().unwrap().origin + point(px(pan.x + at.0 * zoom), px(pan.y + at.1 * zoom))
    })
}

#[gpui::test]
fn a_press_picks_the_edge_where_its_kind_draws_it(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    // Halfway along the straight line, which the spec's curve would miss.
    let at = screen(&view, (250.0, 70.0), &mut cx);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    let picked = cx.update(|_, cx| view.read(cx).editor().selected_edge().map(str::to_owned));
    assert_eq!(picked.as_deref(), Some("e"));
}

#[gpui::test]
fn a_kinds_field_is_what_the_label_edits(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    let at = screen(&view, (250.0, 70.0), &mut cx);
    for clicks in 1..=2 {
        cx.simulate_event(gpui::MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: clicks,
            first_mouse: false,
        });
        cx.simulate_event(gpui::MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: clicks,
        });
    }
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    cx.simulate_input("go");
    cx.run_until_parked();
    let edge = cx.update(|_, cx| view.read(cx).editor().canvas().edge("e").cloned());
    let edge = edge.expect("still there");
    // The app's field took it, and the spec's label was left alone.
    assert_eq!(edge.extra.get("note").and_then(|n| n.as_str()), Some("go"));
    assert_eq!(edge.label, None);
}

/// The side an edge leaves from is still the spec's to name.
#[test]
fn a_named_side_wins_over_the_facing_one() {
    let ends = Ends {
        from_side: Some(Side::Top),
        ..ends()
    };
    assert_eq!(path::line(&ends).start(), point(100.0, 0.0));
}
