//! Two windows and one page. The page shows in the window whose "Take" was
//! pressed last; the other window says where it went.

use browser::WebView;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, Subscription, Window, WindowBounds,
    WindowOptions, div, point, prelude::*, px, rgb, size,
};

/// Which window shows the page.
struct Holder {
    page: Entity<WebView>,
    at: usize,
}

struct Pane {
    index: usize,
    holder: Entity<Holder>,
    _holder: Subscription,
}

impl Pane {
    fn new(index: usize, holder: Entity<Holder>, cx: &mut Context<Self>) -> Self {
        let observed = cx.observe(&holder, |_, _, cx| cx.notify());
        Self {
            index,
            holder,
            _holder: observed,
        }
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let index = self.index;
        let holder = self.holder.read(cx);
        let here = holder.at == index;
        let page = holder.page.clone();
        let at = holder.at;
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
                    .gap_2()
                    .child(format!("Window {}", index + 1))
                    .child(
                        div()
                            .id("take")
                            .px_2()
                            .cursor_pointer()
                            .child("Take")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.holder.update(cx, |holder, cx| {
                                    holder.at = index;
                                    cx.notify();
                                });
                            })),
                    ),
            )
            .child(div().flex_1().m_4().map(|pane| {
                if here {
                    pane.child(page)
                } else {
                    pane.child(format!("The page is in window {}.", at + 1))
                }
            }))
    }
}

fn open(index: usize, holder: Option<Entity<Holder>>, cx: &mut App) -> Entity<Holder> {
    let bounds = Bounds::new(
        point(px(80.0 + 720.0 * index as f32), px(120.0)),
        size(px(700.0), px(560.0)),
    );
    let mut made = None;
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| {
            let holder = holder.unwrap_or_else(|| {
                let page = cx.new(|cx| WebView::new("https://example.com", window, cx));
                cx.new(|_| Holder { page, at: index })
            });
            made = Some(holder.clone());
            cx.new(|cx| Pane::new(index, holder, cx))
        },
    )
    .unwrap();
    made.expect("the window was built")
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let holder = open(0, None, cx);
        open(1, Some(holder), cx);
        cx.activate(true);
    });
}
