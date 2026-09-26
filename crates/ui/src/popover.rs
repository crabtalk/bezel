//! Popover / menu primitives: an anchored floating layer with the `menu-in`
//! animation, outside-click dismissal, and pure keyboard-navigation + search
//! reducers shared by every picker and menu (feature-inventory §1.12 popovers).
//!
//! gpui pattern (examples/popover.rs at the pinned rev): the trigger element
//! conditionally children a `deferred(anchored().child(content))` — deferred
//! paints on a floating layer above everything, anchored positions it relative
//! to the trigger (or an explicit point for context menus).
//!
//! Pure logic (wrap-around list navigation, ranked substring filtering, key
//! classification) lives in free functions with unit tests; the elements only
//! feed them measurements/events.

mod anchor;
mod modal;
mod nav;
mod parts;
pub mod submenu;

pub use anchor::*;
pub use modal::*;
pub use nav::*;
pub use parts::*;
pub use submenu::Chain;

use crate::{icons, stack};
use gpui::{
    Anchor, AnyElement, ElementId, IntoElement, Pixels, Point, SharedString, div, prelude::*, px,
};
use icons::Icon;
use motion::{self as motion, AnimationExt as _, Fade, PULSE, Painter};
use theme::{TextStyle, Theme, Typeset, hairline, ink};

// ---------------------------------------------------------------------------
// Loadable — async slot state shared by pickers/settings pages
// ---------------------------------------------------------------------------

/// One async-loaded slot: `Idle` (never requested) → `Loading` (skeletons) →
/// `Ready` / `Error` (inline message + Retry).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Loadable<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Error(String),
}

impl<T> Loadable<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Loadable::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Loadable::Loading)
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Loadable::Error(message) => Some(message),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Popup — open/closing/closed lifecycle (exit animations)
// ---------------------------------------------------------------------------

/// Popup state with an exit phase. gpui unmounts an element the frame its
/// state drops, so a closing animation needs the state held alive while
/// [`motion::menu_out`] plays: `open` → `begin_close` (render keeps mounting,
/// with the out animation and dead hit-testing) → [`reap_popup`]'s timer
/// `finish_close`es ~[`motion::MENU_OUT`] later. Use [`Self::is_open`] for
/// logic (a closing popup already reads as closed) and [`Self::get`] /
/// [`Self::is_closing`] for rendering.
pub struct Popup<T> {
    /// `Some((state, closing_since))` while mounted; `closing_since` is the
    /// exit-phase start.
    inner: Option<(T, Option<web_time::Instant>)>,
    /// Whether the popup was still mounted when the current trigger press
    /// began — see [`Self::note_trigger_press`].
    pressed_while_open: bool,
    generation: u64,
}

impl<T> Default for Popup<T> {
    fn default() -> Self {
        Self {
            inner: None,
            pressed_while_open: false,
            generation: 0,
        }
    }
}

impl<T> Popup<T> {
    pub fn open(&mut self, value: T) {
        self.generation = self.generation.wrapping_add(1);
        self.inner = Some((value, None));
    }

    /// Open and interactive (not closing).
    pub fn is_open(&self) -> bool {
        matches!(self.inner, Some((_, None)))
    }

    pub fn is_closing(&self) -> bool {
        matches!(self.inner, Some((_, Some(_))))
    }

    /// When the exit phase began — what the render path hands to the popover
    /// wrappers, which derive the eased exit progress from it each frame.
    pub fn closing_since(&self) -> Option<web_time::Instant> {
        match &self.inner {
            Some((_, Some(since))) => Some(*since),
            _ => None,
        }
    }

    /// The state while mounted — open OR playing the exit animation. Render
    /// paths use this; logic paths use [`Self::as_open`]/[`Self::open_mut`].
    pub fn get(&self) -> Option<&T> {
        self.inner.as_ref().map(|(value, _)| value)
    }

    /// The state only while genuinely open — `None` during the exit phase, so
    /// event handlers on a dying popup fall through.
    pub fn as_open(&self) -> Option<&T> {
        match &self.inner {
            Some((value, None)) => Some(value),
            _ => None,
        }
    }

