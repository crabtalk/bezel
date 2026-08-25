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

use gpui::{
    Anchor, AnyElement, ElementId, IntoElement, Pixels, Point, SharedString, div, prelude::*, px,
};
use motion::{self as motion, AnimationExt as _, PULSE};
use theme::{Theme, hairline, ink};

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
}

impl<T> Default for Popup<T> {
    fn default() -> Self {
        Self {
            inner: None,
            pressed_while_open: false,
        }
    }
}

impl<T> Popup<T> {
    pub fn open(&mut self, value: T) {
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
    pub fn close(&mut self) {
        self.inner = None;
    }

    /// Enter the exit phase. Returns `true` when this call started it (the
    /// caller then schedules [`reap_popup`]); `false` if already closing or
    /// closed.
    pub fn begin_close(&mut self) -> bool {
        match &mut self.inner {
            Some((_, closing @ None)) => {
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
    cx: &mut gpui::Context<V>,
    popup: impl Fn(&mut V) -> &mut Popup<T> + 'static,
) {
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
            popup(view).finish_close();
            cx.notify();
        })
        .ok();
    })
    .detach();
}

// ---------------------------------------------------------------------------
// Pure reducers
// ---------------------------------------------------------------------------

/// Step the active row of a menu: wraps at both ends; `None` enters at the
/// edge matching the direction. Empty menus stay `None`.
pub fn menu_step(active: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count_i = count as isize;
    let next = match active {
        None => {
            if delta >= 0 {
                0
            } else {
                count_i - 1
            }
        }
        Some(at) => (at as isize + delta).rem_euclid(count_i),
    };
    Some(next as usize)
}

/// Match rank of a label against a query: `0` prefix match, `1` substring,
/// `None` no match. Case-insensitive; an empty query matches everything at
/// rank 1 (input order preserved).
pub fn match_rank(query: &str, label: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(1);
    }
    let label = label.to_lowercase();
    if label.starts_with(&query) {
        Some(0)
    } else if label.contains(&query) {
        Some(1)
    } else {
        None
    }
}

/// Filter + rank labels for a search query: prefix matches first, then
/// substring matches, stable within each rank. Returns indices into `labels`.
pub fn filter_indices<S: AsRef<str>>(query: &str, labels: &[S]) -> Vec<usize> {
    let mut ranked: Vec<(usize, usize)> = labels
        .iter()
        .enumerate()
        .filter_map(|(ix, label)| match_rank(query, label.as_ref()).map(|rank| (rank, ix)))
        .collect();
    ranked.sort_by_key(|&(rank, ix)| (rank, ix));
    ranked.into_iter().map(|(_, ix)| ix).collect()
}

/// The state behind a searchable list: the items, the ranked view of them, and
/// which row of that view is active. Shared by every picker — the palette, the
/// combobox — so the mapping below is written and tested once.
pub struct Filter {
    items: Vec<SharedString>,
    /// Indices into `items`, ranked by [`filter_indices`].
    filtered: Vec<usize>,
    /// Position within `filtered`, not within `items`.
    active: Option<usize>,
}

impl Filter {
    pub fn new(items: Vec<SharedString>) -> Self {
        let filtered: Vec<usize> = (0..items.len()).collect();
        let active = (!filtered.is_empty()).then_some(0);
        Self {
            items,
            filtered,
            active,
        }
    }

    pub fn items(&self) -> &[SharedString] {
        &self.items
    }

    /// The ranked view: indices into [`Self::items`], in display order.
    pub fn filtered(&self) -> &[usize] {
        &self.filtered
    }

    /// The highlighted row's position in the FILTERED view — what a renderer
    /// compares each row against.
    pub fn active(&self) -> Option<usize> {
        self.active
    }

    /// Re-rank against `query`, re-entering the list at the top: after
    /// narrowing, the best match should be one Enter away.
    pub fn refilter(&mut self, query: &str) {
        self.filtered = filter_indices(query, &self.items);
        self.active = (!self.filtered.is_empty()).then_some(0);
    }

    pub fn step(&mut self, delta: isize) {
        self.active = menu_step(self.active, self.filtered.len(), delta);
    }

