use canvas::{
    Canvas, CanvasView, Change, Document, DocumentChange, DocumentEvent,
    layout::{self, Arrow},
};
use gpui::{self, AppContext as _, Entity, TestAppContext};
use std::{cell::Cell, rc::Rc};

fn views(cx: &mut TestAppContext) -> (Entity<Document>, Entity<CanvasView>, Entity<CanvasView>) {
    cx.update(|cx| {
        let canvas = Canvas::parse(r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":200,"height":100,"text":"hello"}]}"#).unwrap();
        let document = cx.new(|_| Document::new(canvas));
        let first = cx.new(|cx| CanvasView::for_document(document.clone(), layout::FREE, cx));
        let second = cx.new(|cx| CanvasView::for_document(document.clone(), layout::FREE, cx));
        (document, first, second)
    })
}

#[gpui::test]
fn two_views_share_edits_and_history_but_keep_their_viewports(cx: &mut TestAppContext) {
    let (document, first, second) = views(cx);
    let changes = Rc::new(Cell::new(0));
    let count = changes.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&document, move |_, _: &DocumentEvent, _| {
            count.set(count.get() + 1)
        })
    });
    second.update(cx, |view, cx| {
        view.update_editor(cx, |editor| {
            editor.select(Some("a".into()));
            editor.pan_by(33.0, 44.0);
        })
    });
    first.update(cx, |view, cx| {
        view.update_editor(cx, |editor| {
            editor.submit([Change::MoveNodes {
                moves: vec![("a".into(), (100, 200))],
            }]);
        })
    });
    cx.run_until_parked();
    second.read_with(cx, |view, _| {
        assert_eq!(view.editor().canvas().node("a").unwrap().x, 100);
        assert_eq!(view.editor().selected(), Some("a"));
        assert_eq!(view.editor().pan().x, 33.0);
        assert!(view.editor().can_undo());
    });
    second.update(cx, |view, cx| {
        view.update_editor(cx, |editor| assert!(editor.undo()))
    });
    cx.run_until_parked();
    first.read_with(cx, |view, _| {
        assert_eq!(view.editor().canvas().node("a").unwrap().x, 0);
        assert!(view.editor().can_redo());
        assert_eq!(view.editor().pan().x, 0.0);
    });
    first.update(cx, |view, cx| {
        view.update_editor(cx, |editor| assert!(editor.redo()))
    });
    cx.run_until_parked();
    document.read_with(cx, |document, _| {
        assert_eq!(document.node("a").unwrap().x, 100)
    });
    assert_eq!(changes.get(), 3);
}

#[gpui::test]
fn external_document_edits_prune_selections_and_can_be_undone(cx: &mut TestAppContext) {
    let (document, first, second) = views(cx);
    first.update(cx, |view, cx| {
        view.update_editor(cx, |editor| editor.select(Some("a".into())))
    });
    document.update(cx, |document, cx| {
        document.edit(
            vec![Change::RemoveNodes {
                ids: vec!["a".into()],
            }],
            cx,
        )
    });
    cx.run_until_parked();
    first.read_with(cx, |view, _| assert!(view.editor().selected().is_none()));
    second.update(cx, |view, cx| {
        view.update_editor(cx, |editor| assert!(editor.undo()))
    });
    cx.run_until_parked();
    document.read_with(cx, |document, _| assert!(document.node("a").is_some()));
}

#[gpui::test]
fn reload_preserves_view_state_without_announcing_an_edit(cx: &mut TestAppContext) {
    let (document, first, second) = views(cx);
    first.update(cx, |view, cx| {
        view.update_editor(cx, |editor| {
            editor.select(Some("a".into()));
            editor.pan_by(30.0, 40.0);
            editor.nudge(Arrow::Right);
        })
    });
    cx.run_until_parked();
    let reloads = Rc::new(Cell::new(0));
    let count = reloads.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&document, move |_, event: &DocumentEvent, _| {
            assert_eq!(event.change, DocumentChange::Reloaded);
            count.set(count.get() + 1);
        })
    });
    let mut incoming = document.read_with(cx, |document, _| (**document).clone());
    incoming.nodes[0].text = Some("changed externally".into());
    document.update(cx, |document, cx| document.reload(incoming, cx));
    cx.run_until_parked();
    first.read_with(cx, |view, _| {
        assert_eq!(view.editor().selected(), Some("a"));
        assert_eq!(view.editor().pan(), gpui::point(30.0, 40.0));
        assert!(!view.editor().can_undo());
        assert_eq!(
            view.editor().canvas().nodes[0].text.as_deref(),
            Some("changed externally")
        );
    });
    second.read_with(cx, |view, _| {
        assert_eq!(
            view.editor().canvas().nodes[0].text.as_deref(),
            Some("changed externally")
        )
    });
    assert_eq!(reloads.get(), 1);
}

#[gpui::test]
fn reload_of_unchanged_content_keeps_history(cx: &mut TestAppContext) {
    let (document, first, _) = views(cx);
    first.update(cx, |view, cx| {
        view.update_editor(cx, |editor| {
            editor.select(Some("a".into()));
            editor.nudge(Arrow::Right);
        })
    });
    let incoming = document.read_with(cx, |document, _| (**document).clone());
    document.update(cx, |document, cx| document.reload(incoming, cx));
    assert!(document.read_with(cx, |document, _| document.can_undo()));
}
