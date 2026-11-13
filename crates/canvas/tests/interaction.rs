use canvas::{
    Canvas, CanvasEditor, CanvasEvent, CanvasView, Kinds,
    handle::{self, Role},
    kind, layout,
    model::{self, Node},
    path::Rect,
    tool::{Hand, Hit, Pointer, Select, Tool as _, Wish},
};
use gpui::{
    Modifiers, MouseButton, TestAppContext, VisualTestContext, div, point, prelude::*, px, size,
};
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

#[gpui::test]
fn controls_receive_clicks_above_the_drag_surface(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let clicks = Rc::new(Cell::new(0));
    let clicked = clicks.clone();
    let window = cx.add_window(move |_, cx| {
        CanvasView::new(canvas(), layout::FREE, cx).with_kinds(Kinds::new().with(
            model::LINK,
            kind::link().controls(move |_, _, _, _| {
                let clicked = clicked.clone();
                Some(
                    div()
                        .id("node-control")
                        .absolute()
                        .left(px(20.))
                        .top(px(20.))
                        .w(px(40.))
                        .h(px(30.))
                        .debug_selector(|| "node-control".into())
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            clicked.set(clicked.get() + 1);
                            cx.stop_propagation();
                        })
                        .into_any_element(),
                )
            }),
        ))
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.), px(600.)));
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let at = cx.debug_bounds("node-control").unwrap().center();
    cx.simulate_click(at, Modifiers::none());
    assert_eq!(clicks.get(), 1);
    cx.update(|_, cx| assert!(view.read(cx).editor().selected().is_none()));
}

#[gpui::test]
fn external_editing_requests_the_selected_node(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| {
        CanvasView::new(canvas(), layout::FREE, cx).with_kinds(
            Kinds::new().with(
                model::LINK,
                kind::link().edit(
                    kind::Field::new(
                        |node| node.url.clone().unwrap_or_default(),
                        |node, url| node.url = Some(url),
                    )
                    .with_control(|_| kind::Control::External),
                ),
            ),
        )
    });
    let view = window.root(cx).unwrap();
    let requested = Rc::new(Cell::new(0));
    let count = requested.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &CanvasEvent, _| {
            if let CanvasEvent::EditRequested { id } = event {
                assert_eq!(id, "link");
                count.set(count.get() + 1);
            }
        })
    });
    window
        .update(cx, |view, window, cx| {
            view.update_editor(cx, |editor| editor.select(Some("link".into())));
            view.edit_selected(window, cx);
            assert_eq!(view.editor().canvas(), &canvas());
            assert!(!view.editor().can_undo());
        })
        .unwrap();
    assert_eq!(requested.get(), 1);
}