    /// The item confirming right now would pick — an index into
    /// [`Self::items`], never into the filtered view. Confusing the two is the
    /// defining bug of a filtered list: it only appears once a query narrows
    /// the rows, and then every selection picks the wrong thing.
    pub fn active_item(&self) -> Option<usize> {
        self.active
            .and_then(|position| self.filtered.get(position))
            .copied()
    }
}

/// Keys the pickers care about, classified from a raw keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    /// Plain Enter — activate the highlighted row.
    Enter,
    /// Cmd/Ctrl+Enter — the "pick this folder" accelerator in the browser.
    ModEnter,
    Escape,
    Backspace,
    Other,
}

pub fn classify_key(key: &str, cmd: bool, ctrl: bool) -> MenuKey {
    match key {
        "up" => MenuKey::Up,
        "down" => MenuKey::Down,
        // Readline/emacs motion: ctrl-n/ctrl-p mirror ↓/↑ in every picker.
        // Safe to claim frame-wide — neither chord is a text-editing binding
        // in the palette keymaps, so they always bubble here unconsumed.
        "n" if ctrl => MenuKey::Down,
        "p" if ctrl => MenuKey::Up,
        "enter" if cmd || ctrl => MenuKey::ModEnter,
        "enter" => MenuKey::Enter,
        "escape" => MenuKey::Escape,
        "backspace" => MenuKey::Backspace,
        _ => MenuKey::Other,
    }
}

// ---------------------------------------------------------------------------
// Elements
// ---------------------------------------------------------------------------

/// The floating-menu surface (the reference `.glass-surface` + `menuSurface`):
/// `rounded-xl border border-white/[0.1] p-1` over the material glass tint —
/// the real recipe now that the fork paints backdrop blur: the
/// [`Theme::glass_overlay`] tint (`oklch(0.33 0 0 / 34%)` on dark) over the
/// [`crate::material::MENU_BLUR`] blur from the mount helpers below, plus the
/// same hairline + baked-in shadow. Opaque platforms keep the near-opaque
/// tone the reference composites to on the dark panels (~#161616).
/// The inner inset of a [`popover_card`], and so the amount [`menu_row`]'s
/// corners come in by. Named because two things read it: the card's padding and
/// its rows' radius. Change it and the rows follow.
pub(crate) const MENU_PAD: f32 = 4.0;

pub fn popover_card(theme: &Theme) -> gpui::Div {
    let card = div()
        .border_1()
        .border_color(hairline(0.10))
        .rounded(px(Theme::surface_radius()))
        .shadow_lg()
        .p(px(MENU_PAD))
        .overflow_hidden()
        .text_size(px(13.0))
        .text_color(theme.text);
    if theme.is_glass() {
        // Translucent tint — the backdrop blur beneath it comes from the
        // [`crate::material::material`] wrapper at the mount helpers below.
        card.bg(theme.glass_overlay())
    } else {
        card.bg(theme.surface_overlay)
    }
}

/// [`popover_card`] without the `p-1` inset — for popovers that manage their
/// own internal panes (the harness/model picker's rail + list split).
pub fn popover_card_flush(theme: &Theme) -> gpui::Div {
    popover_card(theme).p(px(0.0))
}

/// Pin a floating layer's origin to the trigger's top-left. The anchored
/// element is absolutely positioned; without explicit insets its *static*
/// position is subject to the trigger's own flex alignment (an `items_center`
/// trigger would vertically center the whole floating layer). A zero-size
/// absolutely-inset wrapper fixes the origin at the corner.
fn pinned_layer(layer: AnyElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_0()
        .child(layer)
        .into_any_element()
}

/// Eased exit progress (0..=1) for a [`Popup`] closing instant, computed from
/// the wall clock at render time. Monotonic by construction — unlike the
/// animation element's own clock, it can never replay from 0 mid-exit.
fn exit_progress(since: web_time::Instant) -> f32 {
    let total = motion::MENU_OUT
        .total()
        .mul_f32(motion::speed_scale())
        .as_secs_f32();
    let raw = if total <= 0.0 {
        1.0
    } else {
        (since.elapsed().as_secs_f32() / total).clamp(0.0, 1.0)
    };
    motion::MENU_OUT.progress(raw)
}

