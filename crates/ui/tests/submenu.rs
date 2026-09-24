//! Submenus under gpui's own harness — a real window, real layout, real hit
//! testing.
//!
//! The pure half of the model is covered in `menubar.rs`. What no pure function
//! can say is whether the panel a [`ui::menu::Item::Submenu`] row drops actually
//! *paints*, and paints somewhere the pointer can reach: it hangs on a nested
//! deferred layer, escaping the card's own clip, at an offset nothing in the
//! element tree states. So this drives the pointer at it and asks what it hit.

use gpui::{
    Focusable, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, div, point,
    prelude::*, px, size,
};
use ui::{
    menu::{self, Item},
    menubar::{self, Menu, Menubar},
};

/// `New · Recent › (bezel.md · ─ · Clear) · ─ · Close(disabled)`, on one title
/// so the sweeps below have only one card to find.
const RECENT: usize = 1;

const WIDTH: gpui::Pixels = px(900.0);
const HEIGHT: gpui::Pixels = px(600.0);
/// Fine enough to land inside a row of any height the test system shapes.
const SWEEP: f32 = 2.0;

fn menus() -> Vec<Menu> {
    vec![Menu::new(
        "File",
        vec![
            Item::action("New Window").with_keystroke("⌘N"),
            Item::submenu(
                "Open Recent",
                vec![
                    Item::action("bezel.md"),
                    Item::Separator,
                    Item::action("Clear Menu"),
                ],
            ),
            Item::Separator,
            Item::action("Close").disabled(),
        ],
    )]
}

/// A drawn window with the bar focused and its one menu already down.
fn open(cx: &mut TestAppContext) -> (gpui::Entity<Menubar>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        menubar::init(cx);
    });
    let window = cx.add_window(|_, cx| Menubar::new(menus(), cx));
    let bar = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    // Wide enough that a submenu opening rightward is never snapped back into
    // the window, which would put it on top of the card that dropped it. The
    // test text system shapes a fixed-width em, so the card lands nowhere a
    // constant could name — everything below finds it by sweeping.
    visual.simulate_resize(size(WIDTH, HEIGHT));
    visual.update(|window, cx| {
        let handle = bar.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    // `enter` on a focused, closed bar drops the first menu.
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    (bar, visual)
}

fn cursor(bar: &gpui::Entity<Menubar>, cx: &mut VisualTestContext) -> (Vec<usize>, Option<usize>) {
    cx.update(|_, cx| {
        let cursor = bar.read(cx).cursor();
        (cursor.open().to_vec(), cursor.row())
    })
}

fn move_to(cx: &mut VisualTestContext, at: Point<gpui::Pixels>) {
    cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::default());
}

/// The y the submenu row sits at, found by walking the pointer down the card
/// rather than by arithmetic on row metrics no test should have to know.
fn submenu_row_y(bar: &gpui::Entity<Menubar>, cx: &mut VisualTestContext) -> gpui::Pixels {
    assert_eq!(
        cursor(bar, cx),
        (vec![], None),
        "the menu opens with nothing lit"
    );
    for step in 0..(f32::from(HEIGHT) / SWEEP) as usize {
        let y = px(step as f32 * SWEEP);
        move_to(cx, point(px(24.0), y));
        if cursor(bar, cx).0 == [RECENT] {
            return y;
        }
    }
    panic!("never found the submenu row by sweeping the card");
}

/// The first place to the right of `y` that answers the pointer with a row of
/// its own. Nothing but the panel is painted out there, and a miss lands on
/// nothing at all — which leaves the cursor exactly where it was.
fn probe_right(
    bar: &gpui::Entity<Menubar>,
    cx: &mut VisualTestContext,
    y: gpui::Pixels,
) -> Option<(Point<gpui::Pixels>, Option<usize>)> {
    (0..(f32::from(WIDTH) / SWEEP) as usize).find_map(|step| {
        let at = point(px(step as f32 * SWEEP), y);
        move_to(cx, at);
        let (open, row) = cursor(bar, cx);
        (open == [RECENT] && row.is_some()).then_some((at, row))
    })
}

#[gpui::test]
fn the_panel_paints_where_the_pointer_can_reach_it(cx: &mut TestAppContext) {
    let (bar, mut cx) = open(cx);
    let y = submenu_row_y(&bar, &mut cx);

    // Straight out to the right, along the row that opened it. The panel hangs
    // on a deferred layer of its own — it has to escape the card's `overflow`
    // clip to be there at all, and it lines its first row up with the row it
    // hangs on.
    assert_eq!(
        probe_right(&bar, &mut cx, y).map(|(_, row)| row),
        Some(Some(0)),
        "the panel never answered the pointer to the right of the row it hangs on"
    );
}

