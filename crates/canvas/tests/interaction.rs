use canvas::{
    Canvas, CanvasEditor, CanvasEvent, CanvasView, Kinds,
    handle::{self, Role},
    kind, layout,
    model::{self, Node},
    path::Rect,
    tool::{Hand, Hit, Pointer, Select, Tool as _, Wish},
};
use gpui::{Modifiers, MouseButton, TestAppContext, point, px};
use std::{cell::Cell, rc::Rc};

fn canvas() -> Canvas {
    Canvas {
        nodes: vec![Node {
            id: "link".into(),
            kind: model::LINK.into(),
            width: 300,
            height: 200,
            url: Some("https://example.com/".into()),
            ..Node::default()
        }],
        ..Canvas::default()
    }
}

#[test]
fn connectors_leave_every_resize_border_clear_at_every_zoom() {
    let handles = handle::frame(&canvas().nodes[0]);
    for zoom in [0.25, 1., 4.] {
        let rect = Rect {
            x: 50.,
            y: 40.,
            w: 300. * zoom,
            h: 200. * zoom,
        };
        for connector in handles.iter().filter(|handle| handle.role == Role::Connect) {
            let bounds = connector.bounds(rect, 8.).unwrap();
            assert_eq!(bounds.size, gpui::size(8., 8.));
            for resize in handles.iter().filter(|handle| handle.role == Role::Resize) {
                assert!(!bounds.intersects(&resize.bounds(rect, 8.).unwrap()));
            }
            let (at, out) = connector.spot.on(rect).unwrap();
            assert_eq!(bounds.center(), at + out * 12.);
        }
    }
}

#[test]
fn link_cards_select_and_move_without_requesting_activation() {
    let mut editor = CanvasEditor::new(canvas(), layout::FREE);
    let mut select = Select::default();
    let mut pointer = Pointer {
        screen: point(px(10.), px(10.)),
        at: point(10., 10.),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        space: false,
        clicks: 1,
        hit: Hit::Node("link".into()),
    };
    let mut hand = Hand::new(&mut editor);
    assert!(select.press(&pointer, &mut hand));
    assert_eq!(hand.wish, Wish::Nothing);
    assert_eq!(hand.editor.selected(), Some("link"));
    pointer.screen.x += px(30.);
    pointer.at.x += 30.;
    select.drag(&pointer, &mut hand);
    select.release(&pointer, &mut hand);
    assert_eq!(hand.editor.canvas().node("link").unwrap().x, 30);
    assert_eq!(hand.wish, Wish::Nothing);
    pointer.clicks = 2;
    assert!(select.press(&pointer, &mut hand));
    assert_eq!(hand.wish, Wish::Enter("link".into()));
}

#[gpui::test]
fn the_open_action_runs_a_kinds_activation_once(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let count = Rc::new(Cell::new(0));
    let activated = count.clone();
    let window = cx.add_window(|_, cx| {
        CanvasView::new(canvas(), layout::FREE, cx).with_kinds(Kinds::new().with(
            model::LINK,
            kind::link().open(move |_, _| activated.set(activated.get() + 1)),
        ))
    });
    window
        .update(cx, |view, window, cx| {
            assert!(view.open_node("link", window, cx));
        })
        .unwrap();
    assert_eq!(count.get(), 1);
}

#[gpui::test]
fn host_activation_requests_the_original_node_without_mutating_it(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| {
        CanvasView::new(canvas(), layout::FREE, cx)
            .with_kinds(Kinds::new().with(model::LINK, kind::link().open_in_host()))
    });
    let view = window.root(cx).unwrap();
    let count = Rc::new(Cell::new(0));
    let requested = count.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &CanvasEvent, _| {
            if let CanvasEvent::OpenRequested { id } = event {
                assert_eq!(id, "link");
                requested.set(requested.get() + 1);
            }
        })
    });
    window
        .update(cx, |view, window, cx| {
            assert!(view.open_node("link", window, cx));
            assert_eq!(view.editor().canvas(), &canvas());
            assert!(!view.editor().can_undo());
        })
        .unwrap();
    assert_eq!(count.get(), 1);
}
