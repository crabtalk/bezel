//! What a surface prints for a chord, against the keymap that would fire it.
//!
//! Every one of these opens a window. `shortcut` and `shortcut_in` ask gpui's
//! dispatch tree what is bound *here*, which needs a real focus path — the
//! same reason the tab tests open one — and the formatting is checked through
//! them rather than beside them, since a label nothing resolved is a label no
//! caller will ever see.
//!
//! The claim under all of it is that rebinding moves the label. So the
//! interesting tests are not "⌘B prints ⌘B" but the three ways an app changes
//! the keymap out from under a printed hint: binding over a default, unbinding
//! it, and binding it somewhere the hint cannot see.

use gpui::{
    Context, FocusHandle, IntoElement, KeyBinding, KeyContext, NoAction, Render, TestAppContext,
    VisualTestContext, Window, actions, div, prelude::*, px, size,
};
use ui::{keys, menu::Item};

actions!(keys_test, [Bold, Italic, Unbound]);

/// The context the bindings under test are scoped to.
const SURFACE: &str = "Surface";

/// One focusable surface claiming [`SURFACE`], so a scoped binding has a
/// context stack to match against.
struct Host {
    surface: FocusHandle,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut context = KeyContext::default();
        context.add(SURFACE);
        div().size_full().child(
            div()
                .key_context(context)
                .track_focus(&self.surface)
                .on_action(cx.listener(|_: &mut Host, _: &Bold, _, _| {})),
        )
    }
}

/// Open a window with `bindings` installed and focus inside [`SURFACE`].
fn open(bindings: Vec<KeyBinding>, cx: &mut TestAppContext) -> VisualTestContext {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        cx.bind_keys(bindings);
    });
    let window = cx.add_window(|_, cx| Host {
        surface: cx.focus_handle(),
    });
    let host = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(200.0), px(200.0)));
    let surface = host.read_with(&visual, |host, _| host.surface.clone());
    visual.update(|window, cx| surface.focus(window, cx));
    visual.run_until_parked();
    visual
}

/// Apple's order is control, option, shift, command — not the order gpui's
/// `Modifiers` declares them in, and not the order gpui's own `Display`
/// prints them in, which is why this formatter exists at all.
#[gpui::test]
fn modifiers_are_in_the_platform_order(cx: &mut TestAppContext) {
    let mut visual = open(
        vec![KeyBinding::new("ctrl-alt-shift-cmd-b", Bold, Some(SURFACE))],
        cx,
    );
    let label = visual
        .update(|window, _| keys::shortcut(&Bold, window))
        .unwrap();
    #[cfg(target_os = "macos")]
    assert_eq!(label.as_ref(), "⌃⌥⇧⌘B");
    #[cfg(not(target_os = "macos"))]
    assert_eq!(label.as_ref(), "Super+Ctrl+Alt+Shift+B");
}

/// A named key is a glyph where the platform has one, and the modifiers run
/// straight into it with nothing between.
#[gpui::test]
fn named_keys_print_as_the_platform_writes_them(cx: &mut TestAppContext) {
    let mut visual = open(
        vec![
            KeyBinding::new("cmd-backspace", Bold, Some(SURFACE)),
            KeyBinding::new("shift-enter", Italic, Some(SURFACE)),
        ],
        cx,
    );
    let (bold, italic) = visual.update(|window, _| {
        (
            keys::shortcut(&Bold, window).unwrap(),
            keys::shortcut(&Italic, window).unwrap(),
        )
    });
    #[cfg(target_os = "macos")]
    {
        assert_eq!(bold.as_ref(), "⌘⌫");
        assert_eq!(italic.as_ref(), "⇧↩");
    }
    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(bold.as_ref(), "Super+Backspace");
        assert_eq!(italic.as_ref(), "Shift+Enter");
    }
}

/// A sequence is its keystrokes with a space between, the way both platforms
/// print one.
#[gpui::test]
fn a_chord_prints_as_two_keystrokes(cx: &mut TestAppContext) {
    let mut visual = open(
        vec![KeyBinding::new("ctrl-k ctrl-b", Bold, Some(SURFACE))],
        cx,
    );
    let label = visual
        .update(|window, _| keys::shortcut(&Bold, window))
        .unwrap();
    #[cfg(target_os = "macos")]
    assert_eq!(label.as_ref(), "⌃K ⌃B");
    #[cfg(not(target_os = "macos"))]
    assert_eq!(label.as_ref(), "Ctrl+K Ctrl+B");
}

