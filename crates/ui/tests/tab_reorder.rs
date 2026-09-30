use gpui::{
    Context, Entity, Modifiers, MouseButton, Pixels, Point, Render, TestAppContext,
    VisualTestContext, Window, div, point, prelude::*, px, size,
};
use theme::{Appearance, Theme};
use ui::tabs::{self, Reorder, Strip};

struct Host {
    strip: Strip<&'static str>,
    reorder: Reorder<&'static str>,
    moves: Vec<tabs::Move>,
    clicks: usize,
    outside: Vec<&'static str>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let children = self.strip.tabs().iter().map(|&id| {
            let tab = tabs::tab(theme, id, tabs::Label::new(id), tabs::State::Resting)
                .debug_selector(move || format!("tab-{id}"))
                .w(px(match id {
                    "b" => 140.0,
                    _ => 80.0,
                }))
                .on_click(cx.listener(|host, _, _, _| host.clicks += 1))
                .child(
                    tabs::close(theme, id, tabs::Close::Always)
                        .debug_selector(move || format!("tab-close-{id}"))
                        .on_click(cx.listener(move |host, _, _, cx| {
                            cx.stop_propagation();
                            host.strip.close(&id);
                            cx.notify();
                        })),
                );
            (id, tab)
        });
        div().size_full().child(
            self.reorder
                .bar("strip", &self.strip, children)
                .on_reorder(cx.listener(|host, movement: &tabs::Move, _, cx| {
                    host.strip.reorder(movement.from, movement.to);
                    host.moves.push(*movement);
                    cx.notify();
                }))
                .on_drop_outside(cx.listener(
                    |host, drop: &tabs::OutsideDrop<&'static str>, _, _| {
                        host.outside.push(drop.id);
                    },
                )),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<Host>, VisualTestContext) {
    cx.update(|cx| Theme::install(Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| Host {
        strip: ["a", "b", "c"].into_iter().collect(),
        reorder: Reorder::new(motion::Painter::of(cx)),
        moves: Vec::new(),
        clicks: 0,
        outside: Vec::new(),
    });
    let host = window.root(cx).unwrap();
    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(500.), px(200.)));
    cx.run_until_parked();
    (host, cx)
}

fn down(at: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(at, None, Modifiers::default());
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
}

fn travel(at: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

fn up(at: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn reorder_is_live_and_the_carried_tab_stays_under_the_pointer(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    let start = cx.debug_bounds("tab-a").unwrap().origin + point(px(10.), px(10.));
    down(start, &mut cx);
    let at = start + point(px(160.), px(0.));
    travel(at, &mut cx);
    cx.update(|_, cx| {
        assert_eq!(host.read(cx).strip.tabs(), &["a", "b", "c"]);
        assert_eq!(host.read(cx).strip.active(), Some(&"a"));
        assert!(host.read(cx).moves.is_empty());
        assert!(
            !cx.has_active_drag(),
            "the strip must not create a gpui ghost"
        );
    });
    assert_eq!(cx.debug_bounds("tab-a").unwrap().left(), at.x - px(10.));
    up(at, &mut cx);
    cx.update(|_, cx| {
        assert_eq!(host.read(cx).clicks, 0);
        assert_eq!(host.read(cx).strip.tabs(), &["b", "a", "c"]);
        assert_eq!(host.read(cx).moves, [tabs::Move { from: 0, to: 1 }]);
    });
}

#[gpui::test]
fn several_samples_before_paint_use_the_latest_order(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    let at = point(px(10.), px(10.));
    down(at, &mut cx);
    for x in [170., 290.] {
        cx.simulate_mouse_move(point(px(x), at.y), MouseButton::Left, Modifiers::default());
    }
    cx.update(|_, cx| {
        assert_eq!(host.read(cx).strip.tabs(), &["a", "b", "c"]);
        assert!(host.read(cx).moves.is_empty());
    });
    up(point(px(290.), at.y), &mut cx);
    cx.update(|_, cx| {
        assert_eq!(host.read(cx).strip.tabs(), &["b", "c", "a"]);
        assert_eq!(host.read(cx).moves, [tabs::Move { from: 0, to: 2 }]);
    });
}

#[gpui::test]
fn a_click_and_subthreshold_motion_still_activate(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    let at = point(px(10.), px(10.));
    down(at, &mut cx);
    travel(at + point(px(1.), px(0.)), &mut cx);
    up(at, &mut cx);
    cx.update(|_, cx| {
        assert_eq!(host.read(cx).clicks, 1);
        assert!(host.read(cx).moves.is_empty());
    });
}

#[gpui::test]
fn outside_release_reports_the_tab_and_ends_the_gesture(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    down(point(px(10.), px(10.)), &mut cx);
    let at = point(px(170.), px(100.));
    travel(at, &mut cx);
    up(at, &mut cx);
    cx.simulate_mouse_move(point(px(280.), px(10.)), None, Modifiers::default());
    cx.update(|_, cx| {
        assert_eq!(host.read(cx).outside, ["a"]);
        assert_eq!(host.read(cx).strip.tabs(), &["a", "b", "c"]);
        assert_eq!(host.read(cx).clicks, 0);
    });
}

#[gpui::test]
fn close_button_does_not_start_a_tab_drag(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    let at = cx.debug_bounds("tab-close-a").unwrap().center();
    down(at, &mut cx);
    travel(at + point(px(200.), px(0.)), &mut cx);
    up(at + point(px(200.), px(0.)), &mut cx);
    cx.update(|_, cx| assert!(host.read(cx).moves.is_empty()));
    cx.simulate_click(at, Modifiers::default());
    cx.update(|_, cx| assert_eq!(host.read(cx).strip.tabs(), &["b", "c"]));
}

#[gpui::test]
fn removing_the_carried_tab_cancels_the_gesture(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    down(point(px(10.), px(10.)), &mut cx);
    travel(point(px(40.), px(10.)), &mut cx);
    host.update(&mut cx, |host, cx| {
        host.strip.close(&"a");
        cx.notify();
    });
    cx.run_until_parked();
    travel(point(px(300.), px(10.)), &mut cx);
    up(point(px(300.), px(10.)), &mut cx);
    cx.update(|_, cx| {
        assert_eq!(host.read(cx).strip.tabs(), &["b", "c"]);
        assert!(host.read(cx).moves.is_empty());
    });
}

#[gpui::test]
fn neighbours_slide_and_the_released_tab_settles(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    let before = cx.debug_bounds("tab-b").unwrap().left();
    down(point(px(10.), px(10.)), &mut cx);
    travel(point(px(170.), px(10.)), &mut cx);
    assert_eq!(cx.debug_bounds("tab-b").unwrap().left(), before);
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("tab-b").unwrap().left(), px(0.));
    assert_eq!(cx.debug_bounds("tab-a").unwrap().left(), px(160.));
    up(point(px(170.), px(10.)), &mut cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("tab-a").unwrap().left(), px(142.));
    assert_eq!(cx.update(|_, cx| host.read(cx).clicks), 0);
}

#[gpui::test]
fn reversing_mid_slide_preserves_the_grab_point(cx: &mut TestAppContext) {
    let (host, mut cx) = open(cx);
    down(point(px(10.), px(10.)), &mut cx);
    travel(point(px(290.), px(10.)), &mut cx);
    travel(point(px(5.), px(10.)), &mut cx);
    cx.update(|_, cx| assert_eq!(host.read(cx).strip.tabs(), &["a", "b", "c"]));
    assert_eq!(cx.debug_bounds("tab-a").unwrap().left(), px(-5.));
    up(point(px(5.), px(10.)), &mut cx);
}