/// The material card for a popover layer: full blur while open; while exiting
/// the blur radius rides the exit progress down to 0 — the `BackdropBlur`
/// primitive ignores `element_opacity`, so without this the glass slab would
/// hold full strength through the fade and pop off at unmount.
fn material_menu(exit: Option<f32>, content: AnyElement) -> AnyElement {
    let blur = crate::material::MENU_BLUR * (1.0 - exit.unwrap_or(0.0));
    crate::material::material(Theme::surface_radius(), blur, content).into_any_element()
}

/// Entrance or exit motion for a popover layer. While exiting (the [`Popup`]
/// closing phase, `exit = Some(progress)`) the content plays
/// [`motion::menu_out`] under a fresh animation id (same-id reuse would
/// inherit the entrance's finished clock and snap to the end state) and gets
/// an occluding overlay on top — the dying menu's rows must not take clicks,
/// and the overlay also keeps stray clicks from reaching whatever sits
/// underneath.
fn menu_motion(id: SharedString, exit: Option<f32>, inner: gpui::Div) -> AnyElement {
    if let Some(t) = exit {
        let inner = inner.relative().child(div().absolute().inset_0().occlude());
        motion::menu_out(SharedString::from(format!("{id}-out")), t, inner).into_any_element()
    } else {
        motion::menu_in(id, inner).into_any_element()
    }
}

/// Wrap popover content in a floating anchored layer attached to the trigger:
/// the caller `.child(anchored_menu(...))`s this from the trigger element while
/// open. Plays `menu-in` (0.14s fade + 2px drop); `closing` (the [`Popup`]
/// exit phase) swaps in `menu-out`. Dismissal is the caller's
/// `.on_mouse_down_out` on the content. The layer `.occlude()`s: hitboxes are
/// paint-order only in gpui, so without it clicks on menu rows would ALSO fire
/// whatever clickable sits under the floating layer.
pub fn anchored_menu(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    let exit = closing.map(exit_progress);
    let content = material_menu(exit, content);
    pinned_layer(
        gpui::deferred(
            gpui::anchored()
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.0))
                .child(menu_motion(
                    id.into(),
                    exit,
                    div().occlude().pt(px(6.0)).child(content),
                )),
        )
        .priority(1)
        .into_any_element(),
    )
}

/// [`anchored_menu`] opening DOWNWARD from the trigger's bottom edge — a
/// dropdown proper (the sidebar's space filter). The default variant pins to
/// the trigger's top-left, which reads fine for context-style menus but
/// covers a button-shaped trigger.
pub fn anchored_menu_below(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    anchored_menu_below_gap(id, content, closing, 6.0)
}

/// [`anchored_menu_below`] with a caller-chosen trigger→card gap — the
/// changes-header dropdowns hang off a tight titlebar band and need more
/// breathing room than the default 6px (user report; t3code sits near 10).
pub fn anchored_menu_below_gap(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
    gap: f32,
) -> AnyElement {
    let exit = closing.map(exit_progress);
    let content = material_menu(exit, content);
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .size_0()
        .child(
            gpui::deferred(
                gpui::anchored()
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.0))
                    .child(menu_motion(
                        id.into(),
                        exit,
                        div().occlude().pt(px(gap)).child(content),
                    )),
            )
            .priority(1)
            .into_any_element(),
        )
        .into_any_element()
}

/// [`anchored_menu`] opening UPWARD from the trigger (composer pickers, the
/// user menu — anything anchored near the window bottom; Radix flips these
/// automatically, gpui's `anchored` needs the side picked).
pub fn anchored_menu_above(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    let exit = closing.map(exit_progress);
    let content = material_menu(exit, content);
    pinned_layer(
        gpui::deferred(
            gpui::anchored()
                .anchor(Anchor::BottomLeft)
                .snap_to_window_with_margin(px(8.0))
                .child(menu_motion(
                    id.into(),
                    exit,
                    div().occlude().pb(px(6.0)).child(content),
                )),
        )
        .priority(1)
        .into_any_element(),
    )
}

