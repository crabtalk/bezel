use gpui::{
    Context, FocusHandle, Modifiers, Render, TestAppContext, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};
use ui::{
    focus,
    widgets::{Button, ButtonStyle},
};

struct Host {
    root: FocusHandle,
    presses: usize,
    enabled: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus::traversal(div().track_focus(&self.root).size_full()).child(
            Button::new("save", "Save")
                .button_style(ButtonStyle::Prominent)
                .enabled(self.enabled)
                .absolute()
                .left(px(10.0))
                .top(px(10.0))
                .w(px(100.0))
                .h(px(40.0))
                .on_press(cx.listener(|view, _, _, cx| {
                    view.presses += 1;
                    cx.notify();
                })),
        )
    }
}

#[gpui::test]
fn pointer_and_keyboard_share_activation_across_renders(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        focus::init(cx);
    });
    let window = cx.add_window(|_, cx| Host {
        root: cx.focus_handle(),
        presses: 0,
        enabled: true,
    });
    let host = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(200.0), px(150.0)));
    cx.update(|window, cx| host.read(cx).root.clone().focus(window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("tab enter space");
    assert_eq!(cx.update(|_, cx| host.read(cx).presses), 2);
    cx.simulate_click(point(px(50.0), px(25.0)), Modifiers::default());
    assert_eq!(cx.update(|_, cx| host.read(cx).presses), 3);
    cx.update(|_, cx| {
        host.update(cx, |view, cx| {
            view.enabled = false;
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter space");
    cx.simulate_click(point(px(50.0), px(25.0)), Modifiers::default());
    assert_eq!(
        cx.update(|_, cx| host.read(cx).presses),
        3,
        "disabled blocks both paths"
    );
    cx.update(|_, cx| {
        host.update(cx, |view, cx| {
            view.enabled = true;
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    assert_eq!(
        cx.update(|_, cx| host.read(cx).presses),
        4,
        "focus survives rerendering"
    );
}

#[test]
fn widget_washes_follow_the_receiver_theme() {
    use ui::widgets::{Content, Controls};
    let _guard = theme::lock_appearance();
    theme::set_current_appearance(theme::Appearance::Dark);
    let light = theme::Theme::light();
    let mut avatar = light.avatar("中");
    let mut toggle = light.toggle(false);
    assert_eq!(avatar.style().background, Some(light.ink(0.12).into()));
    assert_eq!(toggle.style().background, Some(light.ink(0.15).into()));
}
