use gpui::{
    Axis, Context, Modifiers, MouseButton, Render, ScrollHandle, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px, size,
};
use ui::scroll::{self as scrollbars, Visibility as Scrollbars};

struct Host {
    handle: ScrollHandle,
    axis: Axis,
    presses: usize,
    visibility: Option<Scrollbars>,
    end_inset: gpui::Pixels,
    margin: Option<gpui::Pixels>,
    narrow: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = div()
            .id("content")
            .debug_selector(|| "scroll-content".into())
            .flex_none()
            .w(px(600.))
            .h(px(600.))
            .on_click(cx.listener(|this, _, _, _| this.presses += 1));
        let mut overlay =
            scrollbars::Overlay::new("test-bar", &self.handle, self.axis).end_inset(self.end_inset);
        if let Some(margin) = self.margin {
            overlay = overlay.margin(margin);
        }
        if let Some(visibility) = self.visibility {
            overlay = overlay.visibility(visibility);
        }
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("viewport")
                    .size_full()
                    .when(self.narrow, |el| el.w(px(120.)))
                    .overflow_scroll()
                    .track_scroll(&self.handle)
                    .child(content),
            )
            .child(overlay)
    }
}

fn open(axis: Axis, cx: &mut TestAppContext) -> (gpui::Entity<Host>, VisualTestContext) {
    cx.update(|cx| scrollbars::set_visibility(Scrollbars::Always, cx));
    let window = cx.add_window(|_, _| Host {
        handle: ScrollHandle::new(),
        axis,
        presses: 0,
        visibility: None,
        end_inset: px(0.),
        margin: None,
        narrow: false,
    });
    let host = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(200.), px(200.)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    (host, cx)
}

#[gpui::test]
fn overlay_preserves_viewport_and_content_clicks(cx: &mut TestAppContext) {
    let (host, mut cx) = open(Axis::Vertical, cx);
    let bounds = cx.update(|_, cx| host.read(cx).handle.bounds());
    assert_eq!(bounds.size, size(px(200.), px(200.)));
    cx.simulate_click(point(px(40.), px(40.)), Modifiers::default());
    assert_eq!(cx.update(|_, cx| host.read(cx).presses), 1);
}