/// Open an upward menu at a point inside a relative trigger. Useful for text
/// completions, whose natural anchor is the token/caret rather than the input
/// element's outer edge.
pub fn anchored_menu_above_at(
    id: impl Into<SharedString>,
    position: Point<Pixels>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    div()
        .absolute()
        .left(position.x)
        .top(position.y)
        .size_0()
        .child(anchored_menu_above(id, content, closing))
        .into_any_element()
}

/// [`anchored_menu_above`] right-aligned to the trigger's right edge (t3code
/// ComboboxPopup `align="end"` — right-side triggers like the composer's ref
/// picker open leftward instead of running off the window).
pub fn anchored_menu_above_end(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    let exit = closing.map(exit_progress);
    let content = material_menu(exit, content);
    div()
        .absolute()
        .top_0()
        .right_0()
        .size_0()
        .child(
            gpui::deferred(
                gpui::anchored()
                    .anchor(Anchor::BottomRight)
                    .snap_to_window_with_margin(px(8.0))
                    .child(menu_motion(
                        id.into(),
                        exit,
                        div().occlude().pb(px(6.0)).child(content),
                    )),
            )
            .priority(1)
            .into_any_element(),
        )
        .into_any_element()
}

/// A floating menu at an explicit window position (context menus). Occludes
/// like [`anchored_menu`] so row clicks never reach elements underneath.
pub fn menu_at(
    id: impl Into<SharedString>,
    position: Point<Pixels>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement {
    let exit = closing.map(exit_progress);
    let content = material_menu(exit, content);
    gpui::deferred(
        gpui::anchored()
            .position(position)
            .anchor(Anchor::TopLeft)
            .snap_to_window_with_margin(px(8.0))
            .child(menu_motion(id.into(), exit, div().occlude().child(content))),
    )
    .priority(1)
    .into_any_element()
}

/// Modal/overlay scrim at the *current* appearance, quoted in dark-mode terms
/// like [`ink`]/[`hairline`] — for callers (`modal`, the attachment lightbox)
/// that paint from a `deferred`/`anchored` layer with no `Theme`/`cx` in
/// scope. Mirrors [`Theme::scrim`], which is pinned at `X = 0.6` dark /
/// `0.32` light; other dark-mode alphas scale the light side by the same
/// ratio so the *dark* result is always exactly `alpha_dark` (never routed
/// through [`Hsla::opacity`], whose `0..=1` clamp would clip a
/// larger-than-0.6 alpha before it could scale the light side).
pub(crate) fn scrim_alpha(alpha_dark: f32) -> gpui::Hsla {
    theme::scrim(alpha_dark)
}

/// Full-window modal: dim scrim + centered card with the `dialog-in` entrance.
/// The scrim swallows clicks; the caller wires its own dismiss/confirm.
/// `viewport` is the window size (an `anchored` layer sizes to its children,
/// so the scrim needs explicit dimensions). The frost radius matches
/// [`dialog_card`]'s 16px rounding.
/// `on_dismiss` is the scrim press. It is a parameter rather than the caller's
/// `.on_mouse_down_out`, for the same reason [`sheet`]'s is: the scrim lives
/// inside this deferred layer, so nothing outside can reach it. Without it a
/// dialog could not be dismissed by clicking away from it *by any caller* —
/// which is how this one shipped, and what it looked like was a dialog that
/// only closed on its own buttons.
pub fn modal(
    id: impl Into<ElementId>,
    viewport: gpui::Size<Pixels>,
    card: AnyElement,
    on_dismiss: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    modal_with(id, viewport, card, DIALOG_RADIUS, 0.6, on_dismiss)
}

/// [`modal`] for glass-tinted cards (the add-space palette): a LIGHTER scrim,
/// so the material card reads like the popovers — the standard 0.6 dim buried
/// the backdrop hue under the blur and the palette came out a flat grey slab
/// next to the hue-inheriting menus (user report).
///
/// The radius is [`Theme::surface_radius`], not a parameter: a glass-tinted
/// modal *is* a popover surface, and the parameter this used to take carried
/// the doc line "must match the card's rounding" — a footgun handed to the
/// caller in writing.
pub fn modal_glass(
    id: impl Into<ElementId>,
    viewport: gpui::Size<Pixels>,
    card: AnyElement,
    on_dismiss: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    modal_with(
        id,
        viewport,
        card,
        Theme::surface_radius(),
        0.35,
        on_dismiss,
    )
}

fn modal_with(
    id: impl Into<ElementId>,
    viewport: gpui::Size<Pixels>,
    card: AnyElement,
    corner_radius: f32,
    scrim: f32,
    on_dismiss: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let card = crate::material::material(corner_radius, crate::material::MENU_BLUR, card)
        .into_any_element();
    gpui::deferred(
        gpui::anchored()
            .position(gpui::point(px(0.0), px(0.0)))
            .child(
                div()
                    .occlude()
                    .w(viewport.width)
                    .h(viewport.height)
                    .bg(scrim_alpha(scrim))
                    .flex()
                    .items_center()
                    .justify_center()
                    // On the card's wrapper, not the scrim: a press inside the
                    // card is not "out", so the dialog's own buttons keep
                    // working with no occluding overlay and no propagation
                    // games. The scrim covers the viewport and occludes, so
                    // "outside the card" and "on the scrim" are the same press.
                    .child(motion::dialog_in(
                        id,
                        div().child(card).on_mouse_down_out(on_dismiss),
                    )),
            ),
    )
    .priority(2)
    .into_any_element()
}

// ---------------------------------------------------------------------------
// Sheet — a dialog pinned to an edge
// ---------------------------------------------------------------------------

/// Which edge a [`sheet`] slides in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Corner rounding of [`dialog_card`], and of a [`sheet_panel`]'s two inner
/// corners (the two on the window edge are off-screen). One number rather than
/// two that happen to match: a sheet *is* the dialog card, pinned to an edge
/// instead of centred. Read three times over — the card, the sheet panel, and
/// the blur under each — which is exactly why it is not a literal.
const DIALOG_RADIUS: f32 = 16.0;

/// The full-height panel body of a [`sheet`]: glass card chrome rounded and
/// hairlined on its *inner* edge only, so it reads as pulled out of the window
/// side rather than floating near it.
pub fn sheet_panel(theme: &Theme, side: Side) -> gpui::Div {
    let card = div()
        .size_full()
        .flex()
        .flex_col()
        .shadow_lg()
        .text_color(theme.text);
    let card = match side {
        Side::Left => card
            .rounded_r(px(DIALOG_RADIUS))
            .border_r_1()
            .border_color(hairline(0.10)),
        Side::Right => card
            .rounded_l(px(DIALOG_RADIUS))
            .border_l_1()
            .border_color(hairline(0.10)),
    };
    if theme.is_glass() {
        card.bg(theme.glass_overlay())
    } else {
        card.bg(theme.surface_overlay)
    }
}

/// Full-height side panel over a dim scrim — [`modal`] pinned to an edge. It
/// slides in over [`motion::DIALOG_IN`] and, once the caller's [`Popup`]
/// enters its exit phase, back out over [`motion::MENU_OUT`] — which it must,
/// because [`Popup::finish_close`] reaps on that spec's span.
///
/// `on_dismiss` is the scrim click. Unlike the anchored menus, dismissal
/// cannot be the caller's `.on_mouse_down_out`: the scrim lives inside this
/// deferred layer, so nothing outside can reach it.
///
/// The slide is written here rather than as a `motion` helper because
/// only the *spec* is motion — which inset carries it is layout, and it
/// differs per side.
pub fn sheet(
    id: impl Into<SharedString>,
    viewport: gpui::Size<Pixels>,
    side: Side,
    width: Pixels,
    content: AnyElement,
    closing: Option<web_time::Instant>,
    on_dismiss: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let id = id.into();
    let exit = closing.map(exit_progress);
    let blur = crate::material::MENU_BLUR * (1.0 - exit.unwrap_or(0.0));

    let panel = div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(width)
        .child(crate::material::material(DIALOG_RADIUS, blur, content));
    // `t` runs 0 (fully off-screen) → 1 (seated against the edge).
    let seat = move |el: gpui::Div, t: f32| {
        let inset = width * (t - 1.0);
        match side {
            Side::Left => el.left(inset),
            Side::Right => el.right(inset),
        }
    };
    let panel = if let Some(t) = exit {
        // The dying panel must not take clicks — same overlay `menu_motion`
        // puts over an exiting menu.
        let panel = seat(panel, 1.0 - t).child(div().absolute().inset_0().occlude());
        panel
            .with_animation(
                SharedString::from(format!("{id}-out")),
                motion::MENU_OUT.animation(),
                move |el, _| el,
            )
            .into_any_element()
    } else {
        panel
            .with_animation(id.clone(), motion::DIALOG_IN.animation(), seat)
            .into_any_element()
    };

    gpui::deferred(
        gpui::anchored()
            .position(gpui::point(px(0.0), px(0.0)))
            .child(
                div()
                    .id(SharedString::from(format!("{id}-scrim")))
                    .occlude()
                    .relative()
                    .w(viewport.width)
                    .h(viewport.height)
                    .bg(scrim_alpha(0.6 * (1.0 - exit.unwrap_or(0.0))))
                    .on_click(on_dismiss)
                    .child(panel),
            ),
    )
    .priority(2)
    .into_any_element()
}

/// One menu row (the reference `menuItem`): `gap-2.5 rounded-lg px-2 py-1.5
/// text-[13px]`, active = `bg-white/10 text-foreground`, hover wash
/// `white/[0.08]` fading over `transition-colors` (floating-styles.ts) via the
/// per-`fade_key` [`motion::hover_blend`]. The caller adds the id/click
/// listener — `fade_key` must be unique app-wide and stable across frames
/// (the id string is a good choice).
pub fn menu_row(theme: &Theme, active: bool, fade_key: impl Into<SharedString>) -> gpui::Div {
    let row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .px(px(8.0))
        .py(px(6.0))
        // Concentric with the card it sits in rather than a radius of its own:
        // 12 − 4 = 8, which is where the crate's most-repeated corner value
        // came from all along.
        .rounded(px(Theme::inset_radius(Theme::surface_radius(), MENU_PAD)))
        .text_size(px(13.0))
        .cursor_pointer();
    if active {
        row.bg(theme::card_selected_bg()).text_color(theme.text)
    } else {
        let fade_key = fade_key.into();
        let mut row = row
            .text_color(motion::hover_blend(
                &fade_key,
                theme.text.opacity(0.9),
                theme.text,
            ))
            .bg(motion::hover_blend(
                &fade_key,
                theme::wash(0.0),
                theme::card_selected_bg(),
            ));
        // Imperative form — the caller's `.id(...)` makes the element stateful
        // (hover listeners need element state, `.on_hover` needs `Stateful`).
        row.interactivity()
            .on_hover(motion::hover_listener(fade_key));
        row
    }
}

/// [`menu_row`] with a distinct keyboard-navigation highlight: a selected row
/// carries the full `bg-white/10` wash, the keyboard cursor the lighter
/// `bg-white/[0.08]` (the reference's `data-[highlighted]` styling) — two selected-
/// looking rows never appear at once.
pub fn menu_row_nav(
    theme: &Theme,
    selected: bool,
    highlighted: bool,
    fade_key: impl Into<SharedString>,
) -> gpui::Div {
    let row = menu_row(theme, selected, fade_key);
    if !selected && highlighted {
        row.bg(theme::card_selected_bg()).text_color(theme.text)
    } else {
        row
    }
}

/// Small uppercase section heading inside a floating menu (the reference
/// `MenuHeading`): `px-2 pb-1 pt-1.5 text-[10px] font-medium uppercase
/// tracking-[0.1em] text-muted-foreground/60`. gpui has no letter-spacing at
/// the pinned rev; the tracking is approximated with hair spaces.
pub fn menu_heading(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    let label = label.into();
    div()
        .px(px(8.0))
        .pb(px(4.0))
        .pt(px(6.0))
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.text_muted.opacity(0.6))
        .child(tracked_upper(&label))
}