/// Nothing bound prints nothing. The caller's fallback is a row with no
/// accelerator, never a chord it made up.
#[gpui::test]
fn an_unbound_action_has_no_label(cx: &mut TestAppContext) {
    let mut visual = open(vec![KeyBinding::new("cmd-b", Bold, Some(SURFACE))], cx);
    assert_eq!(
        visual.update(|window, _| keys::shortcut(&Unbound, window)),
        None
    );
}

/// A binding with no context at all is an app's own global chord — the palette
/// opener is one — and it resolves from inside a context, not only outside
/// every one.
#[gpui::test]
fn a_global_binding_resolves_from_inside_a_context(cx: &mut TestAppContext) {
    let mut visual = open(vec![KeyBinding::new("cmd-k", Bold, None)], cx);
    let label = visual
        .update(|window, _| keys::shortcut(&Bold, window))
        .unwrap();
    #[cfg(target_os = "macos")]
    assert_eq!(label.as_ref(), "⌘K");
    #[cfg(not(target_os = "macos"))]
    assert_eq!(label.as_ref(), "Super+K");
}

/// The whole point: an app that binds its own chord over a default gets its
/// own chord printed, without touching the surface that prints it. A later
/// binding wins in gpui's dispatch, and the label has to agree.
#[gpui::test]
fn rebinding_moves_the_label(cx: &mut TestAppContext) {
    let mut visual = open(
        vec![
            KeyBinding::new("cmd-b", Bold, Some(SURFACE)),
            // The app's, installed after the library's.
            KeyBinding::new("cmd-shift-b", Bold, Some(SURFACE)),
        ],
        cx,
    );
    let label = visual
        .update(|window, _| keys::shortcut(&Bold, window))
        .unwrap();
    #[cfg(target_os = "macos")]
    assert_eq!(label.as_ref(), "⇧⌘B");
    #[cfg(not(target_os = "macos"))]
    assert_eq!(label.as_ref(), "Super+Shift+B");
}

/// The other half of customizing: `NoAction` over a chord takes it away, and
/// the hint goes with it rather than outliving it.
#[gpui::test]
fn unbinding_takes_the_label_away(cx: &mut TestAppContext) {
    let mut visual = open(
        vec![
            KeyBinding::new("cmd-b", Bold, Some(SURFACE)),
            KeyBinding::new("cmd-b", NoAction, Some(SURFACE)),
        ],
        cx,
    );
    assert_eq!(
        visual.update(|window, _| keys::shortcut(&Bold, window)),
        None
    );
}

/// `shortcut` answers for the focus path, so a chord scoped to a context
/// nothing is focused inside is invisible to it — which is the case every menu
/// row is in, since opening the menu took focus off the surface. `shortcut_in`
/// is the question to ask there.
#[gpui::test]
fn a_context_out_of_the_focus_path_needs_shortcut_in(cx: &mut TestAppContext) {
    let mut visual = open(vec![KeyBinding::new("cmd-b", Bold, Some("Elsewhere"))], cx);
    assert_eq!(
        visual.update(|window, _| keys::shortcut(&Bold, window)),
        None
    );
    let label = visual
        .update(|window, _| keys::shortcut_in(&Bold, "Elsewhere", window))
        .unwrap();
    #[cfg(target_os = "macos")]
    assert_eq!(label.as_ref(), "⌘B");
    #[cfg(not(target_os = "macos"))]
    assert_eq!(label.as_ref(), "Super+B");
}

/// The menu row reads the same answer, into the slot a hand-typed accelerator
/// used to fill — so the two builders are interchangeable and only one of them
/// can go stale.
#[gpui::test]
fn a_menu_row_fills_its_accelerator_from_the_keymap(cx: &mut TestAppContext) {
    let mut visual = open(vec![KeyBinding::new("cmd-b", Bold, Some("Elsewhere"))], cx);
    let (bound, unbound, printed) = visual.update(|window, _| {
        (
            Item::action("Bold").with_shortcut_in(&Bold, "Elsewhere", window),
            Item::action("Italic").with_shortcut_in(&Italic, "Elsewhere", window),
            keys::shortcut_in(&Bold, "Elsewhere", window).unwrap(),
        )
    });
    assert_eq!(bound, Item::action("Bold").with_keystroke(printed));
    // Nothing bound leaves the row bare rather than inventing a chord.
    assert_eq!(unbound, Item::action("Italic"));
}
