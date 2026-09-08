//! A wheel over a nested pane, under gpui's own harness — a real window, real
//! hit testing, real dispatch.
//!
//! gpui's scroll listener neither stops propagation nor asks whether an
//! ancestor scrolls too, so every scrolling pane under the pointer moves at
//! once. Nothing pure can show that: it takes two live panes and an event that
//! reaches both.

use gpui::{
    Axis, ScrollDelta, ScrollHandle, ScrollWheelEvent, TestAppContext, VisualTestContext, div,
    point, prelude::*, px, size,
};
use ui::scroll::{self, ClaimState};

const WIDTH: f32 = 400.0;
const HEIGHT: f32 = 300.0;
/// The inner pane's box, and its content — 400 of content in 100 of box, so it
/// has 300 to travel before it runs out.
const INNER_BOX: f32 = 100.0;
const INNER_CONTENT: f32 = 400.0;
/// Below the inner pane, so the outer has somewhere to go as well.
const SPACER: f32 = 600.0;
/// One notch, in pixels. Negative is downwards — gpui's offsets go negative as
/// you travel.
const NOTCH: f32 = -40.0;

struct Nested {
    outer: ScrollHandle,
    inner: ScrollHandle,
    /// Whether the inner pane claims the wheel it can act on. Off is the bug.
    claims: bool,
    claim: ClaimState,
}

impl gpui::Render for Nested {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let inner = div()
            .id("inner")
            .w_full()
            .h(px(INNER_BOX))
            .overflow_y_scroll()
            .track_scroll(&self.inner)
            .child(div().w_full().h(px(INNER_CONTENT)));
        let inner = if self.claims {
            scroll::claim_wheel(inner, &self.inner, Axis::Vertical, &self.claim)
        } else {
            inner
        };
        div()
            .id("outer")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.outer)
            .child(inner)
            .child(div().w_full().h(px(SPACER)))
    }
}

fn open(claims: bool, cx: &mut TestAppContext) -> (gpui::Entity<Nested>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Nested {
        outer: ScrollHandle::new(),
        inner: ScrollHandle::new(),
        claims,
        claim: ClaimState::new(),
    });
    let view = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    visual.run_until_parked();
    (view, visual)
}

/// One notch of wheel with the pointer over the inner pane.
fn wheel_over_inner(cx: &mut VisualTestContext) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(WIDTH / 2.0), px(INNER_BOX / 2.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(NOTCH))),
        modifiers: Default::default(),
        touch_phase: Default::default(),
    });
    cx.run_until_parked();
}

/// How far each pane has travelled, as a positive distance.
fn travelled(view: &gpui::Entity<Nested>, cx: &mut VisualTestContext) -> (f32, f32) {
    cx.update(|_, cx| {
        let view = view.read(cx);
        (
            f32::from(view.outer.offset().y).abs(),
            f32::from(view.inner.offset().y).abs(),
        )
    })
}

#[gpui::test]
fn a_wheel_over_a_nested_pane_moves_only_that_pane(cx: &mut TestAppContext) {
    let (view, mut cx) = open(true, cx);

    wheel_over_inner(&mut cx);
    let (outer, inner) = travelled(&view, &mut cx);
    assert!(inner > 0.0, "the pane under the pointer takes the wheel");
    assert_eq!(outer, 0.0, "and the page behind it stays where it was");
}

#[gpui::test]
fn a_nested_pane_at_its_end_passes_the_wheel_on(cx: &mut TestAppContext) {
    let (view, mut cx) = open(true, cx);

    // Run the inner pane out of travel: 300 of overflow, 40 to a notch.
    for _ in 0..(((INNER_CONTENT - INNER_BOX) / -NOTCH) as usize + 1) {
        wheel_over_inner(&mut cx);
    }
    let (outer, inner) = travelled(&view, &mut cx);
    assert_eq!(
        inner,
        INNER_CONTENT - INNER_BOX,
        "the inner pane is at its end"
    );
    assert_eq!(outer, 0.0, "and got there without dragging the page along");

    wheel_over_inner(&mut cx);
    let (outer, _) = travelled(&view, &mut cx);
    assert!(
        outer > 0.0,
        "with nowhere left to go it hands the wheel to the page, rather than \
         stranding it and making the pointer move"
    );
}

#[gpui::test]
fn without_the_claim_both_panes_move_at_once(cx: &mut TestAppContext) {
    let (view, mut cx) = open(false, cx);

    wheel_over_inner(&mut cx);
    let (outer, inner) = travelled(&view, &mut cx);
    assert!(inner > 0.0 && outer > 0.0, "which is the reported bug");
}
