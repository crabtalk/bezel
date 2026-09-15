//! The canvas inside a host that does `ui::focus` traversal, the way an app
//! root does.

use canvas::{Canvas, CanvasView};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, Modifiers,
    MouseButton, ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext,
    Window, div, point, px, size,
};

struct Host {
    canvas: Entity<CanvasView>,
    focus: FocusHandle,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        ui::focus::traversal(div().size_full().track_focus(&self.focus)).child(self.canvas.clone())
    }
}

fn open(json: &str, cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        ui::focus::init(cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let canvas = Canvas::parse(json).unwrap();
    let window = cx.add_window(|_, cx| Host {
        canvas: cx.new(|cx| CanvasView::new(canvas, cx)),
        focus: cx.focus_handle(),
    });
    let host = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(800.0), px(600.0)));
    visual.run_until_parked();
    // A test window has no frame clock: draw until the resize, the centring and
    // the measured heights have all painted.
    for _ in 0..3 {
        visual.update(|window, cx| window.draw(cx).clear(cx));
    }
    let view = visual.update(|_, cx| host.read(cx).canvas.clone());
    (view, visual)
}

#[gpui::test]
fn a_click_focuses_and_tab_adds_a_node(cx: &mut TestAppContext) {
    let (view, mut cx) = open("{}", cx);
    cx.simulate_mouse_down(
        point(px(100.0), px(100.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(100.0), px(100.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| view.read(cx).canvas().nodes.len()), 1);
}

#[gpui::test]
fn dragging_the_background_pans(cx: &mut TestAppContext) {
    let (view, mut cx) = open("{}", cx);
    let before = cx.update(|_, cx| view.read(cx).pan());
    cx.simulate_mouse_down(
        point(px(100.0), px(100.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(px(150.0), px(130.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(150.0), px(130.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let after = cx.update(|_, cx| view.read(cx).pan());
    assert_eq!((after.x - before.x, after.y - before.y), (50.0, 30.0));
}
