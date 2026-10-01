//! A clicked link goes to the handler the app installed.

use markdown::AppExt as _;
use std::sync::Mutex;

use gpui::{
    App, Context, Modifiers, Render, TestAppContext, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};
use markdown::{Caption, Doc, parse, render};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 200.0;

static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn record(url: &str, _: &mut Window, _: &mut App) {
    OPENED.lock().unwrap().push(url.to_string());
}

struct Page(Doc);

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(WIDTH))
            .child(render(&self.0, Caption::default(), window, cx))
    }
}

#[gpui::test]
fn a_clicked_link_goes_to_the_installed_handler(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        cx.set_link_handler(record);
    });
    let window = cx.add_window(|_, _| {
        Page(parse(
            "[a link that runs the whole width of the line](https://example.com)",
        ))
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    cx.run_until_parked();

    cx.simulate_click(point(px(40.0), px(12.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(*OPENED.lock().unwrap(), ["https://example.com"]);
    assert_eq!(cx.cx.opened_url(), None);
}