/// Uppercase + hair-space tracking (see [`menu_heading`]).
pub fn tracked_upper(label: &str) -> String {
    let upper = label.to_uppercase();
    let mut out = String::with_capacity(upper.len() * 2);
    let mut first = true;
    for ch in upper.chars() {
        if !first {
            out.push('\u{200A}'); // hair space ≈ 0.1em tracking
        }
        out.push(ch);
        first = false;
    }
    out
}

/// Hairline divider between menu sections (the reference `MenuSeparator`:
/// `mx-1 my-1 h-px bg-white/[0.07]`).
pub fn divider() -> gpui::Div {
    // Full-bleed: negative margins cancel the card's p-1 inset so the hairline
    // runs border to border (user request).
    div().h(px(1.0)).mx(px(-4.0)).my(px(4.0)).bg(hairline(0.07))
}

/// The recessed band tone for a palette/picker header or footer strip — a
/// translucent black so the glass still reads through (the add-space palette
/// converged on this; measured subtler tones vanish against the dim scrim).
/// Free function (like [`ink`]/[`hairline`]/[`wash`]), mirroring
/// [`Theme::band`], for the several callers with no `Theme`/`cx` in scope
/// (some outside this crate's `ui` module tree — threading a `&Theme` param
/// would ripple past this task's file scope).
pub fn band() -> gpui::Hsla {
    theme::band()
}

