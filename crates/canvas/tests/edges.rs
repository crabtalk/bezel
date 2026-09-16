//! Edges picked, removed and labelled, connectors drawn out of a node's side,
//! and a box pulled by its corner.

use canvas::{Canvas, CanvasView, Layout, layout, mindmap};
use gpui::{
    Entity, Focusable, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point,
    TestAppContext, VisualTestContext, point, px, size,
};

/// Two links, which keep their size, joined left to right.
const PAIR: &str = r#"{
  "nodes": [
    {"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"},
    {"id":"b","type":"link","x":400,"y":0,"width":200,"height":40,"url":"b"}
  ],
  "edges": [{"id":"e","fromNode":"a","fromSide":"right","toNode":"b","toSide":"left"}]
}"#;

fn open(layout: Layout, cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let doc = Canvas::parse(PAIR).unwrap();
    let window = cx.add_window(move |_, cx| CanvasView::new(doc, layout, cx));
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    draw(&mut cx);
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    (view, cx)
}

/// A test window has no frame clock.
fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

fn press(keys: &str, cx: &mut VisualTestContext) {
    let platform = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&keys.replace("cmd", platform));
    cx.run_until_parked();
}

fn doc(view: &Entity<CanvasView>, cx: &mut VisualTestContext) -> Canvas {
    cx.update(|_, cx| view.read(cx).editor().canvas().clone())
}

/// A canvas point, in window coordinates.
fn screen(view: &Entity<CanvasView>, at: (i64, i64), cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.update(|_, cx| {
        let view = view.read(cx);
        let (bounds, pan, zoom) = (
            view.bounds().unwrap(),
            view.editor().pan(),
            view.editor().zoom(),
        );
        bounds.origin
            + point(
                px(pan.x + at.0 as f32 * zoom),
                px(pan.y + at.1 as f32 * zoom),
            )
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

fn double_click(at: Point<Pixels>, cx: &mut VisualTestContext) {
    for click_count in 1..=2 {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count,
        });
    }
}

#[gpui::test]
fn a_click_on_an_edge_picks_it_and_backspace_removes_it(cx: &mut TestAppContext) {
    let (view, mut cx) = open(layout::FREE, cx);
    let middle = screen(&view, (300, 20), &mut cx);
    cx.simulate_mouse_down(middle, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(middle, MouseButton::Left, Modifiers::none());
    let picked = cx.update(|_, cx| view.read(cx).editor().selected_edge().map(str::to_owned));
    assert_eq!(picked.as_deref(), Some("e"));

    press("backspace", &mut cx);
    assert!(doc(&view, &mut cx).edges.is_empty());
    press("cmd-z", &mut cx);
    assert_eq!(doc(&view, &mut cx).edges.len(), 1);
}

#[gpui::test]
fn a_double_click_on_an_edge_edits_its_label(cx: &mut TestAppContext) {
    let (view, mut cx) = open(layout::FREE, cx);
    double_click(screen(&view, (300, 20), &mut cx), &mut cx);
    draw(&mut cx);
    cx.simulate_input("go");
    press("escape", &mut cx);
    let label = doc(&view, &mut cx).edge("e").unwrap().label.clone();
    assert_eq!(label.as_deref(), Some("go"));
}

#[gpui::test]
fn a_side_handle_draws_a_connector(cx: &mut TestAppContext) {
    let (view, mut cx) = open(layout::FREE, cx);
    select(&view, "b", &mut cx);
    // b's bottom handle onto a.
    let (from, to) = (
        screen(&view, (500, 40), &mut cx),
        screen(&view, (100, 20), &mut cx),
    );
    drag(from, to, &mut cx);
    let canvas = doc(&view, &mut cx);
    assert_eq!(canvas.edges.len(), 2);
    let made = canvas.edges.last().unwrap();
    assert_eq!((made.from_node.as_str(), made.to_node.as_str()), ("b", "a"));

    // Onto nothing, a node is made there.
    select(&view, "a", &mut cx);
    let (from, to) = (
        screen(&view, (100, 40), &mut cx),
        screen(&view, (100, 300), &mut cx),
    );
    drag(from, to, &mut cx);
    let canvas = doc(&view, &mut cx);
    assert_eq!((canvas.nodes.len(), canvas.edges.len()), (3, 3));
    let made = canvas.nodes.last().unwrap();
    assert_eq!(canvas.edges.last().unwrap().to_node, made.id);
    assert!((made.y..made.y + made.height).contains(&300));
}

#[gpui::test]
fn under_a_tree_a_connector_to_a_node_is_a_cross_link(cx: &mut TestAppContext) {
    // Laid out before it first paints, so nothing is gliding.
    let (view, mut cx) = open(layout::MINDMAP, cx);
    select(&view, "b", &mut cx);
    let (bx, by, bh) = {
        let b = doc(&view, &mut cx).node("b").cloned().unwrap();
        (b.x, b.y, b.height)
    };
    let a = doc(&view, &mut cx).node("a").cloned().unwrap();
    drag(
        screen(&view, (bx + 100, by + bh), &mut cx),
        screen(&view, (a.x + 100, a.y + 20), &mut cx),
        &mut cx,
    );
    let canvas = doc(&view, &mut cx);
    let made = canvas.edges.last().unwrap();
    assert_eq!(made.to_node, "a");
    assert!(!mindmap::is_branch(made));
    assert_eq!(mindmap::parent(&canvas, "a"), None);
}

#[gpui::test]
fn the_corner_resizes_and_undo_puts_it_back(cx: &mut TestAppContext) {
    let (view, mut cx) = open(layout::FREE, cx);
    select(&view, "b", &mut cx);
    drag(
        screen(&view, (600, 40), &mut cx),
        screen(&view, (650, 60), &mut cx),
        &mut cx,
    );
    let b = doc(&view, &mut cx).node("b").cloned().unwrap();
    assert_eq!((b.width, b.height), (250, 60));
    press("cmd-z", &mut cx);
    let b = doc(&view, &mut cx).node("b").cloned().unwrap();
    assert_eq!((b.width, b.height), (200, 40));
}