    pub fn open_mut(&mut self) -> Option<&mut T> {
        match &mut self.inner {
            Some((value, None)) => Some(value),
            _ => None,
        }
    }

    /// Unmount now, with no exit phase.
    ///
    /// For a surface that has no exit animation to play — [`modal`], which
    /// takes no `closing` and paints the same either way. Sending one of those
    /// through [`Self::begin_close`] buys nothing and costs everything: it
    /// stays fully painted for the animation's span, and if the reap never
    /// lands it stays forever, because nothing retries.
    ///
    /// Not for a popup something toggles: unmounting on the press erases what
    /// [`Self::note_trigger_press`] has to read, so the note comes back false
    /// and the trigger reopens what it just shut. Those close through
    /// [`close_popup`].
    pub fn close(&mut self) {
        self.inner = None;
    }

    /// Enter the exit phase. Returns `true` when this call started it (the
    /// caller then schedules [`reap_popup`]); `false` if already closing or
    /// closed.
    pub fn begin_close(&mut self) -> bool {
        match &mut self.inner {
            Some((_, closing @ None)) => {
                self.generation = self.generation.wrapping_add(1);
                *closing = Some(web_time::Instant::now());
                true
            }
            _ => false,
        }
    }

    /// Record, from the trigger's `on_mouse_down`, whether this popup is
    /// still mounted. The anchored card's `on_mouse_down_out` fires on that
    /// same press and begins the close, so by click (mouse-up) time the
    /// popup already reads as closed — the click handler alone cannot tell
    /// "this press dismissed it; stay closed" from "open fresh", and a
    /// plain toggle closes-and-reopens (user report). Both handler orders
    /// work: open and mid-exit each count as mounted. Every trigger click
    /// is preceded by a trigger mouse-down, so the note is never stale.
    ///
    /// [`menu_trigger`] wires this and the matching click together. Reach for
    /// it directly only when the open is not a click — a gutter handle whose
    /// menu belongs to the release of a possible drag.
    pub fn note_trigger_press(&mut self) {
        self.note_trigger_press_matching(|_| true);
    }

    /// [`Self::note_trigger_press`] for popups whose state distinguishes
    /// which trigger owns them (e.g. one `Popup<PickerKind>` shared by
    /// several triggers): only a press on the OWNING trigger counts, so
    /// clicking a different trigger switches menus instead of swallowing.
    pub fn note_trigger_press_matching(&mut self, owns: impl FnOnce(&T) -> bool) {
        self.pressed_while_open = self.inner.as_ref().is_some_and(|(value, _)| owns(value));
    }

    /// Consume the press note: `true` when the press that produced the
    /// current click found the popup mounted — the click should leave it
    /// closed rather than reopen it.
    pub fn take_press_was_open(&mut self) -> bool {
        std::mem::take(&mut self.pressed_while_open)
    }

    /// Drop the state now the exit phase has run its course. A popup reopened
    /// since the matching [`Self::begin_close`] is left alone — it is `None`
    /// again in the second slot, and the newer phase's own reap handles it.
    ///
    /// It does not re-check the clock. [`reap_popup`] already waited out the
    /// span on the executor's timer, and asking `Instant::elapsed` to agree
    /// makes one deadline depend on two clocks — where they disagree, and a
    /// throttled executor is where, the popup is stranded open with nothing
    /// left to retry.
    pub fn finish_close(&mut self) {
        if matches!(&self.inner, Some((_, Some(_)))) {
            self.inner = None;
        }
    }
}

/// Schedule the reap for a [`Popup::begin_close`]: after the exit animation's
/// span, drop the popup state and repaint. `popup` re-borrows the field from
/// the view (the state can't be captured — the view owns it).
pub fn reap_popup<V: 'static, T: 'static>(
    view: &mut V,
    cx: &mut gpui::Context<V>,
    popup: impl Fn(&mut V) -> &mut Popup<T> + 'static,
) {
    let generation = popup(view).generation;
    cx.spawn(async move |view, cx| {
        cx.background_executor()
            .timer(
                motion::MENU_OUT
                    .total()
                    .mul_f32(motion::speed_scale())
                    .saturating_add(std::time::Duration::from_millis(20)),
            )
            .await;
        view.update(cx, |view, cx| {
            let popup = popup(view);
            if popup.generation == generation && popup.is_closing() {
                popup.finish_close();
                cx.notify();
            }
        })
        .ok();
    })
    .detach();
}

