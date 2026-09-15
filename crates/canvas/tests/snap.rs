//! Boxes settle onto lines other nodes share, else onto the grid.

use canvas::{
    Canvas, CanvasView, Snap, layout,
    model::{Node, TEXT},
    snap::{self, Axis, Guide},
};
use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size};

fn node(id: &str, x: i64, y: i64) -> Node {
    Node {
        id: id.into(),
        kind: TEXT.into(),
        x,
        y,
        width: 100,
        height: 40,
        ..Node::default()
    }
}

#[test]
fn a_box_catches_on_a_line_another_shares() {
    let canvas = Canvas {
        nodes: vec![node("a", 0, 0), node("b", 300, 200)],
        ..Canvas::default()
    };
    let guides_only = Snap {
        grid: None,
        guides: true,
    };
    let (at, guides) = snap::settle(&canvas, &["b".into()], (3, 200), (100, 40), guides_only, 6);
    assert_eq!(at, (0, 200));
    assert_eq!(
        guides,
        [Guide {
            axis: Axis::X,
            at: 0,
            from: 0,
            to: 240
        }]
    );
}

#[test]
fn off_the_lines_a_box_lands_on_the_grid() {
    let both = Snap {
        grid: Some(20),
        guides: true,
    };
    let (at, guides) = snap::settle(&Canvas::default(), &[], (33, 47), (100, 40), both, 6);
    assert_eq!(at, (40, 40));
    assert!(guides.is_empty());
    assert_eq!(snap::round_to(-31, 20), -40);
}

const PAIR: &str = r#"{
  "nodes": [
    {"id":"a","type":"link","x":0,"y":0,"width":200,"height":40,"url":"a"},
    {"id":"b","type":"link","x":400,"y":300,"width":200,"height":40,"url":"b"}
  ]
}"#;

#[gpui::test]
fn a_drag_on_a_grid_lands_on_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let grid = Snap {
        grid: Some(20),
        guides: false,
    };
    let window = cx.add_window(move |_, cx| {
        CanvasView::new(Canvas::parse(PAIR).unwrap(), cx)
            .with_layout(layout::FREE)
            .with_snap(grid)
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    let (at, zoom) = cx.update(|_, cx| {
        let view = view.read(cx);
        let (bounds, pan, zoom) = (view.bounds().unwrap(), view.pan(), view.zoom());
        let at = bounds.origin + point(px(pan.x + 100.0 * zoom), px(pan.y + 20.0 * zoom));
        (at, zoom)
    });
    let to = at + point(px(33.0 * zoom), px(47.0 * zoom));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());

    let a = cx.update(|_, cx| view.read(cx).canvas().node("a").cloned().unwrap());
    assert_eq!((a.x, a.y), (40, 40));
}