#[gpui::test]
fn horizontal_thumb_drags_only_the_horizontal_axis(cx: &mut TestAppContext) {
    let (host, mut cx) = open(Axis::Horizontal, cx);
    cx.simulate_mouse_move(point(px(20.), px(196.)), None, Modifiers::default());
    cx.simulate_mouse_down(
        point(px(20.), px(196.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(px(30.), px(196.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(px(110.), px(196.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(110.), px(196.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let offset = cx.update(|_, cx| host.read(cx).handle.offset());
    assert!(
        offset.x < px(-100.),
        "thumb should move the content: {offset:?}"
    );
    assert_eq!(offset.y, px(0.));
}

#[gpui::test]
fn vertical_thumb_drags_and_never_mode_removes_it(cx: &mut TestAppContext) {
    let (host, mut cx) = open(Axis::Vertical, cx);
    for mode in [Scrollbars::Always, Scrollbars::Never] {
        cx.update(|window, cx| {
            scrollbars::set_visibility(mode, cx);
            host.read(cx).handle.set_offset(point(px(0.), px(0.)));
            window.refresh();
        });
        cx.run_until_parked();
        cx.simulate_mouse_move(point(px(196.), px(20.)), None, Modifiers::default());
        cx.simulate_mouse_down(
            point(px(196.), px(20.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(px(196.), px(30.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(px(196.), px(110.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(px(196.), px(110.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        let offset = cx.update(|_, cx| host.read(cx).handle.offset());
        if mode == Scrollbars::Always {
            assert!(offset.y < px(-100.));
        } else {
            assert_eq!(offset.y, px(0.));
        }
        assert_eq!(offset.x, px(0.));
    }
}

struct Framed;
impl Render for Framed {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().flex_col().child(
            scrollbars::Viewport::new(
                "frame",
                div()
                    .id("framed-content")
                    .debug_selector(|| "framed-content".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().h(px(600.)).w_full()),
                Axis::Vertical,
            )
            .fill(),
        )
    }
}

#[gpui::test]
fn frame_keeps_distinct_handle_and_scrollbar_state(cx: &mut TestAppContext) {
    cx.update(|cx| scrollbars::set_visibility(Scrollbars::Always, cx));
    let window = cx.add_window(|_, _| Framed);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(200.), px(200.)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.simulate_mouse_move(point(px(196.), px(20.)), None, Modifiers::default());
    assert_eq!(
        cx.debug_bounds("framed-content").unwrap().size,
        size(px(200.), px(200.))
    );
}

#[gpui::test]
fn horizontal_scrollbar_reserves_no_layout_space(cx: &mut TestAppContext) {
    let (host, mut cx) = open(Axis::Horizontal, cx);
    cx.update(|window, cx| {
        scrollbars::set_visibility(Scrollbars::Never, cx);
        window.refresh();
    });
    cx.run_until_parked();
    let viewport = cx.update(|_, cx| host.read(cx).handle.bounds());
    let content = cx.debug_bounds("scroll-content").unwrap();
    let overflow = cx.update(|_, cx| host.read(cx).handle.max_offset());
    for mode in [Scrollbars::Always, Scrollbars::Scrolling] {
        cx.update(|window, cx| {
            scrollbars::set_visibility(mode, cx);
            window.refresh();
        });
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| host.read(cx).handle.bounds()), viewport);
        assert_eq!(cx.debug_bounds("scroll-content").unwrap(), content);
        assert_eq!(
            cx.update(|_, cx| host.read(cx).handle.max_offset()),
            overflow
        );
    }
}

#[gpui::test]
fn transient_horizontal_bar_fades_and_returns_when_scrolled(cx: &mut TestAppContext) {
    let (host, mut cx) = open(Axis::Horizontal, cx);
    cx.update(|_, cx| scrollbars::set_visibility(Scrollbars::Scrolling, cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("test-bar-thumb")
            .is_some_and(|bounds| bounds.size.width > px(0.))
    );
    cx.executor()
        .advance_clock(scrollbars::TRANSIENT_IDLE + std::time::Duration::from_millis(50));
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("test-bar-thumb")
            .is_none_or(|bounds| bounds.size.width == px(0.))
    );
    cx.update(|window, cx| {
        host.read(cx).handle.set_offset(point(px(-40.), px(0.)));
        window.refresh();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("test-bar-thumb")
            .is_some_and(|bounds| bounds.size.width > px(0.))
    );
}

#[gpui::test]
fn pane_visibility_overrides_the_global_default(cx: &mut TestAppContext) {
    let (host, mut cx) = open(Axis::Horizontal, cx);
    cx.update(|window, cx| {
        host.update(cx, |host, cx| {
            host.visibility = Some(Scrollbars::Never);
            cx.notify();
        });
        window.refresh();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("test-bar-thumb").is_none());
    cx.update(|window, cx| {
        scrollbars::set_visibility(Scrollbars::Never, cx);
        host.update(cx, |host, cx| {
            host.visibility = Some(Scrollbars::Always);
            cx.notify();
        });
        window.refresh();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("test-bar-thumb")
            .is_some_and(|bounds| bounds.size.width > px(0.))
    );
}

fn check_inset(axis: Axis, cx: &mut TestAppContext) {
    let (host, mut cx) = open(axis, cx);
    let track = cx.debug_bounds("test-bar-track").unwrap();
    let thumb = cx.debug_bounds("test-bar-thumb").unwrap();
    let (origin, dimensions) = match axis {
        Axis::Horizontal => (point(px(4.), px(190.)), size(px(192.), px(10.))),
        Axis::Vertical => (point(px(190.), px(4.)), size(px(10.), px(192.))),
    };
    assert_eq!(track.origin, origin);
    assert_eq!(track.size, dimensions);
    match axis {
        Axis::Horizontal => {
            assert_eq!(thumb.left(), px(4.));
            assert_eq!(thumb.size.width, px(64.));
        }
        Axis::Vertical => {
            assert_eq!(thumb.top(), px(4.));
            assert_eq!(thumb.size.height, px(64.));
        }
    }
    let start = thumb.center();
    let end = match axis {
        Axis::Horizontal => point(px(220.), start.y),
        Axis::Vertical => point(start.x, px(220.)),
    };
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    let middle = match axis {
        Axis::Horizontal => point(start.x + px(10.), start.y),
        Axis::Vertical => point(start.x, start.y + px(10.)),
    };
    cx.simulate_mouse_move(middle, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    let offset = cx.update(|_, cx| host.read(cx).handle.offset());
    let thumb = cx.debug_bounds("test-bar-thumb").unwrap();
    match axis {
        Axis::Horizontal => {
            assert_eq!(offset.x, px(-400.));
            assert_eq!(thumb.right(), px(196.));
        }
        Axis::Vertical => {
            assert_eq!(offset.y, px(-400.));
            assert_eq!(thumb.bottom(), px(196.));
        }
    }
    cx.simulate_mouse_move(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::default());
    assert_eq!(
        cx.update(|_, cx| host.read(cx).handle.offset()),
        point(px(0.), px(0.))
    );
}

#[gpui::test]
fn horizontal_inset_preserves_drag_endpoints(cx: &mut TestAppContext) {
    check_inset(Axis::Horizontal, cx);
}

#[gpui::test]
fn vertical_inset_preserves_drag_endpoints(cx: &mut TestAppContext) {
    check_inset(Axis::Vertical, cx);
}

#[gpui::test]
fn detached_track_clears_footer_and_reaches_scroll_end(cx: &mut TestAppContext) {
    let (host, mut cx) = open(Axis::Vertical, cx);
    for mode in [Scrollbars::Always, Scrollbars::Scrolling] {
        cx.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.end_inset = px(60.);
                host.narrow = true;
                host.visibility = Some(mode);
                host.handle.set_offset(point(px(0.), px(0.)));
                cx.notify();
            });
            window.refresh();
        });
        cx.run_until_parked();
        let track = cx.debug_bounds("test-bar-track").unwrap();
        assert_eq!(track.right(), px(200.));
        assert_eq!(track.bottom(), px(136.));
        assert_eq!(
            cx.update(|_, cx| host.read(cx).handle.bounds().size),
            size(px(120.), px(200.))
        );
        let start = cx.debug_bounds("test-bar-thumb").unwrap().center();
        cx.simulate_mouse_move(start, None, Modifiers::default());
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(start.x, start.y + px(10.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        let end = point(start.x, px(170.));
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| host.read(cx).handle.offset().y),
            px(-400.)
        );
        assert_eq!(
            cx.debug_bounds("test-bar-thumb").unwrap().bottom(),
            px(136.)
        );
    }
}

/// The thumb stands `margin` off the pane's edge, 2px unless set, and the
/// track under it keeps its width.
#[gpui::test]
fn margin_sets_the_gap_between_the_thumb_and_the_edge(cx: &mut TestAppContext) {
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let (host, mut cx) = open(axis, cx);
        for margin in [None, Some(px(0.)), Some(px(12.))] {
            cx.update(|window, cx| {
                host.update(cx, |host, cx| {
                    host.margin = margin;
                    cx.notify();
                });
                window.refresh();
            });
            cx.run_until_parked();
            let thumb = cx.debug_bounds("test-bar-thumb").unwrap();
            let track = cx.debug_bounds("test-bar-track").unwrap();
            let pane = cx.update(|_, cx| host.read(cx).handle.bounds());
            let (gap, thickness, width) = match axis {
                Axis::Vertical => (
                    pane.right() - thumb.right(),
                    thumb.size.width,
                    track.size.width,
                ),
                Axis::Horizontal => (
                    pane.bottom() - thumb.bottom(),
                    thumb.size.height,
                    track.size.height,
                ),
            };
            assert_eq!(gap, margin.unwrap_or(px(2.)), "{axis:?} {margin:?}");
            assert_eq!(thickness, px(4.), "{axis:?} {margin:?}");
            assert_eq!(width, px(10.), "{axis:?} {margin:?}");
        }
    }
}

fn thumb_up(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("test-bar-thumb")
        .is_some_and(|bounds| bounds.size.width > px(0.) && bounds.size.height > px(0.))
}

/// Past the fade, with a frame drawn at the end of it.
fn idle(cx: &mut VisualTestContext) {
    cx.executor()
        .advance_clock(scrollbars::TRANSIENT_IDLE + std::time::Duration::from_millis(50));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

/// gpui keeps the pointer's last position when it leaves the window, so the
/// track under it would read as hovered again on the next frame.
#[gpui::test]
fn a_bar_fades_after_the_pointer_leaves_the_window_across_its_track(cx: &mut TestAppContext) {
    for (axis, on_track) in [
        (Axis::Vertical, point(px(196.), px(100.))),
        (Axis::Horizontal, point(px(100.), px(196.))),
    ] {
        let (_host, mut cx) = open(axis, cx);
        cx.update(|_, cx| scrollbars::set_visibility(Scrollbars::Scrolling, cx));
        cx.run_until_parked();
        cx.simulate_mouse_move(on_track, None, Modifiers::default());
        idle(&mut cx);
        assert!(
            thumb_up(&mut cx),
            "{axis:?}: hovering the track holds the bar up"
        );

        cx.simulate_event(gpui::MouseExitEvent {
            position: on_track,
            pressed_button: None,
            modifiers: Modifiers::default(),
        });
        cx.run_until_parked();
        idle(&mut cx);
        assert!(
            !thumb_up(&mut cx),
            "{axis:?}: the bar stayed up after the pointer left"
        );

        cx.simulate_mouse_move(on_track, None, Modifiers::default());
        idle(&mut cx);
        assert!(
            thumb_up(&mut cx),
            "{axis:?}: coming back over the track raises it again"
        );
    }
}
