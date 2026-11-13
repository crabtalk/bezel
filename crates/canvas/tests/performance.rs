use canvas::{
    Canvas, CanvasEditor, layout,
    model::{LINK, Node},
    tool::{Hand, Hit, Pointer, Select, Tool as _},
};
use gpui::{Modifiers, MouseButton, point, px};
use std::{hint, time::Instant};

#[test]
#[ignore]
fn dragging_large_documents() {
    for count in [100, 1_000, 5_000] {
        let canvas = Canvas {
            nodes: (0..count)
                .map(|index| Node {
                    id: index.to_string(),
                    kind: LINK.into(),
                    x: index as i64 * 300,
                    width: 200,
                    height: 100,
                    text: Some("x".repeat(2048)),
                    ..Node::default()
                })
                .collect(),
            ..Canvas::default()
        };
        let mut editor = CanvasEditor::new(canvas, layout::FREE);
        let mut select = Select::default();
        let mut pointer = Pointer {
            screen: point(px(0.), px(0.)),
            at: point(0., 0.),
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            space: false,
            clicks: 1,
            hit: Hit::Node("0".into()),
        };
        select.press(&pointer, &mut Hand::new(&mut editor));
        let start = Instant::now();
        for step in 1..=200 {
            pointer.at = point(step as f32 * 10., 20.);
            select.drag(&pointer, &mut Hand::new(&mut editor));
            editor.reflow([], Some("0"));
            hint::black_box(editor.painted());
        }
        eprintln!("drag {count} nodes, 200 updates: {:?}", start.elapsed());
        select.cancel(&mut Hand::new(&mut editor));
    }
}
