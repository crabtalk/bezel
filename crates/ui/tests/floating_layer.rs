//! What a layer over a page takes, and what it lets past — under gpui's own
//! harness, because hit testing is the whole subject.
//!
//! gpui hands a press to every hitbox containing the pointer and a wheel to
//! every one in the hit list at all, so a band drawn over a page passes both
//! through unless it blocks the mouse.

use std::{cell::Cell, rc::Rc};

use gpui::{
    Modifiers, ScrollDelta, ScrollHandle, ScrollWheelEvent, TestAppContext, VisualTestContext, div,
    point, prelude::*, px, size,
};
use ui::{
    floating,
    scroll::{self, Axes, ScrollbarState},
};

const WIDTH: f32 = 400.0;
const HEIGHT: f32 = 300.0;
/// The page's content, long enough that it has somewhere to scroll.
const CONTENT: f32 = 2000.0;
/// The band's box, held at the top so a press at a known point lands in it.
const BAND: f32 = 60.0;

struct Page {
    scroll: ScrollHandle,
    bar: ScrollbarState,
    /// Presses that reached the page under the band.
    hits: Rc<Cell<usize>>,
    /// Whether the band is drawn as a layer or as a plain absolute box.
    layered: bool,
}

impl gpui::Render for Page {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let hits = self.hits.clone();
        let band = match self.layered {
            true => floating::layer("band"),
            false => div().id("band").absolute(),
        };
        div()
            .size_full()
            .relative()
            .child(
                scroll::pane("page", Axes::Vertical)
                    .size_full()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .id("page-content")
                            .w_full()
                            .h(px(CONTENT))
                            .on_click(move |_, _, _| hits.set(hits.get() + 1)),
                    ),
            )
            .child(scroll::scrollbar("page-bar", &self.scroll, &self.bar))
            .child(band.top_0().left_0().right_0().h(px(BAND)))
    }
}

fn open(layered: bool, cx: &mut TestAppContext) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| Page {
        scroll: ScrollHandle::new(),
        bar: ScrollbarState::new(motion::Painter::of(cx)),
        hits: Rc::new(Cell::new(0)),
        layered,
    });
    let view = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    visual.run_until_parked();
    (view, visual)
}

fn wheel(at: gpui::Point<gpui::Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        modifiers: Default::default(),
        touch_phase: Default::default(),
    });
    cx.run_until_parked();
}

fn state(view: &gpui::Entity<Page>, cx: &mut VisualTestContext) -> (usize, f32) {
    cx.update(|_, cx| {
        let view = view.read(cx);
        (view.hits.get(), f32::from(view.scroll.offset().y).abs())
    })
}

/// In the middle of the band, clear of the scrollbar at the right edge.
fn in_band() -> gpui::Point<gpui::Pixels> {
    point(px(WIDTH / 2.0), px(BAND / 2.0))
}

#[gpui::test]
fn a_layer_takes_the_press_and_the_wheel_over_it(cx: &mut TestAppContext) {
    let (view, mut cx) = open(true, cx);

    cx.simulate_click(in_band(), Modifiers::default());
    cx.run_until_parked();
    wheel(in_band(), &mut cx);

    let (hits, travelled) = state(&view, &mut cx);
    assert_eq!(hits, 0, "the page under the band took the press");
    assert_eq!(travelled, 0.0, "the page scrolled under the band");
}

/// The bug the layer exists for, from the other side: a plain absolute box
/// passes both through.
#[gpui::test]
fn a_plain_absolute_box_passes_both_through(cx: &mut TestAppContext) {
    let (view, mut cx) = open(false, cx);

    cx.simulate_click(in_band(), Modifiers::default());
    cx.run_until_parked();
    wheel(in_band(), &mut cx);

    let (hits, travelled) = state(&view, &mut cx);
    assert_eq!(hits, 1);
    assert!(travelled > 0.0);
}

/// A bar is laid over the pane it reports on: the press is the bar's, and the
/// wheel is still the pane's.
#[gpui::test]
fn a_scrollbar_takes_the_press_but_not_the_wheel(cx: &mut TestAppContext) {
    let (view, mut cx) = open(true, cx);
    let on_bar = cx
        .debug_bounds("page-bar-track")
        .expect("the bar is showing")
        .center();

    cx.simulate_click(on_bar, Modifiers::default());
    cx.run_until_parked();
    let (hits, _) = state(&view, &mut cx);
    assert_eq!(hits, 0, "the content under the bar took the press");

    wheel(on_bar, &mut cx);
    let (_, travelled) = state(&view, &mut cx);
    assert!(
        travelled > 0.0,
        "a wheel over the bar should scroll the pane"
    );
}
