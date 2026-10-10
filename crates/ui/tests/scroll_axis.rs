//! Which axis a gesture moves, under gpui's own dispatch: each test fires a
//! single-axis gesture at a [`ui::scroll::pane`] and asserts it moves only the
//! axes the pane scrolls.

use gpui::{
    AnyElement, Point, ScrollDelta, ScrollHandle, ScrollWheelEvent, SharedString, TestAppContext,
    VisualTestContext, div, point, prelude::*, px, size,
};
use ui::scroll::{self, Axes};

const WIDTH: f32 = 400.0;
const HEIGHT: f32 = 300.0;
/// Bigger than the box on both sides, so there is room to go either way.
const CONTENT: f32 = 2000.0;
/// One gesture, in pixels. Negative is forwards — gpui's offsets go negative.
const SWIPE: f32 = -80.0;

struct Host {
    scroll: ScrollHandle,
    axes: Axes,
}

impl gpui::Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        scroll::pane("pane", self.axes)
            .size_full()
            .track_scroll(&self.scroll)
            .child(div().w(px(CONTENT)).h(px(CONTENT)))
    }
}

/// One gesture at the middle of the pane, and where it left the offset.
fn swipe(axes: Axes, delta: Point<gpui::Pixels>, cx: &mut TestAppContext) -> (f32, f32) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Host {
        scroll: ScrollHandle::new(),
        axes,
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    cx.run_until_parked();
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(WIDTH / 2.0), px(HEIGHT / 2.0)),
        delta: ScrollDelta::Pixels(delta),
        modifiers: Default::default(),
        touch_phase: Default::default(),
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let offset = view.read(cx).scroll.offset();
        (f32::from(offset.x), f32::from(offset.y))
    })
}

fn sideways() -> Point<gpui::Pixels> {
    point(px(SWIPE), px(0.0))
}

fn downwards() -> Point<gpui::Pixels> {
    point(px(0.0), px(SWIPE))
}

#[gpui::test]
fn a_sideways_swipe_leaves_a_vertical_pane_where_it_was(cx: &mut TestAppContext) {
    assert_eq!(swipe(Axes::Vertical, sideways(), cx), (0.0, 0.0));
}

#[gpui::test]
fn a_downward_swipe_leaves_a_horizontal_pane_where_it_was(cx: &mut TestAppContext) {
    assert_eq!(swipe(Axes::Horizontal, downwards(), cx), (0.0, 0.0));
}

#[gpui::test]
fn a_pane_answers_its_own_axis(cx: &mut TestAppContext) {
    assert_eq!(swipe(Axes::Vertical, downwards(), cx), (0.0, SWIPE));
    assert_eq!(swipe(Axes::Horizontal, sideways(), cx), (SWIPE, 0.0));
}

#[gpui::test]
fn a_both_axes_pane_answers_either(cx: &mut TestAppContext) {
    assert_eq!(swipe(Axes::Both, sideways(), cx), (SWIPE, 0.0));
    assert_eq!(swipe(Axes::Both, downwards(), cx), (0.0, SWIPE));
}

// ---------------------------------------------------------------------------
// A board: columns that scroll down, inside a board that scrolls across
// ---------------------------------------------------------------------------

/// One column's width. Four in a 400pt viewport, so the board has somewhere to
/// go sideways.
const COLUMN: f32 = 150.0;
const COLUMNS: usize = 4;

/// The kanban shape: a horizontal pane whose children are vertical panes. The
/// pointer is over both at the same time, and each is the other's ancestor for
/// the axis it does not scroll.
struct BoardHost {
    board: ScrollHandle,
    column: ScrollHandle,
}

impl BoardHost {
    /// The first column — the one under the pointer, and the one measured. The
    /// others only make the board wider than its viewport.
    fn column(&self) -> AnyElement {
        let cards = div().w_full().h(px(CONTENT));
        div()
            .flex_none()
            .w(px(COLUMN))
            .h_full()
            .child(
                scroll::pane("column", Axes::Vertical)
                    .size_full()
                    .track_scroll(&self.column)
                    .child(cards),
            )
            .into_any_element()
    }
}

impl gpui::Render for BoardHost {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().size_full().child(
            scroll::pane("board", Axes::Horizontal)
                .size_full()
                .flex()
                .flex_row()
                .track_scroll(&self.board)
                .child(self.column())
                .children((1..COLUMNS).map(|ix| {
                    div()
                        .flex_none()
                        .w(px(COLUMN))
                        .h_full()
                        .child(SharedString::from(format!("filler {ix}")))
                })),
        )
    }
}

/// One gesture over the first column, and where it left both panes:
/// `(board.x, column.y)`.
fn swipe_board(delta: Point<gpui::Pixels>, cx: &mut TestAppContext) -> (f32, f32) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| BoardHost {
        board: ScrollHandle::new(),
        column: ScrollHandle::new(),
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    cx.run_until_parked();
    cx.simulate_event(ScrollWheelEvent {
        // Inside the first column, which is inside the board.
        position: point(px(COLUMN / 2.0), px(HEIGHT / 2.0)),
        delta: ScrollDelta::Pixels(delta),
        modifiers: Default::default(),
        touch_phase: Default::default(),
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let host = view.read(cx);
        (
            f32::from(host.board.offset().x),
            f32::from(host.column.offset().y),
        )
    })
}

#[gpui::test]
fn a_column_scrolls_without_panning_the_board(cx: &mut TestAppContext) {
    assert_eq!(swipe_board(downwards(), cx), (0.0, SWIPE));
}

#[gpui::test]
fn the_board_pans_without_scrolling_a_column(cx: &mut TestAppContext) {
    assert_eq!(swipe_board(sideways(), cx), (SWIPE, 0.0));
}
