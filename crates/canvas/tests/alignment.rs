use canvas::{Arrange, Canvas, CanvasEditor, layout};

fn editor() -> CanvasEditor {
    let canvas = Canvas::parse(r#"{"nodes":[{"id":"a","type":"link","x":0,"y":0,"width":100,"height":40},{"id":"b","type":"link","x":180,"y":90,"width":80,"height":60},{"id":"c","type":"link","x":400,"y":200,"width":120,"height":80}]}"#).unwrap();
    let mut editor = CanvasEditor::new(canvas, layout::FREE);
    editor.select_all();
    editor
}

#[test]
fn align_uses_card_edges_and_undoes_the_whole_selection() {
    let mut editor = editor();
    let before = editor.canvas().clone();
    editor.arrange(Arrange::Right);
    assert!(
        editor
            .canvas()
            .nodes
            .iter()
            .all(|node| node.x + node.width == 520)
    );
    assert!(editor.undo());
    assert_eq!(editor.canvas(), &before);
    assert!(!editor.can_undo());
}

#[test]
fn distribution_preserves_extremes_and_equalizes_gaps() {
    let mut editor = editor();
    editor.arrange(Arrange::DistributeHorizontal);
    let nodes = &editor.canvas().nodes;
    assert_eq!((nodes[0].x, nodes[2].x), (0, 400));
    assert_eq!(
        nodes[1].x - nodes[0].x - nodes[0].width,
        nodes[2].x - nodes[1].x - nodes[1].width
    );
}

#[test]
fn grid_uses_each_columns_and_rows_largest_card() {
    let mut editor = editor();
    editor.arrange(Arrange::Grid {
        columns: 2,
        gap: 20,
    });
    let nodes = &editor.canvas().nodes;
    assert_eq!((nodes[0].x, nodes[0].y), (0, 0));
    assert_eq!((nodes[1].x, nodes[1].y), (140, 0));
    assert_eq!((nodes[2].x, nodes[2].y), (0, 80));
}

#[test]
fn justify_respects_resize_capabilities() {
    let mut editor = editor();
    let mut locked = editor.canvas().node("b").unwrap().clone();
    locked.extra.insert("resizable".into(), false.into());
    editor.apply([canvas::Change::UpdateNode {
        node: locked.clone(),
    }]);
    editor.arrange(Arrange::JustifyHorizontal);
    assert_eq!(editor.canvas().node("b").unwrap(), &locked);
    for id in ["a", "c"] {
        let node = editor.canvas().node(id).unwrap();
        assert_eq!((node.x, node.width), (0, 520));
    }
}

#[test]
fn arranging_a_group_carries_its_contents_once() {
    let canvas = Canvas::parse(r#"{"nodes":[{"id":"g","type":"group","x":0,"y":0,"width":200,"height":200},{"id":"a","type":"link","x":20,"y":20,"width":100,"height":40},{"id":"b","type":"link","x":400,"y":300,"width":100,"height":80}]}"#).unwrap();
    let mut editor = CanvasEditor::new(canvas, layout::FREE);
    editor.select_all();
    editor.arrange(Arrange::Bottom);
    assert_eq!(editor.canvas().node("g").unwrap().y, 180);
    assert_eq!(editor.canvas().node("a").unwrap().y, 200);
    assert_eq!(editor.canvas().node("b").unwrap().y, 300);
}
