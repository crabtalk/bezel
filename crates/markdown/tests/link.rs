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

#[test]
fn any_scheme_is_a_link_but_only_http_links_itself() {
    assert!(markdown::is_link("cydonia://cydonia#43"));
    assert!(!markdown::is_url("cydonia://cydonia#43"));
    assert!(!markdown::is_link("cydonia://"));
    assert!(!markdown::is_link("not a link://x"));
}

#[test]
fn an_app_link_alone_on_a_line_is_a_bookmark() {
    let doc = markdown::parse("[cydonia://cydonia#43](cydonia://cydonia#43 \"embed\")");
    assert!(matches!(
        &doc.blocks[0].kind,
        markdown::BlockKind::Bookmark { url, form: markdown::Form::Embed(None) } if url == "cydonia://cydonia#43"
    ));
    assert_eq!(
        markdown::serialize(&doc),
        "[cydonia://cydonia#43](cydonia://cydonia#43 \"embed\")"
    );
}

#[test]
fn an_app_link_in_bare_text_stays_text() {
    let doc = markdown::parse("see cydonia://cydonia#43");
    assert_eq!(markdown::serialize(&doc), "see cydonia://cydonia#43");
}
