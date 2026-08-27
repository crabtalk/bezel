//! Keyboard focus traversal — `tab` and `shift-tab` between controls.
//!
//! gpui has all the machinery and none of it is on by default: a focus handle
//! carries a `tab_index` and a `tab_stop` flag, `.track_focus` registers the
//! handle for the frame, and [`Window::focus_next`] walks the order — but
//! `tab_stop` starts `false` and gpui binds no keys. This module turns it on.
//!
//! # Order is paint order
//!
//! gpui sorts tab stops by their `tab_index` path and then by insertion, so
//! leaving every index at 0 yields the order the controls are painted in.
//! Nothing has to be numbered by hand, and inserting a control in the middle of
//! a form does not renumber the rest — which is the failure mode that makes
//! HTML `tabindex` a liability.
//!
//! # Where the handle lives
//!
//! Most of this crate is `fn(&Theme, ..) -> Div`: stateless, with the app
//! owning whether a checkbox is checked. Focus is more of that state, so the
//! app owns the handle too and [`focusable`] wires it up. Giving every widget a
//! handle of its own would mean giving every widget an identity and a lifetime,
//! which is the entity machinery [`crate::input::TextField`] needs and a
//! checkbox does not.
//!
//! ```ignore
//! ui::focus::init(cx);                    // once, at startup
//!
//! // ..and on the root view, so `tab` works wherever focus currently is:
//! focus::traversal(div().track_focus(&self.focus_handle))
//!     .child(focus::focusable(&theme, &self.ok_focus, popover::button(&theme, "OK", "ok")))
//! ```

use gpui::{App, Div, FocusHandle, KeyBinding, Window, actions, prelude::*};

use theme::Theme;

actions!(
    bezel_focus,
    [FocusNext, FocusPrev, Activate, Decrement, Increment]
);

/// Claimed by every [`focusable`] control, so `enter` and `space` mean "press
/// this" only where something is actually focused.
///
/// Scoping matters here: [`crate::palette`] and [`crate::combobox`] both bind
/// `enter` for their own lists, and a multi-line field binds it to insert a
/// newline. A focused control sits deeper in the focus path than any of them,
/// so it wins `enter` while focused and gives it straight back afterwards.
pub const CONTROL_KEY_CONTEXT: &str = "Control";

/// Bind `tab` and `shift-tab`. Call once at startup.
///
/// Optional, like [`crate::input::init`] — the actions are public, so an app
/// that wants different keys binds those instead. The bindings are global
/// rather than scoped to a context: traversal is a property of the window, not
/// of whatever happens to be focused.
///
/// Nothing in this crate claims `tab` for itself, deliberately. A multi-line
/// field could reasonably insert one, but trapping `tab` inside a text box is
/// the classic way to make a form impossible to leave by keyboard.
///
/// [`Decrement`]/[`Increment`] on `left`/`right` are for a control that holds a
/// *value* rather than a press — [`slider`](crate::widgets::Controls::slider)
/// is the one. They
/// carry no step: only the caller knows the range, and a library that picked
/// one would be picking it for a percentage and a font size alike.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrev, None),
        // Both, because both are standard and they disagree by platform: the
        // web and Windows press a focused button with `space`, macOS with
        // `enter`. Scoped to a focused control, neither is ambiguous.
        KeyBinding::new("enter", Activate, Some(CONTROL_KEY_CONTEXT)),
        KeyBinding::new("space", Activate, Some(CONTROL_KEY_CONTEXT)),
        KeyBinding::new("left", Decrement, Some(CONTROL_KEY_CONTEXT)),
        KeyBinding::new("right", Increment, Some(CONTROL_KEY_CONTEXT)),
    ]);
}

/// Attach the traversal handlers, normally to the app's root element.
///
/// It has to live on an element rather than on the app because moving focus
/// needs a [`Window`], and an app-level action handler only gets an [`App`].
pub fn traversal(el: Div) -> Div {
    el.on_action(|_: &FocusNext, window: &mut Window, cx: &mut App| window.focus_next(cx))
        .on_action(|_: &FocusPrev, window: &mut Window, cx: &mut App| window.focus_prev(cx))
}

/// Put a stateless control into the tab order, show when it holds focus, and
/// let `enter`/`space` press it.
///
/// The ring is the same one [`crate::input::TextField`] paints — the border in
/// [`Theme::ring`] — so a focused button and a focused field read alike.
///
/// Keyboard focus only, like CSS `:focus-visible`. A control also takes focus
/// when clicked, and a ring that landed on it there would outline a slider for
/// the whole drag — the pointer already says which control is being used.
///
/// It lands on the control's *own* border, which is why every control in
/// [`crate::widgets`] carries one even where it paints nothing: gpui sizes
/// border-box, so a border that only appeared on focus would move the content
/// under it by a pixel. A ring wrapped *around* the control instead would cost
/// every one of them a radius parameter, and would prise a focused tab off the
/// hairline its underline has to overlap.
///
/// Pressing dispatches [`Activate`], which the caller handles beside its
/// `on_click`. Deliberately not folded into one callback: a control that is
/// pressed by mouse and by key is doing the same thing, but only the caller
/// knows what that is, and a keyboard-only affordance that silently diverges
/// from the click is worse than none.
pub fn focusable(theme: &Theme, handle: &FocusHandle, el: Div) -> Div {
    // `tab_stop` writes through to the shared focus entry, so re-asserting it
    // every render is free and keeps the flag next to the element that wants
    // it, rather than at whatever distant place the handle was constructed.
    let handle = handle.clone().tab_stop(true);
    el.key_context(CONTROL_KEY_CONTEXT)
        .track_focus(&handle)
        .focus_visible(|style| style.border_color(theme.ring))
}
