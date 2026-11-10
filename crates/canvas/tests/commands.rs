use canvas::{
    Canvas, CanvasEditor, Options,
    handle::Handle,
    kind::Capability,
    layout,
    model::{End, Node, Side},
};

fn editor() -> CanvasEditor {
    CanvasEditor::new(Canvas::parse(r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":100,"height":40,"text":"a"},{"id":"b","type":"text","x":200,"y":100,"width":100,"height":40,"text":"b"}]}"#).unwrap(), layout::FREE)
}

#[test]
fn grouping_names_the_container_and_undoes_once() {
    let mut editor = editor();
    let before = editor.canvas().clone();
    editor.select_all();
    let group = editor.group_selected("Ideas", 20).unwrap();
    let node = editor.canvas().node(&group).unwrap();
    assert_eq!(
        (node.x, node.y, node.width, node.height),
        (-20, -20, 340, 180)
    );
    for id in ["a", "b"] {
        assert_eq!(
            canvas::contain::named(editor.canvas().node(id).unwrap()),
            Some(group.as_str())
        );
    }
    assert!(editor.undo());
    assert_eq!(editor.canvas(), &before);
    assert!(!editor.can_undo());
}

#[test]
fn colors_and_arrowheads_are_undoable_commands() {
    let mut editor = editor();
    editor.select_all();
    editor.set_selection_color(Some("4".into()));
    assert!(
        editor
            .canvas()
            .nodes
            .iter()
            .all(|node| node.color.as_deref() == Some("4"))
    );
    editor.connect("a", &Handle::connect(Side::Right), (250, 120));
    let edge = editor.canvas().edges[0].id.clone();
    editor.select_edge(Some(edge.clone()));
    editor.set_connection_ends(End::Arrow, End::Arrow);
    assert_eq!(
        editor.canvas().edge(&edge).unwrap().from_end,
        Some(End::Arrow)
    );
    assert!(editor.undo());
    assert_eq!(editor.canvas().edge(&edge).unwrap().from_end, None);
}

#[test]
fn read_only_blocks_user_commands_without_discarding_history() {
    let mut editor = editor();
    editor.select(Some("a".into()));
    editor.nudge(layout::Arrow::Right);
    let before = editor.canvas().clone();
    editor.set_options(Options {
        read_only: true,
        ..editor.options()
    });
    assert!(editor.node_can("a", Capability::Selectable));
    assert!(!editor.node_can("a", Capability::Draggable));
    assert!(editor.copy().is_some());
    assert!(!editor.undo());
    assert!(!editor.redo());
    assert!(!editor.can_undo());
    editor.set_selection_color(Some("1".into()));
    editor.remove_selected();
    assert!(editor.add_root((0, 0)).is_none());
    assert!(editor.group_selected("Group", 20).is_none());
    editor.paste("new card");
    editor.pan_by(10.0, 20.0);
    editor.zoom_in();
    assert_eq!(editor.canvas(), &before);
    editor.set_options(Options {
        read_only: false,
        ..editor.options()
    });
    assert!(editor.can_undo());
    assert!(editor.undo());
    assert_eq!(editor.canvas().node("a").unwrap().x, 0);
}

#[test]
fn supplied_nodes_keep_their_content_and_receive_a_fresh_id() {
    let mut editor = editor();
    let id = editor
        .add_node(
            Node {
                id: "a".into(),
                kind: "file".into(),
                file: Some("notes.md".into()),
                width: 300,
                height: 200,
                ..Node::default()
            },
            (80, -40),
        )
        .unwrap();
    let added = editor.canvas().node(&id).unwrap();
    assert_ne!(id, "a");
    assert_eq!((added.x, added.y), (80, -40));
    assert_eq!(added.file.as_deref(), Some("notes.md"));
    assert_eq!(editor.selected(), Some(id.as_str()));
}

#[test]
fn authored_sizes_and_presentation_are_undoable_for_nodes_and_edges() {
    let mut editor = editor();
    editor.select_all();
    let before = editor.canvas().clone();
    assert_eq!(editor.selection_text_size(), Some(16.));
    editor.set_selection_text_size(Some(32.));
    assert_eq!(editor.selection_text_size(), Some(32.));
    assert!(
        editor
            .canvas()
            .nodes
            .iter()
            .all(|node| node.text_size() == 32.)
    );
    assert!(editor.undo());
    assert_eq!(editor.canvas(), &before);
    assert_eq!(
        editor.selection_text_mode(),
        Some(canvas::model::TextMode::Card)
    );
    editor.set_selection_text_mode(canvas::model::TextMode::Label);
    assert_eq!(
        editor.selection_text_mode(),
        Some(canvas::model::TextMode::Label)
    );
    assert!(editor.canvas().nodes.iter().all(|node| node.is_label()));
    assert!(editor.undo());
    assert_eq!(editor.canvas(), &before);

    editor.connect("a", &Handle::connect(Side::Right), (250, 120));
    let edge = editor.canvas().edges[0].id.clone();
    editor.select_edge(Some(edge.clone()));
    editor.set_selection_text_size(Some(48.));
    assert_eq!(editor.canvas().edge(&edge).unwrap().text_size(), 48.);
    assert!(editor.undo());
    assert_eq!(editor.canvas().edge(&edge).unwrap().font_size, None);

    let before = editor.canvas().clone();
    for size in [0., -1., f32::NAN, f32::INFINITY] {
        editor.set_selection_text_size(Some(size));
        assert_eq!(editor.canvas(), &before);
    }
}