/// Begin a popup's exit phase and schedule its reap — [`Popup::begin_close`]
/// and [`reap_popup`], which are only ever correct together. A popup already
/// closing or closed is left alone.
pub fn close_popup<V: 'static, T: 'static>(
    view: &mut V,
    cx: &mut gpui::Context<V>,
    popup: impl Fn(&mut V) -> &mut Popup<T> + Copy + 'static,
) {
    if popup(view).begin_close() {
        reap_popup(view, cx, popup);
        cx.notify();
    }
}

/// Dismiss `popup` on a press outside `el` — the card side of the pair
/// [`menu_trigger`] completes.
///
/// It closes through [`close_popup`] rather than [`Popup::close`], and that is
/// load-bearing rather than cosmetic: the exit phase is what keeps the popup
/// reading as mounted while the trigger's own press handler runs, whichever of
/// the two the frame happens to dispatch first.
///
/// The press is swallowed, as [`crate::menu::card`]'s own dismissal swallows
/// it: this listener runs in the capture phase, and stopping there skips the
/// bubble phase where gpui records the press a click is built from. A press
/// that dismisses an open card is spent on the dismissal, and whatever it
/// landed on is one more press away.
pub fn dismiss_on_out<V: 'static, T: 'static, E: gpui::InteractiveElement>(
    el: E,
    popup: impl Fn(&mut V) -> &mut Popup<T> + Copy + 'static,
    cx: &gpui::Context<V>,
) -> E {
    el.on_mouse_down_out(cx.listener(move |view, _: &gpui::MouseDownEvent, _, cx| {
        close_popup(view, cx, popup);
        cx.stop_propagation();
    }))
}

/// Wire a trigger to the popup it toggles: press note on the way down, open or
/// close on the way up.
///
/// The press/release split is why this exists rather than a plain
/// `on_click`. `on_mouse_down_out` fires on the **press** and `on_click` on the
/// **release**, so one physical click on the trigger of an open menu runs the
/// card's dismissal first and the trigger's toggle second — by which time the
/// state a toggle would branch on is already gone, and the menu reopens on the
/// click that shut it (user report). No click handler can tell the two apart;
/// the note has to be taken in the phase the dismissal cannot precede.
///
/// `value` is what to open with, from the click that opened it — a point for a
/// menu anchored where it was pressed, or `move |_| ..` for one that already
/// knows.
///
/// This is a true toggle either way: a card that dismisses itself on the
/// out-press is already closing by the release and the close here is a no-op,
/// while a card that does not (one dismissed by Escape alone, or one the
/// trigger sits inside) is closed by the release itself.
pub fn menu_trigger<V: 'static, T: 'static, E: gpui::StatefulInteractiveElement>(
    el: E,
    popup: impl Fn(&mut V) -> &mut Popup<T> + Copy + 'static,
    value: impl Fn(&gpui::ClickEvent) -> T + 'static,
    cx: &gpui::Context<V>,
) -> E {
    menu_trigger_matching(el, popup, |_| true, value, cx)
}

/// [`menu_trigger`] for one popup shared by several triggers (a `Popup<Menu>`
/// with a row's index inside it): `owns` says whether the open popup is *this*
/// trigger's, so pressing another trigger switches menus instead of swallowing
/// the press. See [`Popup::note_trigger_press_matching`].
pub fn menu_trigger_matching<V: 'static, T: 'static, E: gpui::StatefulInteractiveElement>(
    el: E,
    popup: impl Fn(&mut V) -> &mut Popup<T> + Copy + 'static,
    owns: impl Fn(&T) -> bool + 'static,
    value: impl Fn(&gpui::ClickEvent) -> T + 'static,
    cx: &gpui::Context<V>,
) -> E {
    trigger_press_matching(el, popup, owns, cx).on_click(cx.listener(
        move |view, event: &gpui::ClickEvent, _, cx| {
            if popup(view).take_press_was_open() {
                close_popup(view, cx, popup);
            } else {
                popup(view).open(value(event));
            }
            cx.notify();
        },
    ))
}

