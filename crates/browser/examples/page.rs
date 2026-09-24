//! One window: a bar that drives the page and shows what it reports, and the
//! page under it. Events and a script's answer are printed to stdout.

use browser::{LoadState, WebView, WebViewEvent};
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, SharedString, Subscription, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use std::time::Duration;

struct Example {
    page: Entity<WebView>,
    shown: bool,
    _events: Subscription,
}

impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let page = cx.new(|cx| WebView::new("https://example.com", window, cx));
        let events = cx.subscribe(&page, |_, page, event: &WebViewEvent, cx| {
            println!("{event:?}");
            if *event == WebViewEvent::Load(LoadState::Finished) {
                let links = page.read(cx).eval::<usize>(
                    "document.links.length",
                    Duration::from_secs(2),
                    cx,
                );
                cx.spawn(async move |_, _| println!("links: {:?}", links.await))
                    .detach();
            }
            cx.notify();
        });
        Self {
            page,
            shown: true,
            _events: events,
        }
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<Example>,
    press: impl Fn(&mut Example, &mut Context<Example>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_2()
        .cursor_pointer()
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| press(this, cx)))
}

impl Render for Example {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = SharedString::from(self.page.read(cx).title().to_owned());
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x202020))
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .flex()
                    .p_2()
                    .child(button("back", "Back", cx, |this, cx| {
                        this.page.update(cx, |page, _| page.back())
                    }))
                    .child(button("forward", "Forward", cx, |this, cx| {
                        this.page.update(cx, |page, _| page.forward())
                    }))
                    .child(button("reload", "Reload", cx, |this, cx| {
                        this.page.update(cx, |page, _| page.reload())
                    }))
                    .child(button(
                        "toggle",
                        if self.shown { "Hide" } else { "Show" },
                        cx,
                        |this, cx| {
                            this.shown = !this.shown;
                            cx.notify();
                        },
                    ))
                    .child(div().px_2().child(title)),
            )
            .child(
                div()
                    .flex_1()
                    .m_4()
                    .when(self.shown, |this| this.child(self.page.clone())),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(960.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
