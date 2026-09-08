//! [`ui::popover::menu_trigger`] under gpui's own harness — a real window, real
//! dispatch phases.
//!
//! The state machine underneath is covered by `popover.rs`. What no pure
//! function can say is whether the two halves land in the phases the fix
//! depends on: `on_mouse_down_out` fires on the press and `on_click` on the
//! release, so a trigger whose note is dispatched after the dismissal reads
//! what the dismissal left and reopens the menu it just shut. Only a window
//! that actually dispatches a press and a release can tell.

use gpui::{Modifiers, TestAppContext, VisualTestContext, div, point, prelude::*, px, size};
use ui::popover::{self, Popup};

/// The card, so a test can ask whether it is on screen.
const CARD: &str = "trigger-card";

/// The trigger's box, and a point inside it. Absolute, so the press lands on
/// the trigger whatever the test text system shapes.
const TRIGGER: (f32, f32, f32, f32) = (20.0, 20.0, 60.0, 24.0);
/// Where the card hangs — clear of the trigger, so pressing the trigger is a
/// press *outside* the card and the dismissal is the one under test.
const CARD_AT: (f32, f32) = (20.0, 60.0);

struct Toggling {
    menu: Popup<()>,
    /// Whether the card dismisses itself on an outside press. Off is the case
    /// a plain toggle gets right and the naive fix gets wrong: nothing closes
    /// the menu on the press, so the click has to.
    dismisses: bool,
}

impl gpui::Render for Toggling {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = theme::Theme::of(cx).clone();
        let (x, y, w, h) = TRIGGER;
        let trigger = popover::menu_trigger(
            div()
                .id("trigger")
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h)),
            |this: &mut Self| &mut this.menu,
            |_| (),
            cx,
        );
        let card = popover::popover_card(&theme)
            .debug_selector(|| CARD.to_string())
            .w(px(120.0))
            .h(px(80.0));
        let card = if self.dismisses {
            popover::dismiss_on_out(card, |this: &mut Self| &mut this.menu, cx)
        } else {
            card
        };
        div()
            .size_full()
            .relative()
            .child(trigger)
            .children(self.menu.get().map(|()| {
                popover::menu_at(
                    "trigger-menu",
                    point(px(CARD_AT.0), px(CARD_AT.1)),
                    card.into_any_element(),
                    self.menu.closing_since(),
                )
            }))
    }
}

fn open(dismisses: bool, cx: &mut TestAppContext) -> VisualTestContext {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Toggling {
        menu: Popup::default(),
        dismisses,
    });
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(400.0), px(300.0)));
    visual.run_until_parked();
    visual
}

/// Press and release on the trigger, then run the exit animation out — so what
/// is left is what stayed, not what is still fading.
fn click_trigger(cx: &mut VisualTestContext) {
    let (x, y, w, h) = TRIGGER;
    cx.simulate_click(
        point(px(x + w / 2.0), px(y + h / 2.0)),
        Modifiers::default(),
    );
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
}

fn card_up(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds(CARD).is_some()
}

#[gpui::test]
fn a_second_click_on_the_trigger_leaves_the_menu_shut(cx: &mut TestAppContext) {
    let mut cx = open(true, cx);

    click_trigger(&mut cx);
    assert!(card_up(&mut cx), "the first click opens it");

    click_trigger(&mut cx);
    assert!(
        !card_up(&mut cx),
        "the card's out-press shut it; the release must not reopen it"
    );

    click_trigger(&mut cx);
    assert!(
        card_up(&mut cx),
        "and it is a toggle, not a one-way latch — the next click opens it again"
    );
}

#[gpui::test]
fn a_card_that_does_not_dismiss_itself_still_toggles(cx: &mut TestAppContext) {
    let mut cx = open(false, cx);

    click_trigger(&mut cx);
    assert!(card_up(&mut cx), "the first click opens it");

    click_trigger(&mut cx);
    assert!(
        !card_up(&mut cx),
        "nothing closed it on the press, so the release has to"
    );
}
