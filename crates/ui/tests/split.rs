use gpui::{
    Context, Entity, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, Window,
    div, point, prelude::*, px, size,
};
use theme::{Appearance, Theme};
use ui::split::Split;

struct Host {
    ratios: [f32; 2],
    vertical: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .children((0..2).map(|i| {
                let make = if self.vertical {
                    Split::vertical
                } else {
                    Split::horizontal
                };
                make(
                    ("split", i),
                    div()
                        .size_full()
                        .debug_selector(move || format!("first-{i}")),
                    div()
                        .size_full()
                        .debug_selector(move || format!("second-{i}")),
                )
                .ratio(self.ratios[i])
                .min(px(if self.vertical { 10.0 } else { 50.0 }))
                .h(px(100.0))
                .on_resize(cx.listener(move |host, ratio, _, cx| {
                    host.ratios[i] = *ratio;
                    cx.notify();
                }))
            }))
    }
}

fn host(cx: &mut TestAppContext) -> (Entity<Host>, VisualTestContext) {
    cx.update(|cx| Theme::install(Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Host {
        ratios: [0.5, 0.25],
        vertical: false,
    });
    let host = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(409.0), px(200.0)));
    visual.run_until_parked();
    (host, visual)
}

#[gpui::test]
fn ratios_exclude_the_divider_and_minimums_constrain_layout(cx: &mut TestAppContext) {
    let (host, mut cx) = host(cx);
    assert_eq!(cx.debug_bounds("first-0").unwrap().size.width, px(200.0));
    assert_eq!(cx.debug_bounds("first-1").unwrap().size.width, px(100.0));
    cx.update(|_, cx| {
        host.update(cx, |host, cx| {
            host.ratios[0] = 0.0;
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("first-0").unwrap().size.width, px(50.0));
    assert_eq!(cx.debug_bounds("second-0").unwrap().size.width, px(350.0));
}

#[gpui::test]
fn a_drag_updates_only_its_split_and_stops_at_the_minimum(cx: &mut TestAppContext) {
    let (host, mut cx) = host(cx);
    let start = point(px(204.5), px(50.0));
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    for x in [210.0, 300.0, 408.0] {
        cx.simulate_mouse_move(
            point(px(x), px(50.0)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(
        point(px(408.0), px(50.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    let ratios = cx.update(|_, cx| host.read(cx).ratios);
    assert_eq!(ratios, [0.875, 0.25]);
    assert_eq!(cx.debug_bounds("second-0").unwrap().size.width, px(50.0));
}

#[gpui::test]
fn vertical_splits_resize_along_the_y_axis(cx: &mut TestAppContext) {
    let (host, mut cx) = host(cx);
    cx.update(|_, cx| {
        host.update(cx, |host, cx| {
            host.vertical = true;
            cx.notify();
        })
    });
    cx.run_until_parked();
    let first = cx.debug_bounds("first-0").unwrap();
    assert!((first.size.height - px(45.5)).abs() < px(1.0), "{first:?}");
    let start = point(px(100.0), first.bottom() + px(4.5));
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    for y in [55.0, 80.0] {
        cx.simulate_mouse_move(
            point(px(100.0), px(y)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(
        point(px(100.0), px(80.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let ratios = cx.update(|_, cx| host.read(cx).ratios);
    assert!(ratios[0] > 0.75);
    assert_eq!(ratios[1], 0.25);
}