/// One footer key-cap (22px, rounded-5, `white/[0.05]`) holding arbitrary
/// children — the base of [`key_hint`]/[`key_hint_pair`] and the search-bar
/// chips ("⌘K", "esc").
pub fn key_cap(_theme: &Theme) -> gpui::Div {
    div()
        .h(px(22.0))
        .px(px(5.0))
        .rounded(px(5.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .bg(ink(0.05))
}

/// The tiny verb after a key-cap.
fn key_hint_label(theme: &Theme, label: &'static str) -> gpui::Div {
    div()
        .text_size(px(10.5))
        .text_color(theme.text_muted.opacity(0.45))
        .child(SharedString::from(label))
}

/// A footer legend: one icon key-cap + tiny verb (the add-space palette's
/// footer voice, shared by the pickers).
pub fn key_hint(theme: &Theme, icon_path: &'static str, label: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .child(
            key_cap(theme).child(
                crate::icons::icon(icon_path)
                    .size(px(12.5))
                    .text_color(theme.text_muted.opacity(0.7)),
            ),
        )
        .child(key_hint_label(theme, label))
}

/// A footer legend whose cap holds a WORD ("tab", "esc") instead of a glyph
/// — for keys with no icon in the set.
pub fn key_hint_text(theme: &Theme, cap: &'static str, label: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .child(
            key_cap(theme)
                .text_size(px(11.0))
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_muted.opacity(0.7))
                .child(SharedString::from(cap)),
        )
        .child(key_hint_label(theme, label))
}

