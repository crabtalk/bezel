//! Tab traversal under gpui's own dispatcher: which binding answers the key,
//! which no pure function can say.

use gpui::{
    Context, FocusHandle, IntoElement, KeyBinding, KeyContext, Render, TestAppContext,
    VisualTestContext, Window, actions, div, prelude::*, px, size,
};
use ui::focus;

actions!(focus_test, [Indent]);

/// Two stops, the second optionally claiming `tab`. The root traverses.
struct Host {
    root: FocusHandle,
    first: FocusHandle,
    second: FocusHandle,
    claims_tab: bool,
    indented: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut context = KeyContext::default();
        context.add("Surface");
        if self.claims_tab {
            context.add(focus::CLAIMS_TAB);
        }
        focus::traversal(div())
            // No key context of its own, as an app's root view usually has
            // none: the stack a key dispatches against is empty here.
            .track_focus(&self.root)
            .size_full()
            .child(div().track_focus(&self.first.clone().tab_stop(true)))
            .child(
                div()
                    .key_context(context)
                    .track_focus(&self.second.clone().tab_stop(true))
                    .on_action(cx.listener(|host: &mut Host, _: &Indent, _, _| {
                        host.indented = true;
                    })),
            )
    }
}

fn open(claims_tab: bool, cx: &mut TestAppContext) -> (gpui::Entity<Host>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        // A surface's own binding, scoped to its context, bound *before*
        // traversal's — the order that used to lose the tie.
        cx.bind_keys([KeyBinding::new("tab", Indent, Some("Surface"))]);
        focus::init(cx);
    });
    let window = cx.add_window(|_, cx| Host {
        root: cx.focus_handle(),
        first: cx.focus_handle(),
        second: cx.focus_handle(),
        claims_tab,
        indented: false,
    });
    let host = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(200.0), px(200.0)));
    visual.run_until_parked();
    (host, visual)
}

/// A root view with no key context dispatches against an empty stack, which
/// every predicate fails against — so a `!ClaimsTab` binding would leave this
/// app no `tab` at all.
#[gpui::test]
fn tab_works_from_a_root_that_has_no_key_context(cx: &mut TestAppContext) {
    let (host, mut cx) = open(false, cx);
    cx.update(|window, cx| host.read(cx).root.clone().focus(window, cx));
    cx.simulate_keystrokes("tab");
    assert!(cx.update(|window, cx| host.read(cx).first.is_focused(window)));
}

#[gpui::test]
fn tab_steps_between_stops(cx: &mut TestAppContext) {
    let (host, mut cx) = open(false, cx);
    cx.update(|window, cx| host.read(cx).first.clone().focus(window, cx));
    cx.simulate_keystrokes("tab");
    assert!(
        cx.update(|window, cx| host.read(cx).second.is_focused(window)),
        "tab moves on"
    );
    cx.simulate_keystrokes("shift-tab");
    assert!(
        cx.update(|window, cx| host.read(cx).first.is_focused(window)),
        "and shift-tab back"
    );
    assert!(
        !cx.update(|_, cx| host.read(cx).indented),
        "a surface that never claimed the key was not asked to indent"
    );
}

/// The bug behind #14: both bindings match at the same depth, and the tie went
/// to traversal, bound last. Standing down hands the chord to the next one.
#[gpui::test]
fn a_surface_that_claims_tab_gets_the_key(cx: &mut TestAppContext) {
    let (host, mut cx) = open(true, cx);
    cx.update(|window, cx| host.read(cx).second.clone().focus(window, cx));
    cx.simulate_keystrokes("tab");
    assert!(
        cx.update(|_, cx| host.read(cx).indented),
        "the surface's own binding answered"
    );
    assert!(
        cx.update(|window, cx| host.read(cx).second.is_focused(window)),
        "and focus stayed put"
    );
}

/// Claiming is per surface, not per window.
#[gpui::test]
fn a_claim_only_holds_while_that_surface_is_focused(cx: &mut TestAppContext) {
    let (host, mut cx) = open(true, cx);
    cx.update(|window, cx| host.read(cx).first.clone().focus(window, cx));
    cx.simulate_keystrokes("tab");
    assert!(
        cx.update(|window, cx| host.read(cx).second.is_focused(window)),
        "tab moved into the claiming surface rather than being swallowed"
    );
    assert!(!cx.update(|_, cx| host.read(cx).indented));
}
