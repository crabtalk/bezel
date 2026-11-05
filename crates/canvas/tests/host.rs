use canvas::{
    Canvas, CanvasEditor, CanvasView, Options,
    handle::Handle,
    layout,
    model::{Node, Side},
    tool::{Connect, Hand, Hit, Pointer, Tool, Wish},
};
use gpui::{AppContext as _, Modifiers, MouseButton, TestAppContext, point, px};

const SOURCE: &str =
    r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":200,"height":100,"text":"source"}]}"#;

#[test]
fn the_host_chooses_the_node_at_a_loose_connection_end() {
    let mut editor =
        CanvasEditor::new(Canvas::parse(SOURCE).unwrap(), layout::FREE).with_options(Options {
            choose_connection_node: true,
            ..Options::default()
        });
    let mut tool = Connect::default();
    let handle = Handle::connect(Side::Right);
    let mut pointer = Pointer {
        screen: point(px(200.0), px(50.0)),
        at: point(200.0, 50.0),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        space: false,
        clicks: 1,
        hit: Hit::Handle {
            owner: "a".into(),
            handle: handle.clone(),
        },
    };
    tool.press(&pointer, &mut Hand::new(&mut editor));
    pointer.at = point(500.0, 300.0);
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    let mut hand = Hand::new(&mut editor);
    tool.release(&pointer, &mut hand);
    assert_eq!(
        hand.wish,
        Wish::ChooseConnection {
            from: "a".into(),
            handle: handle.clone(),
            at: (500, 300)
        }
    );
    assert_eq!(editor.canvas().nodes.len(), 1);
    assert!(!editor.can_undo());
    let added = editor
        .extend_connection(
            "a",
            &handle,
            (500, 300),
            Node {
                kind: "file".into(),
                file: Some("notes.md".into()),
                width: 300,
                height: 200,
                ..Node::default()
            },
        )
        .unwrap();
    assert_eq!(
        editor.canvas().node(&added).unwrap().file.as_deref(),
        Some("notes.md")
    );
    assert_eq!(editor.canvas().edges[0].from_node, "a");
    assert!(editor.undo());
    assert_eq!(editor.canvas().nodes.len(), 1);
    assert!(editor.canvas().edges.is_empty());
}

#[gpui::test]
fn host_hit_testing_uses_the_viewport_transform_and_stacking(cx: &mut TestAppContext) {
    let view = cx.update(|cx| {
        cx.new(|cx| CanvasView::new(Canvas::parse(SOURCE).unwrap(), layout::FREE, cx))
    });
    view.update(cx, |view, cx| {
        view.update_editor(cx, |editor| editor.pan_by(50.0, 70.0))
    });
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.canvas_point(point(px(70.0), px(90.0))),
            point(20.0, 20.0)
        );
        assert_eq!(
            view.item_at(point(px(70.0), px(90.0))),
            Some(canvas::Item::Node("a".into()))
        );
        assert_eq!(view.item_at(point(px(600.0), px(600.0))), None);
    });
}