#[gpui::test]
fn a_row_in_the_panel_answers_a_click(cx: &mut TestAppContext) {
    let (bar, mut cx) = open(cx);
    let y = submenu_row_y(&bar, &mut cx);
    let (at, _) = probe_right(&bar, &mut cx, y).expect("no row in the panel");

    // The press lands outside the card that dropped the panel, which is the
    // shape a bounds-only out-click test reads as a click away.
    cx.simulate_click(at, Modifiers::default());
    cx.run_until_parked();
    assert!(
        cx.update(|_, cx| bar.read(cx).open_menu().is_none()),
        "choosing a row in a submenu closes the bar"
    );
}

#[gpui::test]
fn the_arrows_walk_into_the_panel_and_back_out(cx: &mut TestAppContext) {
    let (bar, mut cx) = open(cx);
    // Down onto the first row, down again onto the submenu row, right into it.
    cx.simulate_keystrokes("down down right");
    assert_eq!(cursor(&bar, &mut cx), (vec![RECENT], Some(0)));

    cx.simulate_keystrokes("left");
    assert_eq!(cursor(&bar, &mut cx), (vec![], Some(RECENT)));

    // `enter` on a submenu row opens it the way `right` does.
    cx.simulate_keystrokes("enter");
    assert_eq!(cursor(&bar, &mut cx), (vec![RECENT], Some(0)));
    assert!(cx.update(|_, cx| bar.read(cx).open_menu().is_some()));

    // `escape` closes the level, then the bar.
    cx.simulate_keystrokes("escape");
    assert_eq!(cursor(&bar, &mut cx), (vec![], Some(RECENT)));
    cx.simulate_keystrokes("escape");
    assert!(cx.update(|_, cx| bar.read(cx).open_menu().is_none()));
}

// ---------------------------------------------------------------------------
// Flipping at the window edge
// ---------------------------------------------------------------------------

/// A card pinned to the window's right edge with its submenus already down.
/// The bar cannot set this up — its titles are all on the left — so this drives
/// [`ui::menu::card`] directly, which is the shape any host mounts it in.
struct Pinned {
    items: Vec<Item>,
    cursor: menu::Cursor,
    /// The row the pointer last landed on, recorded rather than acted on: the
    /// open chain has to hold still while the sweeps below walk across it.
    hit: Option<Vec<usize>>,
}

/// `New · Recent › (More › (bezel.md) · Clear)`, every submenu row first in its
/// panel so all three levels line up on one y.
fn nested() -> Vec<Item> {
    vec![
        Item::action("New Window"),
        Item::submenu(
            "Open Recent",
            vec![
                Item::submenu("More", vec![Item::action("bezel.md")]),
                Item::action("Clear Menu"),
            ],
        ),
    ]
}

impl gpui::Render for Pinned {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = theme::Theme::of(cx).clone();
        div()
            .size_full()
            .flex()
            .justify_end()
            .items_start()
            .child(menu::card(
                &theme,
                "pinned",
                &self.items,
                &self.cursor,
                window,
                cx,
                |view: &mut Self, hit, _, _| {
                    if let menu::Hit::Point(path) = hit {
                        view.hit = Some(path);
                    }
                },
            ))
    }
}

/// A drawn window holding [`nested`] at the right edge, opened down `open`.
fn pinned(cx: &mut TestAppContext, open: &[usize]) -> (gpui::Entity<Pinned>, VisualTestContext) {
    pinned_with(cx, nested(), open)
}

fn pinned_with(
    cx: &mut TestAppContext,
    items: Vec<Item>,
    open: &[usize],
) -> (gpui::Entity<Pinned>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| {
        let mut cursor = menu::Cursor::default();
        cursor.point_at(&items, open);
        Pinned {
            items,
            cursor,
            hit: None,
        }
    });
    let view = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(WIDTH, HEIGHT));
    visual.run_until_parked();
    (view, visual)
}

/// The row under `at`, or `None` where nothing answers.
fn row_at(
    view: &gpui::Entity<Pinned>,
    cx: &mut VisualTestContext,
    at: Point<gpui::Pixels>,
) -> Option<Vec<usize>> {
    cx.update(|_, cx| view.update(cx, |view, _| view.hit = None));
    move_to(cx, at);
    cx.update(|_, cx| view.read(cx).hit.clone())
}

