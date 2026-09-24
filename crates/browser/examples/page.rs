//! One window: a bar that shows and hides the page, and the page under it.

use browser::WebView;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

struct Example {
    page: Entity<WebView>,
    shown: bool,
}

impl Render for Example {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x202020))
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .id("toggle")
                    .p_2()
                    .cursor_pointer()
                    .child(if self.shown { "Hide page" } else { "Show page" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shown = !this.shown;
                        cx.notify();
                    })),
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
            |window, cx| {
                cx.new(|cx| Example {
                    page: cx.new(|cx| WebView::new("https://example.com", window, cx)),
                    shown: true,
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
