//! The handles a kind declares: where they sit, and what dragging one does.

use canvas::{
    Canvas, CanvasView, EdgeKinds, Handle, edge,
    handle::{self, Role, Spot, Which},
    kind::{Capabilities, Kind, Kinds},
    layout,
    model::{Edge, Side},
    path::{self, Ends, Rect},
};
use gpui::{
    Entity, IntoElement, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext,
    div, point, px, size,
};

fn rect() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        w: 200.0,
        h: 40.0,
    }
}

#[test]
fn a_spot_on_a_side_sits_where_it_says() {
    let middle = Spot::Side {
        side: Side::Right,
        at: 0.5,
    };
    let (at, out) = middle.on(rect()).unwrap();
    assert_eq!((at, out), (point(200.0, 20.0), point(1.0, 0.0)));

    // A quarter along the top, rather than the middle of it.
    let quarter = Spot::Side {
        side: Side::Top,
        at: 0.25,
    };
    assert_eq!(quarter.on(rect()).unwrap().0, point(50.0, 0.0));
    assert_eq!(Spot::Corner.on(rect()).unwrap().0, point(200.0, 40.0));
    // An edge's spots are not a box's.
    assert_eq!(Spot::End(Which::To).on(rect()), None);
}

#[test]
fn an_edges_spots_sit_along_its_path() {
    let ends = Ends::between(rect(), Rect { x: 400.0, ..rect() });
    let path = path::line(&ends);
    assert_eq!(
        Spot::End(Which::From).along(&path),
        Some(point(200.0, 20.0))
    );
    assert_eq!(Spot::End(Which::To).along(&path), Some(point(400.0, 20.0)));
    assert_eq!(Spot::Along(0.5).along(&path), Some(point(300.0, 20.0)));
    assert_eq!(Spot::Corner.along(&path), None);
}

#[test]
fn an_edge_leaves_the_spot_a_handle_names() {
    // A quarter along the top, which no side could say on its own.
    let quarter = Spot::Side {
        side: Side::Top,
        at: 0.25,
    };
    let named = Ends {
        from_anchor: quarter.on(rect()),
        ..Ends::between(rect(), Rect { x: 400.0, ..rect() })
    };
    assert_eq!(path::line(&named).start(), point(50.0, 0.0));
    // Naming none, it leaves the side the boxes face each other on.
    let plain = Ends::between(rect(), Rect { x: 400.0, ..rect() });
    assert_eq!(path::line(&plain).start(), point(200.0, 20.0));
}

#[test]
fn the_spec_kinds_declare_sides_and_a_corner() {
    let node = canvas::model::Node::default();
    let declared = handle::sides_and_corner(&node);
    let ids: Vec<&str> = declared.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(ids, ["top", "right", "bottom", "left", "corner"]);
    assert_eq!(declared[0].role, Role::Connect);
    assert_eq!(declared[0].side(), Some(Side::Top));
    assert_eq!(declared[4].role, Role::Resize);
    assert_eq!(declared[4].side(), None);

    let edge = Edge::new("e", "a", "b");
    let ends = handle::both_ends(&edge);
    assert_eq!(ends[0].role, Role::Reconnect(Which::From));
    assert_eq!(ends[1].role, Role::Reconnect(Which::To));
}

#[test]
fn a_kind_declares_what_it_likes() {
    let bare = Kind::new(|_, _, _, _| div().into_any_element()).handles(handle::bare_node);
    assert!((bare.rules.handles)(&canvas::model::Node::default()).is_empty());

    let one = Kind::new(|_, _, _, _| div().into_any_element()).handles(|_| {
        vec![Handle::new(
            "tip",
            Spot::Side {
                side: Side::Right,
                at: 0.0,
            },
            Role::Connect,
        )]
    });
    let declared = (one.rules.handles)(&canvas::model::Node::default());
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].id, "tip");

    let plain = EdgeKinds::new().get(&Edge::new("e", "a", "b")).clone();
    assert_eq!((plain.rules.handles)(&Edge::new("e", "a", "b")).len(), 2);
    let bare = edge::curve().handles(handle::bare_edge);
    assert!((bare.rules.handles)(&Edge::new("e", "a", "b")).is_empty());
}

/// Two cards joined left to right; `wall` declares no handles at all.
const DOC: &str = r#"{
  "nodes": [
    {"id":"a","type":"card","x":0,"y":0,"width":200,"height":40},
    {"id":"b","type":"card","x":400,"y":0,"width":200,"height":40},
    {"id":"wall","type":"wall","x":0,"y":300,"width":200,"height":40}
  ],
  "edges": [{"id":"e","fromNode":"a","fromSide":"right","toNode":"b","toSide":"left"}]
}"#;

fn open(cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let blank = || Kind::new(|_, _, _, _| div().into_any_element());
    let kinds = Kinds::new().with("card", blank()).with(
        "wall",
        blank().handles(handle::bare_node).can(Capabilities::ALL),
    );
    let doc = Canvas::parse(DOC).unwrap();
    let window = cx.add_window(move |_, cx| {
        CanvasView::new(doc, layout::FREE, cx)
            .with_kinds(kinds)
            .with_edge_kinds(EdgeKinds::new())
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    draw(&mut cx);
    (view, cx)
}

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

fn screen(view: &Entity<CanvasView>, at: (f32, f32), cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.update(|_, cx| {
        let view = view.read(cx);
        let (pan, zoom) = (view.editor().pan(), view.editor().zoom());
        view.bounds().unwrap().origin + point(px(pan.x + at.0 * zoom), px(pan.y + at.1 * zoom))
    })
}

fn select(view: &Entity<CanvasView>, id: &str, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select(Some(id.into())))
        })
    });
    draw(cx);
}

fn drag(from: Point<Pixels>, to: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    draw(cx);
}

#[gpui::test]
fn a_kind_with_no_handles_has_none_to_pull(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    select(&view, "wall", &mut cx);
    // Where its corner would be, had it declared one.
    drag(
        screen(&view, (200.0, 340.0), &mut cx),
        screen(&view, (260.0, 380.0), &mut cx),
        &mut cx,
    );
    let wall = cx.update(|_, cx| view.read(cx).editor().canvas().node("wall").cloned());
    let wall = wall.expect("still there");
    assert_eq!((wall.width, wall.height), (200, 40), "the wall resized");
}

#[gpui::test]
fn a_declared_corner_still_resizes(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    select(&view, "a", &mut cx);
    drag(
        screen(&view, (200.0, 40.0), &mut cx),
        screen(&view, (250.0, 60.0), &mut cx),
        &mut cx,
    );
    let node = cx.update(|_, cx| view.read(cx).editor().canvas().node("a").cloned());
    let node = node.expect("still there");
    assert_eq!((node.width, node.height), (250, 60));
}

#[gpui::test]
fn an_edge_end_is_carried_onto_another_node(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.update_editor(cx, |editor| editor.select_edge(Some("e".into())))
        })
    });
    draw(&mut cx);
    // The end that arrives at `b`, carried onto `wall`.
    drag(
        screen(&view, (400.0, 20.0), &mut cx),
        screen(&view, (100.0, 320.0), &mut cx),
        &mut cx,
    );
    let edge = cx.update(|_, cx| view.read(cx).editor().canvas().edge("e").cloned());
    let edge = edge.expect("still there");
    assert_eq!(edge.to_node, "wall");
    assert_eq!(edge.from_node, "a");
}
