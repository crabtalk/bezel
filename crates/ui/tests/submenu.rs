//! Submenus under gpui's own harness — a real window, real layout, real hit
//! testing.
//!
//! The pure half of the model is covered in `menubar.rs`. What no pure function
//! can say is whether the panel a [`ui::menu::Item::Submenu`] row drops actually
//! *paints*, and paints somewhere the pointer can reach: it hangs on a nested
//! deferred layer, escaping the card's own clip, at an offset nothing in the
//! element tree states. So this drives the pointer at it and asks what it hit.

use gpui::{
    Focusable, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px, size,
};
use ui::{
    menu::Item,
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
        // The panel enters on `motion::menu_in`, which holds it up to 2px above
        // where it lands — one whole `SWEEP`. Snapped to its end state the
        // sweeps below measure the panel rather than however far the entrance
        // happened to have got by the time the harness looked.
        motion::AppExt::set_reduced_motion(cx, true);
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
