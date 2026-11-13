use canvas::{
    Canvas, CanvasView, Kinds,
    kind::{self, Control},
    layout,
    model::{self, Node, TextMode},
};
use gpui::{TestAppContext, WindowHandle};

fn open(cx: &mut TestAppContext) -> WindowHandle<CanvasView> {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    cx.add_window(|_, cx| CanvasView::new(Canvas::default(), layout::FREE, cx))
}

#[gpui::test]
fn link_creation_waits_for_confirmation_and_undo_removes_it(cx: &mut TestAppContext) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.create_node(
                Node {
                    kind: model::LINK.into(),
                    url: Some("https://example.com/page".into()),
                    ..Node::default()
                },
                (10, 20),
                window,
                cx,
            );
            assert!(view.editor().canvas().nodes.is_empty());
            assert!(!view.editor().can_undo());
            view.confirm_editing(window, cx);
            let node = &view.editor().canvas().nodes[0];
            assert_eq!(node.url.as_deref(), Some("https://example.com/page"));
            assert_eq!((node.x, node.y), (10, 20));
            view.update_editor(cx, |editor| {
                editor.undo();
            });
            assert!(view.editor().canvas().nodes.is_empty());
        })
        .unwrap();
}

#[gpui::test]
fn cancelling_a_link_draft_does_not_change_the_document(cx: &mut TestAppContext) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.create_node(
                Node {
                    kind: model::LINK.into(),
                    ..Node::default()
                },
                (0, 0),
                window,
                cx,
            );
            view.stop_editing(window, cx);
            assert!(view.editor().canvas().nodes.is_empty());
            assert!(!view.editor().can_undo());
        })
        .unwrap();
}

#[gpui::test]
fn an_incomplete_url_cannot_become_a_link(cx: &mut TestAppContext) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.create_node(
                Node {
                    kind: model::LINK.into(),
                    url: Some("https://".into()),
                    ..Node::default()
                },
                (0, 0),
                window,
                cx,
            );
            view.confirm_editing(window, cx);
            assert!(view.editor().canvas().nodes.is_empty());
            assert!(!view.editor().can_undo());
        })
        .unwrap();
}

#[gpui::test]
fn leaving_new_empty_text_discards_it_but_keeps_groups(cx: &mut TestAppContext) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.create_node(
                Node {
                    kind: model::TEXT.into(),
                    text_mode: Some(TextMode::Label),
                    ..Node::default()
                },
                (0, 0),
                window,
                cx,
            );
            view.stop_editing(window, cx);
            assert!(view.editor().canvas().nodes.is_empty());
            view.create_node(
                Node {
                    kind: model::GROUP.into(),
                    ..Node::default()
                },
                (0, 0),
                window,
                cx,
            );
            view.stop_editing(window, cx);
            assert_eq!(view.editor().canvas().nodes.len(), 1);
        })
        .unwrap();
}

#[gpui::test]
fn leaving_existing_empty_text_does_not_delete_it(cx: &mut TestAppContext) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.update_editor(cx, |editor| {
                editor.add_node(
                    Node {
                        kind: model::TEXT.into(),
                        ..Node::default()
                    },
                    (0, 0),
                );
            });
            view.edit_selected(window, cx);
            view.stop_editing(window, cx);
            assert_eq!(view.editor().canvas().nodes.len(), 1);
        })
        .unwrap();
}

#[test]
fn text_presentation_selects_the_editing_control_without_changing_content() {
    let kinds = Kinds::new();
    let field = kinds.get(model::TEXT).rules.edit.as_ref().unwrap();
    let mut node = Node {
        kind: model::TEXT.into(),
        text: Some("Keep this text".into()),
        ..Node::default()
    };
    assert_eq!((field.control)(&node), Control::Document);
    node.text_mode = Some(TextMode::Label);
    assert_eq!((field.control)(&node), Control::Text);
    assert_eq!((field.read)(&node), "Keep this text");
    let group = kind::group();
    assert_eq!((group.rules.edit.unwrap().control)(&node), Control::Name);
}

#[gpui::test]
fn read_only_cannot_commit_an_open_link_draft(cx: &mut TestAppContext) {
    let window = open(cx);
    window
        .update(cx, |view, window, cx| {
            view.create_node(
                Node {
                    kind: model::LINK.into(),
                    url: Some("https://example.com".into()),
                    ..Node::default()
                },
                (0, 0),
                window,
                cx,
            );
            view.update_editor(cx, |editor| {
                let mut options = editor.options();
                options.read_only = true;
                editor.set_options(options);
            });
            view.confirm_editing(window, cx);
            assert!(view.editor().canvas().nodes.is_empty());
        })
        .unwrap();
}
