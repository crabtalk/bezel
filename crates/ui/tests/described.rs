//! Described rows under gpui's own harness — a real window, real layout.
//!
//! What the builder tests in `menubar.rs` cannot say is what a description
//! *does* to the panel it lands in. Two things have to hold at once, and each
//! one breaks the other if it is the only one written: the row keeps the
//! sentence to a single line ([`ui::menu::Item::with_tooltip`] carries the
//! rest), and the panel holds a width of its own to clip that line against. A
//! nowrap line in an auto-width panel does not clip — the panel simply grows
//! to whatever the sentence measures, which is the same paragraph problem in a
//! wider shape.
//!
//! So neither is measured against a pixel constant. Both are measured against
//! *the same menu with a shorter sentence*: what the panel must not do is
//! change size when the words get longer.

use gpui::{
    Focusable, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px, size,
};
use ui::{
    menu::Item,
    menubar::{self, Menu, Menubar},
};

const WIDTH: gpui::Pixels = px(900.0);
const HEIGHT: gpui::Pixels = px(600.0);
/// Fine enough to land inside a row of any height the test system shapes.
const SWEEP: f32 = 2.0;
/// An x that is inside the card at any of the widths under test.
const INSIDE: gpui::Pixels = px(24.0);

/// A sentence, and the same sentence with more of it. No panel in the crate is
/// wide enough for the long one on a single line, whatever the test system
/// shapes an em at.
const SHORT: &str = "Choose a file";
const LONG: &str = "Choose a markdown file from somewhere deep inside this workspace and open it in the editor to edit";

/// `Open…(described) · Close`, on one title so there is only one card to find.
fn menus(description: Option<&'static str>) -> Vec<Menu> {
    let open = Item::action("Open…");
    vec![Menu::new(
        "File",
        vec![
            match description {
                Some(copy) => open.with_description(copy),
                None => open,
            },
            Item::action("Close"),
        ],
    )]
}

/// A drawn window with the bar focused and its one menu already down.
fn open(cx: &mut TestAppContext, description: Option<&'static str>) -> Card {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        menubar::init(cx);
    });
    let window = cx.add_window(|_, cx| Menubar::new(menus(description), cx));
    let bar = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(WIDTH, HEIGHT));
    visual.update(|window, cx| {
        let handle = bar.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    Card { bar, cx: visual }
}

/// One open menu, and the pointer that measures it. Nothing here reads a row's
/// bounds — gpui hands a test no element bounds without a debug selector baked
/// into the element itself — so every measurement below is the pointer asking
/// what it landed on.
struct Card {
    bar: gpui::Entity<Menubar>,
    cx: VisualTestContext,
}

impl Card {
    /// Which row the pointer is on after moving to `at`. `None` means the move
    /// landed on nothing — and *left the cursor where it was*, which is why
    /// every probe below parks the cursor somewhere known first.
    fn hit(&mut self, at: Point<gpui::Pixels>) -> Option<usize> {
        self.cx
            .simulate_mouse_move(at, MouseButton::Left, Modifiers::default());
        let bar = &self.bar;
        self.cx.update(|_, cx| bar.read(cx).cursor().row())
    }

    /// The first y down the card that answers with `row`, walked rather than
    /// worked out from metrics no test should have to know.
    fn top_of(&mut self, row: usize) -> gpui::Pixels {
        for step in 0..(f32::from(HEIGHT) / SWEEP) as usize {
            let y = px(step as f32 * SWEEP);
            if self.hit(point(INSIDE, y)) == Some(row) {
                return y;
            }
        }
        panic!("row {row} never answered the pointer");
    }

    /// How tall the first row stands: from where it starts answering to where
    /// the row under it takes over. A description that wrapped would show up
    /// here and nowhere else.
    fn first_row_height(&mut self) -> gpui::Pixels {
        self.top_of(1) - self.top_of(0)
    }

    /// Whether `x`, along the first row, is inside the card. Parks the cursor
    /// on the second row first: a miss changes nothing, so the answer is
    /// whether the cursor *moved* rather than what it reads.
    fn inside(&mut self, x: gpui::Pixels, y: gpui::Pixels) -> bool {
        let park = self.top_of(1);
        self.hit(point(INSIDE, park));
        self.hit(point(x, y)) == Some(0)
    }

    /// How wide the card's rows run, found by closing in on each edge from a
    /// point known to be inside. Binary search rather than a sweep: every
    /// probe costs a parked cursor and a redraw.
    fn first_row_width(&mut self) -> gpui::Pixels {
        let y = self.top_of(0) + px(SWEEP);
        assert!(self.inside(INSIDE, y), "the sweep never found the card");
        let mut left = (px(0.0), INSIDE);
        while left.1 - left.0 > px(1.0) {
            let mid = (left.0 + left.1) / 2.0;
            if self.inside(mid, y) {
                left.1 = mid;
            } else {
                left.0 = mid;
            }
        }
        let mut right = (INSIDE, WIDTH);
        while right.1 - right.0 > px(1.0) {
            let mid = (right.0 + right.1) / 2.0;
            if self.inside(mid, y) {
                right.0 = mid;
            } else {
                right.1 = mid;
            }
        }
        right.0 - left.1
    }
}

#[gpui::test]
fn a_longer_description_does_not_widen_the_panel(cx: &mut TestAppContext) {
    let short = open(cx, Some(SHORT)).first_row_width();
    let long = open(cx, Some(LONG)).first_row_width();
    assert!(
        (long - short).abs() <= px(2.0),
        "the panel grew with the sentence instead of clipping it: {short} then {long}"
    );
}

#[gpui::test]
fn a_longer_description_does_not_heighten_the_row(cx: &mut TestAppContext) {
    let short = open(cx, Some(SHORT)).first_row_height();
    let long = open(cx, Some(LONG)).first_row_height();
    assert!(
        (long - short).abs() <= px(SWEEP),
        "the description wrapped instead of staying on one line: {short} then {long}"
    );
}

#[gpui::test]
fn a_described_panel_is_the_wider_one(cx: &mut TestAppContext) {
    // The rule the width is there for in the first place: one described row
    // widens the panel, the way one icon opens the glyph gutter.
    let plain = open(cx, None).first_row_width();
    let described = open(cx, Some(SHORT)).first_row_width();
    assert!(
        described > plain,
        "a described panel is no wider than a plain one: {plain} then {described}"
    );
}
