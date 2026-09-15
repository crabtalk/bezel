//! A mindmap on the canvas, with the JSON Canvas file a save would write
//! beside it.
//!
//! The `session` node is an app's own kind: [`render`] is installed with
//! `canvas::set_node_renderer` and paints it from a field the spec does not
//! name. Copy this file.

use canvas::{Canvas, CanvasView, model::Node};
use gpui::{AnyElement, App, Context, Entity, Render, ScrollHandle, Window, div, prelude::*, px};
use theme::{TextStyle, Theme, Typeset};
use ui::scroll::{self, Axes};

const SOURCE: &str = r##"{
  "nodes": [
    {"id":"root","type":"text","x":0,"y":0,"width":200,"height":40,"text":"# Canvas"},
    {"id":"format","type":"text","x":0,"y":0,"width":200,"height":40,"text":"**JSON Canvas**"},
    {"id":"nodes","type":"text","x":0,"y":0,"width":200,"height":40,"text":"Nodes and edges"},
    {"id":"link","type":"link","x":0,"y":0,"width":200,"height":40,"url":"https://jsoncanvas.org"},
    {"id":"keys","type":"text","x":0,"y":0,"width":200,"height":40,"text":"**Keys**","color":"6"},
    {"id":"tab","type":"text","x":0,"y":0,"width":200,"height":40,"text":"`tab` adds a child"},
    {"id":"enter","type":"text","x":0,"y":0,"width":200,"height":40,"text":"`enter` adds a sibling"},
    {"id":"f2","type":"text","x":0,"y":0,"width":200,"height":40,"text":"`f2` or double-click edits"},
    {"id":"plugins","type":"text","x":0,"y":0,"width":200,"height":40,"text":"**Plugins**","color":"4"},
    {"id":"session","type":"session","x":0,"y":0,"width":200,"height":64,"title":"Refactor the layout","turns":12}
  ],
  "edges": [
    {"id":"e1","fromNode":"root","toNode":"format","toEnd":"none"},
    {"id":"e2","fromNode":"format","toNode":"nodes","toEnd":"none"},
    {"id":"e3","fromNode":"format","toNode":"link","toEnd":"none"},
    {"id":"e4","fromNode":"root","toNode":"keys","toEnd":"none"},
    {"id":"e5","fromNode":"keys","toNode":"tab","toEnd":"none"},
    {"id":"e6","fromNode":"keys","toNode":"enter","toEnd":"none"},
    {"id":"e7","fromNode":"keys","toNode":"f2","toEnd":"none"},
    {"id":"e8","fromNode":"root","toNode":"plugins","toEnd":"none"},
    {"id":"e9","fromNode":"plugins","toNode":"session","toEnd":"none"}
  ]
}"##;

/// Paints `session` nodes; every other kind falls through to the canvas.
pub fn render(node: &Node, zoom: f32, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if node.kind != "session" {
        return None;
    }
    let theme = Theme::of(cx);
    let field = |key: &str| node.extra.get(key).cloned().unwrap_or_default();
    let title = field("title").as_str().unwrap_or_default().to_owned();
    let turns = field("turns").as_u64().unwrap_or_default();
    Some(
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(TextStyle::Headline.painted() * zoom))
                    .text_color(theme.text)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(TextStyle::Callout.painted() * zoom))
                    .text_color(theme.text_muted)
                    .child(format!("Session · {turns} turns")),
            )
            .into_any_element(),
    )
}

pub struct CanvasDemo {
    view: Entity<CanvasView>,
    scroll: ScrollHandle,
}

impl CanvasDemo {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let canvas = Canvas::parse(SOURCE).expect("the sample is a canvas");
        let view = cx.new(|cx| CanvasView::new(canvas, cx));
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        Self {
            view,
            scroll: ScrollHandle::new(),
        }
    }
}

impl Render for CanvasDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let json = self.view.read(cx).canvas().to_json();
        div()
            .size_full()
            .flex()
            .child(div().flex_1().min_w_0().child(self.view.clone()))
            .child(
                scroll::pane("canvas-json", Axes::Vertical)
                    .track_scroll(&self.scroll)
                    .w(px(320.0))
                    .h_full()
                    .p_4()
                    .border_l_1()
                    .border_color(theme.border)
                    .font_family(theme.font_mono.clone())
                    .text_style(TextStyle::Callout)
                    .text_color(theme.text_muted)
                    .child(json),
            )
    }
}