/// A footer legend whose cap holds TWO glyphs split by a hairline
/// ("[ ↑ | ↓ ] Navigate") sharing one verb.
pub fn key_hint_pair(
    theme: &Theme,
    first: &'static str,
    second: &'static str,
    label: &'static str,
) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .child(
            key_cap(theme)
                .child(
                    crate::icons::icon(first)
                        .size(px(12.5))
                        .text_color(theme.text_muted.opacity(0.7)),
                )
                .child(div().w(px(1.0)).h(px(11.0)).bg(hairline(0.10)))
                .child(
                    crate::icons::icon(second)
                        .size(px(12.5))
                        .text_color(theme.text_muted.opacity(0.7)),
                ),
        )
        .child(key_hint_label(theme, label))
}

/// A muted kbd hint chip inside menu rows (`⌘↵`-style accelerators).
pub fn kbd_hint(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex_none()
        .px(px(5.0))
        .py(px(1.0))
        .rounded(px(5.0))
        .bg(ink(0.05))
        .text_size(px(10.0))
        .font_family(theme.font_mono.clone())
        .text_color(theme.text_muted.opacity(0.6))
        .child(label.into())
}

/// The search/text input frame at the top of a picker popover (the reference
/// `searchInput`: `w-full rounded-lg bg-white/[0.04] px-2.5 py-1.5
/// text-[13px]` + `mb-1`, borderless — full width inside the card's own
/// p-1, only a 4px bottom margin).
pub fn search_input_frame(_theme: &Theme, input: AnyElement) -> gpui::Div {
    div()
        .mb(px(MENU_PAD))
        .px(px(10.0))
        .py(px(6.0))
        // Concentric with the card, like [`menu_row`] — this frame sits on the
        // same inset, and its own doc already said so.
        .rounded(px(Theme::inset_radius(Theme::surface_radius(), MENU_PAD)))
        .bg(ink(0.04))
        .text_size(px(13.0))
        .child(input)
}

