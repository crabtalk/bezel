//! A browser pane: an address field over a [`WebView`]. The field follows
//! wherever the page goes, unless you are typing in it.
//!
//! Native only; the page is a WKWebView on macOS, WebView2 on Windows, and
//! nothing on Linux.

use browser::{WebView, WebViewEvent};
use gpui::{
    App, Context, Entity, Focusable, KeyBinding, Render, SharedString, Subscription, Window,
    actions, div, prelude::*, px,
};
use motion::{Fade, Painter};
use theme::Theme;
use ui::{
    icons,
    input::TextField,
    widgets::{ButtonStyle, Buttons},
};

actions!(gallery_browser, [Go]);

/// Claimed by the address field on top of `TextField`, so `enter` loads here.
const ADDRESS_CONTEXT: &str = "GalleryAddress";

const HOME: &str = "https://example.com";

/// Bind the address field's keys. Called once at startup beside `input::init`.
pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("enter", Go, Some(ADDRESS_CONTEXT))]);
}

pub struct Browser {
    address: Entity<TextField>,
    /// Built on the first render: a `WebView` is made in a window.
    page: Option<(Entity<WebView>, Subscription)>,
}

impl Browser {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let address = cx.new(|cx| {
            let mut field = TextField::new(cx)
                .with_key_context(ADDRESS_CONTEXT)
                .with_placeholder("Address");
            field.set_content(HOME, cx);
            field
        });
        Self {
            address,
            page: None,
        }
    }

    fn page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<WebView> {
        if let Some((page, _)) = &self.page {
            return page.clone();
        }
        let page = cx.new(|cx| WebView::new(HOME, window, cx));
        let events = cx.subscribe_in(&page, window, |this, _, event, window, cx| {
            if let WebViewEvent::Location(url) = event
                && !this.address.focus_handle(cx).is_focused(window)
            {
                this.address
                    .update(cx, |field, cx| field.set_content(url.clone(), cx));
            }
        });
        self.page = Some((page.clone(), events));
        page
    }

    fn go(&mut self, _: &Go, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.address.read(cx).content().trim().to_owned();
        let Some((page, _)) = &self.page else { return };
        if typed.is_empty() {
            return;
        }
        let url = if typed.contains("://") {
            typed
        } else {
            format!("https://{typed}")
        };
        page.update(cx, |page, _| page.load(url));
        window.focus(&page.focus_handle(cx), cx);
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let painter = Painter::of(cx);
        let page = self.page(window, cx);
        let button = |key: &'static str, glyph: &'static [u8]| {
            theme
                .icon_button(
                    glyph,
                    ButtonStyle::Ghost,
                    Some(Fade::new(painter, format!("browser-{key}"))),
                )
                .id(SharedString::from(format!("browser-{key}")))
        };
        let (back, forward, reload) = (page.clone(), page.clone(), page.clone());
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .p(px(8.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        button("back", icons::glyph::ArrowLeft)
                            .on_click(move |_, _, cx| back.update(cx, |page, _| page.back())),
                    )
                    .child(
                        button("forward", icons::glyph::ArrowRight)
                            .on_click(move |_, _, cx| forward.update(cx, |page, _| page.forward())),
                    )
                    .child(
                        button("reload", icons::glyph::RotateCw)
                            .on_click(move |_, _, cx| reload.update(cx, |page, _| page.reload())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .on_action(cx.listener(Self::go))
                            .child(self.address.clone()),
                    ),
            )
            .child(div().flex_1().min_h_0().child(page))
    }
}
