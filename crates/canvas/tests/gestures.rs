use canvas::{
    Canvas, CanvasEditor, Snap,
    handle::{Handle, Role, Spot},
    layout,
    model::Side,
    tool::{Hand, Hit, Pointer, Resize, Select, SpacePan, Tool},
};
use gpui::{CursorStyle, Modifiers, MouseButton, point, px};

fn editor() -> CanvasEditor {
    CanvasEditor::new(Canvas::parse(r#"{"nodes":[{"id":"a","type":"link","x":100,"y":100,"width":200,"height":100},{"id":"b","type":"link","x":500,"y":300,"width":100,"height":100}]}"#).unwrap(), layout::FREE)
}

fn pointer(hit: Hit) -> Pointer {
    Pointer {
        screen: point(px(0.0), px(0.0)),
        at: point(0.0, 0.0),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        space: false,
        clicks: 1,
        hit,
    }
}

#[test]
fn resizing_the_top_left_preserves_the_opposite_corner_and_undoes_once() {
    let mut editor = editor();
    let before = editor.canvas().clone();
    let mut tool = Resize::default();
    let mut pointer = pointer(Hit::Handle {
        owner: "a".into(),
        handle: Handle::new(
            "nw",
            Spot::CornerAt {
                right: false,
                bottom: false,
            },
            Role::Resize,
        ),
    });
    assert!(tool.press(&pointer, &mut Hand::new(&mut editor)));
    pointer.at = point(-40.0, -20.0);
    pointer.modifiers.shift = true;
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    tool.release(&pointer, &mut Hand::new(&mut editor));
    let node = editor.canvas().node("a").unwrap();
    assert_eq!(
        (node.x, node.y, node.width, node.height),
        (60, 80, 240, 120)
    );
    assert!(editor.undo());
    assert_eq!(editor.canvas(), &before);
    assert!(!editor.can_undo());
}

#[test]
fn a_side_handle_only_changes_its_axis_and_space_bypasses_the_grid() {
    let mut editor = editor().with_snap(Snap {
        grid: Some(20),
        guides: false,
    });
    let mut tool = Resize::default();
    let mut pointer = pointer(Hit::Handle {
        owner: "a".into(),
        handle: Handle::new(
            "left",
            Spot::Side {
                side: Side::Left,
                at: 0.5,
            },
            Role::Resize,
        ),
    });
    tool.press(&pointer, &mut Hand::new(&mut editor));
    pointer.at = point(-13.0, -70.0);
    pointer.space = true;
    tool.release(&pointer, &mut Hand::new(&mut editor));
    let node = editor.canvas().node("a").unwrap();
    assert_eq!(
        (node.x, node.y, node.width, node.height),
        (87, 100, 213, 100)
    );
}

#[test]
fn movement_constrains_an_axis_and_bypasses_snapping() {
    let mut editor = editor().with_snap(Snap {
        grid: Some(20),
        guides: true,
    });
    let mut tool = Select::default();
    let mut pointer = pointer(Hit::Node("a".into()));
    tool.press(&pointer, &mut Hand::new(&mut editor));
    pointer.at = point(33.0, 11.0);
    pointer.modifiers.shift = true;
    pointer.space = true;
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    tool.release(&pointer, &mut Hand::new(&mut editor));
    let node = editor.canvas().node("a").unwrap();
    assert_eq!((node.x, node.y), (133, 100));
    assert_eq!(editor.canvas().nodes.last().unwrap().id, "a");
    assert!(editor.undo());
    assert_eq!(editor.canvas().nodes[0].id, "a");
    assert_eq!(editor.canvas().node("a").unwrap().x, 100);
    assert!(!editor.can_undo());
}

#[test]
fn alt_drag_previews_a_copy_and_commits_it_as_one_step() {
    let mut editor = editor();
    let before = editor.canvas().clone();
    let mut tool = Select::default();
    let mut pointer = pointer(Hit::Node("a".into()));
    pointer.modifiers.alt = true;
    tool.press(&pointer, &mut Hand::new(&mut editor));
    pointer.at = point(50.0, 80.0);
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    assert_eq!(editor.canvas(), &before);
    assert_eq!(editor.painted().nodes.len(), 3);
    tool.release(&pointer, &mut Hand::new(&mut editor));
    let copy = editor.canvas().nodes.last().unwrap();
    assert_eq!((copy.x, copy.y), (150, 180));
    assert_ne!(copy.id, "a");
    assert_eq!(editor.selected(), Some(copy.id.as_str()));
    assert!(editor.undo());
    assert_eq!(editor.canvas(), &before);
    assert!(!editor.can_undo());
}

#[test]
fn cancelling_alt_drag_leaves_no_copy_or_history() {
    let mut editor = editor();
    let before = editor.canvas().clone();
    let mut tool = Select::default();
    let mut pointer = pointer(Hit::Node("a".into()));
    pointer.modifiers.alt = true;
    tool.press(&pointer, &mut Hand::new(&mut editor));
    pointer.at = point(50.0, 80.0);
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    tool.cancel(&mut Hand::new(&mut editor));
    assert_eq!(editor.painted(), &before);
    assert!(!editor.can_undo());
}

#[test]
fn space_drag_pans_over_a_node_without_changing_selection() {
    let mut editor = editor();
    editor.select(Some("a".into()));
    let mut tool = SpacePan::default();
    let mut pointer = pointer(Hit::Node("a".into()));
    pointer.space = true;
    assert!(tool.press(&pointer, &mut Hand::new(&mut editor)));
    pointer.screen = point(px(30.0), px(40.0));
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    tool.release(&pointer, &mut Hand::new(&mut editor));
    assert_eq!(editor.pan(), point(30.0, 40.0));
    assert_eq!(editor.selected(), Some("a"));
    assert!(!editor.can_undo());
}

#[test]
fn shift_drag_can_start_with_shift_already_held() {
    let mut editor = editor();
    editor.select(Some("a".into()));
    let mut tool = Select::default();
    let mut pointer = pointer(Hit::Node("a".into()));
    pointer.modifiers.shift = true;
    tool.press(&pointer, &mut Hand::new(&mut editor));
    pointer.at = point(30.0, 70.0);
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    tool.release(&pointer, &mut Hand::new(&mut editor));
    let node = editor.canvas().node("a").unwrap();
    assert_eq!((node.x, node.y), (100, 170));
}

#[test]
fn resize_keeps_the_pressed_cursor_and_node_until_cancelled() {
    let cases = [
        (
            Spot::Side {
                side: Side::Left,
                at: 0.5,
            },
            CursorStyle::ResizeLeftRight,
        ),
        (
            Spot::Side {
                side: Side::Right,
                at: 0.5,
            },
            CursorStyle::ResizeLeftRight,
        ),
        (
            Spot::Side {
                side: Side::Top,
                at: 0.5,
            },
            CursorStyle::ResizeUpDown,
        ),
        (
            Spot::Side {
                side: Side::Bottom,
                at: 0.5,
            },
            CursorStyle::ResizeUpDown,
        ),
        (
            Spot::CornerAt {
                right: false,
                bottom: false,
            },
            CursorStyle::ResizeUpLeftDownRight,
        ),
        (
            Spot::CornerAt {
                right: true,
                bottom: true,
            },
            CursorStyle::ResizeUpLeftDownRight,
        ),
        (
            Spot::CornerAt {
                right: false,
                bottom: true,
            },
            CursorStyle::ResizeUpRightDownLeft,
        ),
        (
            Spot::CornerAt {
                right: true,
                bottom: false,
            },
            CursorStyle::ResizeUpRightDownLeft,
        ),
    ];
    for (spot, cursor) in cases {
        let mut editor = editor();
        let before = editor.canvas().clone();
        let mut tool = Resize::default();
        let handle = Handle::new("resize", spot, Role::Resize);
        assert_eq!(handle.cursor(), cursor);
        let mut pointer = pointer(Hit::Handle {
            owner: "a".into(),
            handle,
        });
        assert!(tool.press(&pointer, &mut Hand::new(&mut editor)));
        pointer.hit = Hit::Nothing;
        pointer.at = point(40.0, 30.0);
        tool.drag(&pointer, &mut Hand::new(&mut editor));
        assert_eq!(tool.cursor(), Some(cursor));
        assert_eq!(tool.held(), Some("a"));
        tool.cancel(&mut Hand::new(&mut editor));
        assert_eq!(tool.cursor(), None);
        assert_eq!(tool.held(), None);
        assert_eq!(editor.painted(), &before);
    }
}

#[test]
fn resizing_outside_the_viewport_does_not_pan_or_keep_growing() {
    for zoom in [0.25, 1.0, 4.0] {
        let mut editor = editor();
        editor.set_zoom(zoom);
        let mut tool = Resize::default();
        let mut pointer = pointer(Hit::Handle {
            owner: "a".into(),
            handle: Handle::corner(),
        });
        assert!(tool.press(&pointer, &mut Hand::new(&mut editor)));
        pointer.hit = Hit::Nothing;
        pointer.screen = point(px(1200.0), px(900.0));
        pointer.at = point(1200.0 / zoom, 900.0 / zoom);
        let pan = editor.pan();
        tool.drag(&pointer, &mut Hand::new(&mut editor));
        let resized = editor.painted().clone();
        for _ in 0..10 {
            assert!(!tool.drifts());
            tool.drag(&pointer, &mut Hand::new(&mut editor));
            assert_eq!(editor.painted(), &resized);
        }
        assert_eq!(editor.pan(), pan);
        tool.release(&pointer, &mut Hand::new(&mut editor));
        assert_eq!(tool.cursor(), None);
        assert_eq!(tool.held(), None);
        assert_eq!(editor.canvas(), &resized);
        assert!(editor.undo());
        assert!(!editor.can_undo());
    }
}

#[test]
fn grid_drag_follows_each_pointer_step_and_settles_once_on_release() {
    let mut editor = editor().with_snap(Snap {
        grid: Some(20),
        guides: false,
    });
    let mut tool = Select::default();
    let mut pointer = pointer(Hit::Node("a".into()));
    assert!(tool.press(&pointer, &mut Hand::new(&mut editor)));
    for step in 4..=35 {
        pointer.at = point(step as f32, 15.);
        tool.drag(&pointer, &mut Hand::new(&mut editor));
        let node = editor.painted().node("a").unwrap();
        assert_eq!((node.x, node.y), (100 + step, 115));
        assert_eq!(editor.canvas().node("a").unwrap().x, 100);
    }
    tool.release(&pointer, &mut Hand::new(&mut editor));
    let node = editor.canvas().node("a").unwrap();
    assert_eq!((node.x, node.y), (140, 120));
    assert!(editor.undo());
    assert_eq!(editor.canvas().node("a").unwrap().x, 100);
    assert!(!editor.can_undo());
}

#[test]
fn resizing_text_preserves_content_and_custom_fields_through_undo() {
    let mut editor = editor();
    let mut node = editor.canvas().node("a").unwrap().clone();
    node.kind = "text".into();
    node.text = Some("# Title\n\nParagraph one.\n\nParagraph two.".into());
    node.extra
        .insert("custom".into(), serde_json::json!({"keep": true}));
    editor.apply([canvas::Change::UpdateNode { node: node.clone() }]);
    let mut tool = Resize::default();
    let mut pointer = pointer(Hit::Handle {
        owner: "a".into(),
        handle: Handle::corner(),
    });
    assert!(tool.press(&pointer, &mut Hand::new(&mut editor)));
    pointer.at = point(-100., -60.);
    tool.drag(&pointer, &mut Hand::new(&mut editor));
    let preview = editor.painted().node("a").unwrap();
    assert_eq!(preview.text, node.text);
    assert_eq!(preview.extra.get("custom"), node.extra.get("custom"));
    tool.release(&pointer, &mut Hand::new(&mut editor));
    let resized = editor.canvas().node("a").unwrap();
    assert_eq!((resized.width, resized.height), (100, 40));
    assert_eq!(resized.text, node.text);
    assert_eq!(resized.extra.get("custom"), node.extra.get("custom"));
    assert!(editor.undo());
    assert_eq!(editor.canvas().node("a"), Some(&node));
}

#[test]
fn annotation_corners_scale_text_and_sides_only_change_wrapping_width() {
    for (spot, delta, expected) in [
        (Spot::Corner, point(200., 0.), (400, 200, 48.)),
        (
            Spot::Side {
                side: Side::Right,
                at: 0.5,
            },
            point(200., 70.),
            (400, 100, 24.),
        ),
        (
            Spot::CornerAt {
                right: false,
                bottom: false,
            },
            point(-200., 0.),
            (400, 200, 48.),
        ),
    ] {
        let canvas = Canvas::parse(r#"{"nodes":[{"id":"a","type":"text","x":100,"y":100,"width":200,"height":100,"text":"label","textMode":"label","fontSize":24}]}"#).unwrap();
        let mut editor = CanvasEditor::new(canvas, layout::FREE);
        let before = editor.canvas().clone();
        let mut tool = Resize::default();
        let mut pointer = pointer(Hit::Handle {
            owner: "a".into(),
            handle: Handle::new("resize", spot, Role::Resize),
        });
        assert!(tool.press(&pointer, &mut Hand::new(&mut editor)));
        pointer.at = delta;
        tool.drag(&pointer, &mut Hand::new(&mut editor));
        tool.release(&pointer, &mut Hand::new(&mut editor));
        let node = editor.canvas().node("a").unwrap();
        assert_eq!((node.width, node.height, node.text_size()), expected);
        assert_eq!(canvas::kind::min_height(node), None);
        if matches!(
            spot,
            Spot::CornerAt {
                right: false,
                bottom: false
            }
        ) {
            assert_eq!((node.x, node.y), (-100, 0));
        }
        assert!(editor.undo());
        assert_eq!(editor.canvas(), &before);
        assert!(!editor.can_undo());
    }
}