/// The press half of [`menu_trigger`] on its own, for a trigger whose open is
/// more than `Popup::open` — a combobox that clears its query and focuses it, a
/// gutter handle whose menu belongs to the release of a possible drag. The
/// click side is then the caller's, and reads the note with
/// [`Popup::take_press_was_open`].
pub fn trigger_press<V: 'static, T: 'static, E: gpui::InteractiveElement>(
    el: E,
    popup: impl Fn(&mut V) -> &mut Popup<T> + Copy + 'static,
    cx: &gpui::Context<V>,
) -> E {
    trigger_press_matching(el, popup, |_| true, cx)
}

/// [`trigger_press`] for one popup shared by several triggers — see
/// [`menu_trigger_matching`].
pub fn trigger_press_matching<V: 'static, T: 'static, E: gpui::InteractiveElement>(
    el: E,
    popup: impl Fn(&mut V) -> &mut Popup<T> + Copy + 'static,
    owns: impl Fn(&T) -> bool + 'static,
    cx: &gpui::Context<V>,
) -> E {
    // Capture, not bubble: `on_mouse_down_out` is itself a capture-phase
    // listener, so a bubble-phase note is dispatched after every dismissal in
    // the frame and reads whatever they left behind. Survivable while every
    // dismissal goes through [`close_popup`] — mid-exit still counts as
    // mounted — and not survivable the moment one reaches for [`Popup::close`],
    // which is a footgun to leave lying under a component's own trigger.
    el.capture_any_mouse_down(cx.listener(move |view, _: &gpui::MouseDownEvent, _, _| {
        popup(view).note_trigger_press_matching(&owns);
    }))
}

// ---------------------------------------------------------------------------
// Elements
// ---------------------------------------------------------------------------

/// The floating-menu surface: `rounded-xl border border-white/[0.1] p-1` over
/// whichever look [`Theme::menu_style`] names — the hairline and the baked-in
/// shadow are the card's, and the surface under it paints everything inside
/// them. Opaque platforms keep the near-opaque tone the reference composites
/// to on the dark panels (~#161616).
/// The inner inset of a [`popover_card`], and so the amount [`menu_row`]'s
/// corners come in by. Named because two things read it: the card's padding and
/// its rows' radius. Change it and the rows follow.
pub(crate) const MENU_PAD: f32 = 4.0;

/// The gap a floating layer keeps from the window edge once it has run out of
/// room and is snapped back inside it.
pub(crate) const SNAP: f32 = 8.0;

/// A [`menu_row`]'s padding above and below its line box. Named because a
/// list that caps itself at a row count has to know how tall a row is.
pub(crate) const MENU_ROW_PAD_Y: f32 = 6.0;

/// How tall a one-line [`menu_row`] paints. Derived rather than stored: the
/// two would drift, and it moves with the reader's text size.
pub(crate) fn menu_row_height() -> f32 {
    TextStyle::Body.painted_line_height() + 2.0 * MENU_ROW_PAD_Y
}

pub fn popover_card(theme: &Theme) -> gpui::Div {
    let card = div()
        .rounded(px(Theme::surface_radius()))
        .p(px(MENU_PAD))
        .overflow_hidden()
        .text_style(TextStyle::Body)
        .text_color(theme.text);
    // Contents only. Fill, boundary and shadow are the surface's — every look
    // paints its own, so nothing here has to know which one is under it. An
    // card with the recipes off has no surface at all, and keeps the fill.
    if theme.glass {
        card
    } else {
        card.bg(theme.surface_overlay)
            .border_1()
            .border_color(hairline(0.10))
            .shadow_lg()
    }
}

/// [`popover_card`] without the `p-1` inset — for popovers that manage their
/// own internal panes (the harness/model picker's rail + list split).
pub fn popover_card_flush(theme: &Theme) -> gpui::Div {
    popover_card(theme).p(px(0.0))
}
