use crate::{
    layout,
    model::{Canvas, LINK, Node},
    view::{CanvasView, editing::Input},
};
use gpui::TestAppContext;

#[gpui::test]
fn existing_link_edits_preserve_urls_and_paths_and_recover_from_errors(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| {
        CanvasView::new(
            Canvas {
                nodes: vec![Node {
                    id: "link".into(),
                    kind: LINK.into(),
                    url: Some("a".into()),
                    ..Node::default()
                }],
                ..Canvas::default()
            },
            layout::FREE,
            cx,
        )
    });
    for value in [
        "a",
        "../notes.md",
        "/tmp/notes.md",
        "file:///tmp/notes.md",
        "mailto:hello@example.com",
        "https://example.com",
    ] {
        window
            .update(cx, |view, window, cx| {
                view.update_editor(cx, |editor| editor.select(Some("link".into())));
                view.edit_selected(window, cx);
                let original = view.editor.canvas().node("link").unwrap().url.clone();
                let Input::Field { field, .. } = &view.editing.as_ref().unwrap().input else {
                    panic!("URL field expected")
                };
                let field = field.clone();
                for invalid in ["https://", ""] {
                    field.update(cx, |field, cx| field.set_content(invalid, cx));
                    view.confirm_editing(window, cx);
                    assert!(view.editing.as_ref().unwrap().error.is_some());
                    assert_eq!(view.editor.canvas().node("link").unwrap().url, original);
                }
                field.update(cx, |field, cx| field.set_content(value, cx));
                view.confirm_editing(window, cx);
                assert!(view.editing.is_none());
                assert_eq!(
                    view.editor.canvas().node("link").unwrap().url.as_deref(),
                    Some(value)
                );
            })
            .unwrap();
    }
}
