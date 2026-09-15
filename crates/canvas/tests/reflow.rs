//! Measured heights and layout are announced, so a save on `Changed` writes
//! what the view shows.

use std::{cell::RefCell, rc::Rc};

use canvas::{Canvas, CanvasEvent, CanvasView, Change, change};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, TestAppContext, VisualTestContext, Window, div, px, size,
};

/// root → a, b, at heights the content will not measure and nowhere laid out.
const DOC: &str = r#"{
  "nodes": [
    {"id":"root","type":"text","x":0,"y":0,"width":200,"height":7,"text":"root"},
    {"id":"a","type":"text","x":0,"y":0,"width":200,"height":7,"text":"a"},
    {"id":"b","type":"text","x":0,"y":0,"width":200,"height":7,"text":"b"}
  ],
  "edges": [
    {"id":"e1","fromNode":"root","toNode":"a"},
    {"id":"e2","fromNode":"root","toNode":"b"}
  ]
}"#;

type Log = Rc<RefCell<Vec<Change>>>;

struct Host {
    canvas: Entity<CanvasView>,
    _changes: Subscription,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.canvas.clone())
    }
}

/// Open `json`, drawing until it settles, and answer the view's document and
/// every change announced on the way.
fn open(json: &str, cx: &mut TestAppContext) -> (Canvas, Vec<Change>) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
        canvas::init(cx);
    });
    let log = Log::default();
    let canvas = Canvas::parse(json).unwrap();
    let window = cx.add_window({
        let log = log.clone();
        move |_, cx| {
            let canvas = cx.new(|cx| CanvasView::new(canvas, cx));
            let _changes = cx.subscribe(&canvas, move |_, _, event, _| {
                if let CanvasEvent::Changed(change) = event {
                    log.borrow_mut().push(change.clone());
                }
            });
            Host { canvas, _changes }
        }
    });
    let host = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.0), px(600.0)));
    for _ in 0..5 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }
    let doc = cx.update(|_, cx| host.read(cx).canvas.read(cx).canvas().clone());
    (doc, log.take())
}

#[gpui::test]
fn the_announced_changes_replay_to_the_view(cx: &mut TestAppContext) {
    let (shown, changes) = open(DOC, cx);
    assert!(changes.iter().any(|c| matches!(c, Change::Resize { .. })));
    assert!(changes.iter().any(|c| matches!(c, Change::Layout { .. })));
    let mut replayed = Canvas::parse(DOC).unwrap();
    for change in &changes {
        change::apply(&mut replayed, change);
    }
    assert_eq!(replayed, shown);
}

#[gpui::test]
fn a_saved_canvas_reopens_without_a_change(cx: &mut TestAppContext) {
    let (shown, _) = open(DOC, cx);
    let saved = shown.to_json();
    let (reopened, changes) = open(&saved, cx);
    assert_eq!(changes, []);
    assert_eq!(reopened.to_json(), saved);
}