/// The leftmost x at `y` that answers with `path`.
fn sweep_x(
    view: &gpui::Entity<Pinned>,
    cx: &mut VisualTestContext,
    y: gpui::Pixels,
    path: &[usize],
) -> Option<gpui::Pixels> {
    (0..(f32::from(WIDTH) / SWEEP) as usize).find_map(|step| {
        let x = px(step as f32 * SWEEP);
        (row_at(view, cx, point(x, y)).as_deref() == Some(path)).then_some(x)
    })
}

/// The y the submenu row sits at, found by walking down the pinned card.
fn pinned_row_y(view: &gpui::Entity<Pinned>, cx: &mut VisualTestContext) -> gpui::Pixels {
    let x = WIDTH - px(24.0);
    for step in 0..(f32::from(HEIGHT) / SWEEP) as usize {
        let y = px(step as f32 * SWEEP);
        if row_at(view, cx, point(x, y)).as_deref() == Some(&[RECENT][..]) {
            return y;
        }
    }
    panic!("never found the submenu row by sweeping the pinned card");
}

#[gpui::test]
fn a_panel_with_no_room_to_its_right_opens_leftward(cx: &mut TestAppContext) {
    let (view, mut cx) = pinned(cx, &[RECENT]);
    let y = pinned_row_y(&view, &mut cx);

    let row = sweep_x(&view, &mut cx, y, &[RECENT]).expect("no submenu row in the pinned card");
    let panel = sweep_x(&view, &mut cx, y, &[RECENT, 0]).expect("the panel answered nowhere");
    assert!(
        panel < row,
        "the panel opened at the window edge ({panel}) instead of flipping to the card's left ({row})"
    );
}

#[gpui::test]
fn a_flipped_chain_keeps_going_the_same_way(cx: &mut TestAppContext) {
    let (view, mut cx) = pinned(cx, &[RECENT, 0]);
    let y = pinned_row_y(&view, &mut cx);

    // The second panel has room to the right of the first — that room is the
    // card the first one flipped away from, and opening back into it would
    // bury the menu under its own parent.
    let first = sweep_x(&view, &mut cx, y, &[RECENT, 0]).expect("the first panel answered nowhere");
    let second =
        sweep_x(&view, &mut cx, y, &[RECENT, 0, 0]).expect("the second panel answered nowhere");
    assert!(
        second < first,
        "the second panel opened back across its parent ({second} is right of {first})"
    );
}

// ---------------------------------------------------------------------------
// A panel taller than the window
// ---------------------------------------------------------------------------

/// Rows `0..LONG - 1` are actions; the last row is a submenu.
const LONG: usize = 80;

fn long() -> Vec<Item> {
    (0..LONG - 1)
        .map(|row| Item::action(format!("Model {row}")))
        .chain([Item::submenu("More", vec![Item::action("bezel.md")])])
        .collect()
}

/// The first y down the pinned card's column that answers with `path`.
fn find_y(
    view: &gpui::Entity<Pinned>,
    cx: &mut VisualTestContext,
    path: &[usize],
) -> Option<gpui::Pixels> {
    let x = WIDTH - px(24.0);
    (0..(f32::from(HEIGHT) / SWEEP) as usize)
        .map(|step| px(step as f32 * SWEEP))
        .find(|&y| row_at(view, cx, point(x, y)).as_deref() == Some(path))
}

#[gpui::test]
fn a_long_panel_scrolls_to_the_row_the_keyboard_steps_to(cx: &mut TestAppContext) {
    let (view, mut cx) = pinned_with(cx, long(), &[]);
    assert!(
        find_y(&view, &mut cx, &[0]).is_some(),
        "the first row answered nowhere"
    );
    assert_eq!(
        find_y(&view, &mut cx, &[LONG - 2]),
        None,
        "a row past the window edge answered before anything scrolled"
    );

    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            // Up from nothing lands on the last row.
            view.cursor.step(&view.items, -1);
            view.cursor.step(&view.items, -1);
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert!(
        find_y(&view, &mut cx, &[LONG - 2]).is_some(),
        "the stepped-to row was not scrolled into view"
    );
}

#[gpui::test]
fn a_submenu_off_a_scrolled_row_is_not_clipped(cx: &mut TestAppContext) {
    let (view, mut cx) = pinned_with(cx, long(), &[LONG - 1]);
    // The scroll is measured against the viewport of the frame before, which
    // the resize changed; a real window draws the frame that corrects it.
    cx.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
    cx.run_until_parked();
    let y =
        find_y(&view, &mut cx, &[LONG - 1]).expect("the submenu row was not scrolled into view");
    assert!(
        sweep_x(&view, &mut cx, y, &[LONG - 1, 0]).is_some(),
        "the panel off the scrolled row answered nowhere"
    );
}
