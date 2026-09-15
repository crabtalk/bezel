//! Two thousand nodes lay out and paint in a reasonable time. The bounds are
//! loose, for an unoptimised build on a slow machine; they catch a quadratic
//! walk, not a slow frame.

use std::time::Instant;

use canvas::{
    Canvas, CanvasView,
    mindmap::{self, Flow},
    model::{Edge, Node},
};
use gpui::{TestAppContext, VisualTestContext, px, size};

const NODES: usize = 2000;

/// A tree of `n` links, eight children to a node.
fn forest(n: usize) -> Canvas {
    Canvas {
        nodes: (0..n)
            .map(|i| Node {
                id: format!("n{i}"),
                kind: "link".into(),
                width: 160,
                height: 32,
                url: Some(format!("https://example.com/{i}")),
                ..Node::default()
            })
            .collect(),
        edges: (1..n)
            .map(|i| {
                Edge::new(
                    format!("e{i}"),
                    format!("n{}", (i - 1) / 8),
                    format!("n{i}"),
                )
            })
            .collect(),
        ..Canvas::default()
    }
}

#[test]
fn two_thousand_nodes_lay_out() {
    let canvas = forest(NODES);
    let started = Instant::now();
    let moves = mindmap::arrange(&canvas, None, Flow::Right);
    let took = started.elapsed();
    assert_eq!(moves.len(), NODES - 1);
    assert!(took.as_secs_f32() < 2.0, "layout took {took:?}");
}

#[gpui::test]
fn two_thousand_nodes_paint(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let window = cx.add_window(|_, cx| CanvasView::new(forest(NODES), cx));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let started = Instant::now();
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let took = started.elapsed();
    assert!(took.as_secs_f32() < 20.0, "three frames took {took:?}");
}
