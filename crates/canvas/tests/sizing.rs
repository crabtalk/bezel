use canvas::{
    Canvas, CanvasEditor, Kinds,
    kind::{self, Sizing},
    layout,
};

#[test]
fn fixed_text_cards_keep_geometry_while_growing_cards_reflow() {
    let mut canvas = Canvas::parse(r#"{"nodes":[{"id":"a","type":"text","x":40,"y":70,"width":200,"height":100,"text":"long content"}]}"#).unwrap();
    let mut growing = canvas.nodes[0].clone();
    growing.id = "growing".into();
    growing.kind = "growing".into();
    canvas.nodes.push(growing);
    let mut editor = CanvasEditor::new(canvas.clone(), layout::FREE).with_kinds(
        Kinds::new()
            .with("text", kind::text().sizing(Sizing::Fixed))
            .with("growing", kind::text()),
    );
    editor.reflow([("growing".into(), 900)], None);
    assert_eq!(editor.canvas().node("a"), canvas.node("a"));
    assert_eq!(editor.canvas().node("growing").unwrap().height, 900);
}

#[test]
fn growing_text_cards_still_accept_measured_content_height() {
    let canvas = Canvas::parse(r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":200,"height":100,"text":"long content"}]}"#).unwrap();
    let mut editor = CanvasEditor::new(canvas, layout::FREE);
    editor.reflow([("a".into(), 900)], None);
    assert_eq!(editor.canvas().node("a").unwrap().height, 900);
}
