//! A task block's checkbox as a control, under gpui's own harness — where the
//! box landed is something only a real layout knows.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    App, Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size,
};
use markdown::{BlockLayouts, Doc, Editing, OnToggle, Toggle, parse, render_with};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 400.0;
const SOURCE: &str = "- [ ] open\n- [x] done";

/// What the page asks the box to be.
#[derive(Clone, Copy)]
enum Mode {
    /// No toggle at all — a box that paints and takes nothing.
    Marker,
    /// A control whose press the caller resolves against `checkbox_bounds`.
    HitTested,
    /// A control the renderer listens to.
    Handled,
}

struct Page {
    doc: Doc,
    layouts: BlockLayouts,
    /// Every block the toggle was called with, in order.
    toggled: Rc<RefCell<Vec<usize>>>,
    mode: Mode,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = match self.mode {
            Mode::Marker => None,
            Mode::HitTested => Some(Toggle::HitTested),
            Mode::Handled => {
                let toggled = self.toggled.clone();
                Some(Toggle::Handled(Rc::new(
                    move |ix: usize, _: &mut Window, _: &mut App| toggled.borrow_mut().push(ix),
                ) as OnToggle))
            }
        };
        div().w(px(WIDTH)).child(render_with(
            &self.doc,
            Editing {
                layouts: Some(&self.layouts),
                toggle,
                ..Editing::default()
            },
            window,
            cx,
        ))
    }
}

fn open(mode: Mode, cx: &mut TestAppContext) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page {
        doc: parse(SOURCE),
        layouts: BlockLayouts::default(),
        toggled: Rc::new(RefCell::new(Vec::new())),
        mode,
    });
    let page = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    visual.run_until_parked();
    (page, visual)
}

fn toggled(page: &gpui::Entity<Page>, cx: &mut VisualTestContext) -> Vec<usize> {
    cx.update(|_, cx| page.read(cx).toggled.borrow().clone())
}

fn box_of(
    page: &gpui::Entity<Page>,
    ix: usize,
    cx: &mut VisualTestContext,
) -> gpui::Bounds<gpui::Pixels> {
    cx.update(|_, cx| page.read(cx).layouts.checkbox_bounds(ix))
        .unwrap_or_else(|| panic!("block {ix} recorded no checkbox"))
}

#[gpui::test]
fn every_task_records_its_box(cx: &mut TestAppContext) {
    let (page, mut cx) = open(Mode::Marker, cx);

    let open_box = box_of(&page, 0, &mut cx);
    let done_box = box_of(&page, 1, &mut cx);

    assert!(open_box.size.width > px(0.0), "the box painted");
    assert_eq!(
        open_box.size, done_box.size,
        "a checked box is the same box"
    );
    assert!(
        done_box.origin.y > open_box.origin.y,
        "the second row sits under the first"
    );
    // The row is wider than its marker column, and the column wider than the
    // box: a press beside the box has somewhere to land.
    let row = cx
        .update(|_, cx| page.read(cx).layouts.block_bounds(0))
        .expect("the block painted");
    assert!(
        open_box.size.width < row.size.width,
        "the box is not the row"
    );
}

#[gpui::test]
fn a_press_in_the_box_toggles(cx: &mut TestAppContext) {
    let (page, mut cx) = open(Mode::Handled, cx);

    let done = box_of(&page, 1, &mut cx);
    cx.simulate_click(done.center(), gpui::Modifiers::default());

    assert_eq!(toggled(&page, &mut cx), vec![1], "the checked box answered");

    let open_ = box_of(&page, 0, &mut cx);
    cx.simulate_click(open_.center(), gpui::Modifiers::default());

    assert_eq!(
        toggled(&page, &mut cx),
        vec![1, 0],
        "and so did the unchecked one"
    );
}

#[gpui::test]
fn a_press_beside_the_box_does_not(cx: &mut TestAppContext) {
    let (page, mut cx) = open(Mode::Handled, cx);

    let box_ = box_of(&page, 0, &mut cx);
    // The gutter the marker column leaves to the right of the box, which is
    // where a caret goes.
    let beside = gpui::point(box_.origin.x + box_.size.width + px(3.0), box_.center().y);
    cx.simulate_click(beside, gpui::Modifiers::default());

    assert!(
        toggled(&page, &mut cx).is_empty(),
        "the gutter is not the box"
    );
}

#[gpui::test]
fn a_hit_tested_box_takes_no_press(cx: &mut TestAppContext) {
    let (page, mut cx) = open(Mode::HitTested, cx);

    let box_ = box_of(&page, 0, &mut cx);
    cx.simulate_click(box_.center(), gpui::Modifiers::default());

    assert!(
        toggled(&page, &mut cx).is_empty(),
        "the press is the caller's to resolve"
    );
}

#[gpui::test]
fn a_read_only_box_is_a_marker(cx: &mut TestAppContext) {
    let (page, mut cx) = open(Mode::Marker, cx);

    let box_ = box_of(&page, 0, &mut cx);
    cx.simulate_click(box_.center(), gpui::Modifiers::default());

    assert!(
        toggled(&page, &mut cx).is_empty(),
        "nothing was handed a toggle"
    );
}