/// A bordered trailing menu section (the reference picker action groups /
/// branch-picker worktree block: `mt-1 flex flex-col gap-0.5 border-t
/// border-white/[0.06] pt-1` — the hairline runs edge-to-edge of the card's
/// p-1 inset, unlike [`divider`]'s mx-1).
pub fn menu_section() -> gpui::Div {
    div()
        .mt(px(4.0))
        .pt(px(4.0))
        .border_t_1()
        .border_color(hairline(0.06))
        .flex()
        .flex_col()
        .gap(px(2.0))
}

// ---------------------------------------------------------------------------
// Dialog primitives (the reference dialog.tsx / sidebar dialogs.tsx)
// ---------------------------------------------------------------------------

/// The centered dialog card (`dialog-pop`): `w-[360px] rounded-2xl border
/// border-white/[0.1] bg-popover/95 p-5 shadow-2xl` — popover tone ≈ #101010.
pub fn dialog_card(theme: &Theme) -> gpui::Div {
    div()
        .w(px(360.0))
        .p(px(20.0))
        .rounded(px(DIALOG_RADIUS))
        .bg(theme.surface_dialog)
        .border_1()
        .border_color(hairline(0.10))
        .shadow_lg()
        .flex()
        .flex_col()
        .text_color(theme.text)
}

/// Dialog title: `text-[15px] font-semibold tracking-tight`.
pub fn dialog_title(theme: &Theme, title: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(15.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.text)
        .child(title.into())
}

/// Dialog body copy: `text-[13px] leading-relaxed text-muted-foreground`.
pub fn dialog_body(theme: &Theme, copy: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(13.0))
        .line_height(px(19.0))
        .text_color(theme.text_muted)
        .child(copy.into())
}

/// Dialog text-field frame: `rounded-lg border border-white/[0.08]
/// bg-white/[0.04] px-3 py-2 text-[14px]`.
pub fn dialog_field(input: AnyElement) -> gpui::Div {
    div()
        .w_full()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(Theme::button_radius()))
        .border_1()
        .border_color(hairline(0.08))
        .bg(ink(0.04))
        .text_size(px(14.0))
        .child(input)
}

/// Pulsing skeleton rows shown while a list loads (the reference:
/// `h-7 animate-pulse rounded-md bg-white/[0.04]`).
pub fn redacted_rows(
    _id: &'static str,
    _theme: &Theme,
    count: usize,
    view: gpui::EntityId,
    cx: &mut gpui::App,
) -> AnyElement {
    let wash = ink(0.04);
    let delta = motion::pulse_delta(&PULSE, view, cx);
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .py(px(4.0))
        .children((0..count).map(move |i| {
            let phase = motion::staggered_phase(delta, i, 0.08);
            div()
                .h(px(28.0))
                .rounded(px(Theme::control_radius()))
                .bg(wash)
                .opacity(0.35 + 0.4 * motion::pulse_wave(phase))
        }))
        .into_any_element()
}

/// Inline error row + Retry affordance (the caller attaches the listener to the
/// returned id).
pub fn error_row(theme: &Theme, message: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .p(px(Theme::SPACE_SM))
        .text_size(px(12.0))
        .text_color(theme.danger)
        .child(message.into())
}
