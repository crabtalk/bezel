//! Which axis a gesture moves, under gpui's own dispatch.
//!
//! gpui makes scrollability a style field with no default and then guesses at
//! the underspecified case: when a gesture's own axis reads zero, it **remaps
//! the delta onto whichever axis the container can scroll**. A sideways swipe
//! scrolls a vertical list down; a downward swipe pans a wide table sideways.
//!
//! [`ui::scroll::pane`] exists to close that, so these are the tests that would
//! catch it reopening — each one fires a single-axis gesture at a pane built
//! both ways and asserts the plain `div` is wrong where the pane is right.

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

/// How the pane under test was built.
#[derive(Clone, Copy)]
enum Built {
    /// What an app writes without knowing about any of this.
    Plain(Axes),
    /// What [`scroll::pane`] gives it.
    Pane(Axes),
}

struct Host {
    scroll: ScrollHandle,
    built: Built,
}

impl gpui::Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let content = div().w(px(CONTENT)).h(px(CONTENT));
        match self.built {
            Built::Plain(axes) => {
                let el = div().id("pane").size_full();
                match axes {
                    Axes::Vertical => el.overflow_y_scroll(),
                    Axes::Horizontal => el.overflow_x_scroll(),
                    Axes::Both => el.overflow_scroll(),
                }
                .track_scroll(&self.scroll)
                .child(content)
                .into_any_element()
            }
            Built::Pane(axes) => scroll::pane("pane", axes)
                .size_full()
                .track_scroll(&self.scroll)
                .child(content)
                .into_any_element(),
        }
    }
}

/// One gesture at the middle of the pane, and where it left the offset.
fn swipe(built: Built, delta: Point<gpui::Pixels>, cx: &mut TestAppContext) -> (f32, f32) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Host {
        scroll: ScrollHandle::new(),
        built,
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

/// The headline bug: a two-finger sideways swipe scrolls a vertical list down.
#[gpui::test]
fn a_sideways_swipe_leaves_a_vertical_pane_where_it_was(cx: &mut TestAppContext) {
    assert_eq!(
        swipe(Built::Plain(Axes::Vertical), sideways(), cx),
        (0.0, SWIPE),
        "the bug: a plain overflow_y_scroll div takes the x delta as y"
    );
    assert_eq!(
        swipe(Built::Pane(Axes::Vertical), sideways(), cx),
        (0.0, 0.0)
    );
}

/// The same thing the other way round — the one markdown's code fences and
/// tables hit, where a scroll down the page turns sideways as the pointer
/// crosses a wide block.
#[gpui::test]
fn a_downward_swipe_leaves_a_horizontal_pane_where_it_was(cx: &mut TestAppContext) {
    assert_eq!(
        swipe(Built::Plain(Axes::Horizontal), downwards(), cx),
        (SWIPE, 0.0),
        "the bug: a plain overflow_x_scroll div takes the y delta as x"
    );
    assert_eq!(
        swipe(Built::Pane(Axes::Horizontal), downwards(), cx),
        (0.0, 0.0)
    );
}

/// A pane still moves along the axis it was asked for — the fix is not "ignore
/// everything", which a guard this blunt could easily become.
#[gpui::test]
fn a_pane_still_answers_its_own_axis(cx: &mut TestAppContext) {
    assert_eq!(
        swipe(Built::Pane(Axes::Vertical), downwards(), cx),
        (0.0, SWIPE)
    );
    assert_eq!(
        swipe(Built::Pane(Axes::Horizontal), sideways(), cx),
        (SWIPE, 0.0)
    );
}

/// A both-axes pane answers either gesture, and neither is remapped onto the
/// other — there is nothing to remap when both are asked for.
#[gpui::test]
fn a_both_axes_pane_answers_either(cx: &mut TestAppContext) {
    assert_eq!(swipe(Built::Pane(Axes::Both), sideways(), cx), (SWIPE, 0.0));
    assert_eq!(
        swipe(Built::Pane(Axes::Both), downwards(), cx),
        (0.0, SWIPE)
    );
}

// ---------------------------------------------------------------------------
// A board: columns that scroll down, inside a board that scrolls across
// ---------------------------------------------------------------------------

/// One column's width. Four in a 400pt viewport, so the board has somewhere to
/// go sideways.
const COLUMN: f32 = 150.0;
const COLUMNS: usize = 4;

/// The kanban shape, where every one of these failures meets at once: a
/// horizontal pane whose children are vertical panes. The pointer is over both
/// at the same time, and each is the other's ancestor for the axis it does not
/// scroll.
struct BoardHost {
    board: ScrollHandle,
    column: ScrollHandle,
    /// Panes, or the plain divs an app writes knowing none of this.
    fixed: bool,
}

impl BoardHost {
    /// The first column — the one under the pointer, and the one measured. The
    /// others only make the board wider than its viewport.
    fn column(&self) -> AnyElement {
        let cards = div().w_full().h(px(CONTENT));
        let inner = match self.fixed {
            true => scroll::pane("column", Axes::Vertical).size_full(),
            false => div().id("column").size_full().overflow_y_scroll(),
        };
        div()
            .flex_none()
            .w(px(COLUMN))
            .h_full()
            .child(inner.track_scroll(&self.column).child(cards))
            .into_any_element()
    }
}

impl gpui::Render for BoardHost {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let outer = match self.fixed {
            true => scroll::pane("board", Axes::Horizontal).size_full(),
            false => div().id("board").size_full().overflow_x_scroll(),
        };
        div().size_full().child(
            outer
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
fn swipe_board(fixed: bool, delta: Point<gpui::Pixels>, cx: &mut TestAppContext) -> (f32, f32) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| BoardHost {
        board: ScrollHandle::new(),
        column: ScrollHandle::new(),
        fixed,
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

/// Scrolling a column down must not drag the board sideways under it. Plainly
/// built, it does: the board scrolls x only, so gpui hands it the y delta.
#[gpui::test]
fn a_column_scrolls_without_panning_the_board(cx: &mut TestAppContext) {
    assert_eq!(
        swipe_board(false, downwards(), cx),
        (SWIPE, SWIPE),
        "the bug: the column scrolls down and the board pans across with it"
    );
    assert_eq!(swipe_board(true, downwards(), cx), (0.0, SWIPE));
}

/// And panning the board across must not scroll the column down. Plainly
/// built, the column takes the x delta as y before the board ever sees it.
#[gpui::test]
fn the_board_pans_without_scrolling_a_column(cx: &mut TestAppContext) {
    assert_eq!(
        swipe_board(false, sideways(), cx),
        (SWIPE, SWIPE),
        "the bug: the board pans across and the column scrolls down with it"
    );
    assert_eq!(swipe_board(true, sideways(), cx), (SWIPE, 0.0));
}
