//! A clicked link goes to the handler the app installed.

use markdown::AppExt as _;
use std::sync::Mutex;

use gpui::{
    App, Context, Modifiers, Render, TestAppContext, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};
use markdown::{Caption, Doc, Editing, parse, render, render_with};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 200.0;

static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());

static BASES: Mutex<Vec<Option<PathBuf>>> = Mutex::new(Vec::new());

fn record(url: &str, base: Option<&Path>, _: &mut Window, _: &mut App) {
    OPENED.lock().unwrap().push(url.to_string());
    BASES.lock().unwrap().push(base.map(Path::to_path_buf));
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

/// Rendered with [`Editing::base`] set to `/notes`.
struct Based(Doc);

impl Render for Based {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editing = Editing {
            base: Some(Path::new("/notes")),
            ..Default::default()
        };
        div()
            .w(px(WIDTH))
            .child(render_with(&self.0, editing, window, cx))
    }
}

#[gpui::test]
fn a_clicked_link_hands_the_handler_the_document_base(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        cx.set_link_handler(record);
    });
    let window = cx.add_window(|_, _| {
        Based(parse(
            "[a link that runs the whole width of the line](../todo.md)",
        ))
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    cx.run_until_parked();

    cx.simulate_click(point(px(40.0), px(12.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(*OPENED.lock().unwrap(), ["../todo.md"]);
    assert_eq!(*BASES.lock().unwrap(), [Some(PathBuf::from("/notes"))]);
}

/// Rendered with a [`markdown::OnJump`] that records the block it is handed.
struct Jumping(Doc, Rc<Cell<Option<usize>>>);

impl Render for Jumping {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let jumped = self.1.clone();
        let editing = Editing {
            jump: Some(Rc::new(move |block, _, _| jumped.set(Some(block)))),
            ..Default::default()
        };
        div()
            .w(px(WIDTH))
            .child(render_with(&self.0, editing, window, cx))
    }
}

#[gpui::test]
fn a_clicked_fragment_link_jumps_to_its_heading(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        cx.set_link_handler(record);
    });
    let jumped = Rc::new(Cell::new(None));
    let window = cx.add_window({
        let jumped = jumped.clone();
        move |_, _| {
            Jumping(
                parse("[a link that runs the whole width of the line](#set-up)\n\n## Set up"),
                jumped,
            )
        }
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    cx.run_until_parked();

    cx.simulate_click(point(px(40.0), px(12.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(jumped.get(), Some(1));
    assert!(OPENED.lock().unwrap().is_empty());
    assert_eq!(cx.cx.opened_url(), None);
}

#[gpui::test]
fn a_fragment_link_with_no_jump_opens_nothing(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        cx.set_link_handler(record);
    });
    let window = cx.add_window(|_, _| {
        Page(parse(
            "[a link that runs the whole width of the line](#set-up)\n\n## Set up",
        ))
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    cx.run_until_parked();

    cx.simulate_click(point(px(40.0), px(12.0)), Modifiers::none());
    cx.run_until_parked();

    assert!(OPENED.lock().unwrap().is_empty());
    assert_eq!(cx.cx.opened_url(), None);
}

#[test]
fn a_heading_slug_follows_github() {
    assert_eq!(markdown::link::slug("Set Up: the `cli`!"), "set-up-the-cli");
    assert_eq!(
        markdown::link::slug("snake_case and-dash"),
        "snake_case-and-dash"
    );
    assert_eq!(markdown::link::slug("Café über"), "café-über");
}

#[test]
fn a_repeated_heading_takes_the_next_free_suffix() {
    let doc = parse("# Notes\n\n# Notes\n\n# Notes 1\n\n# Notes");
    assert_eq!(markdown::link::heading(&doc, "notes"), Some(0));
    assert_eq!(markdown::link::heading(&doc, "notes-1"), Some(1));
    assert_eq!(markdown::link::heading(&doc, "notes-1-1"), Some(2));
    assert_eq!(markdown::link::heading(&doc, "notes-2"), Some(3));
    assert_eq!(markdown::link::heading(&doc, "Notes"), None);
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
