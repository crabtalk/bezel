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

/// The page as the gallery hosts it, since a canvas alone passes both.
#[cfg(test)]
mod tests {
    use canvas::CanvasView;
    use gpui::{
        Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point,
        px, size,
    };

    use crate::Gallery;

    fn open(cx: &mut TestAppContext) -> (Entity<CanvasView>, VisualTestContext) {
        cx.update(|cx| {
            ui::register_fonts(cx).ok();
            theme::Theme::install(theme::Appearance::Dark, cx);
            crate::init(cx);
        });
        let window = cx.add_window(|_, cx| Gallery::showing("canvas", cx));
        let gallery = window.root(cx).expect("gallery window");
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.simulate_resize(size(px(1000.0), px(860.0)));
        cx.run_until_parked();
        // A test window has no frame clock: draw until the resize, the centring
        // and the measured heights have all painted.
        for _ in 0..3 {
            cx.update(|window, cx| window.draw(cx).clear(cx));
        }
        let view = cx.update(|_, cx| gallery.read(cx).canvas.read(cx).view.clone());
        (view, cx)
    }

    fn click(at: Point<Pixels>, cx: &mut VisualTestContext) {
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    }

    #[gpui::test]
    fn the_background_drags(cx: &mut TestAppContext) {
        let (view, mut cx) = open(cx);
        let bounds = cx.update(|_, cx| view.read(cx).bounds()).expect("painted");
        assert!(bounds.size.height > px(100.0), "canvas is {bounds:?}");
        let at = bounds.origin + point(px(8.0), bounds.size.height - px(8.0));
        let before = cx.update(|_, cx| view.read(cx).pan());
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            at + point(px(40.0), px(-20.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            at + point(px(40.0), px(-20.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        let after = cx.update(|_, cx| view.read(cx).pan());
        assert_eq!((after.x - before.x, after.y - before.y), (40.0, -20.0));
    }

    /// A node's middle, in window coordinates.
    fn middle(view: &CanvasView, id: &str) -> Point<Pixels> {
        let (bounds, pan, zoom) = (view.bounds().expect("painted"), view.pan(), view.zoom());
        let node = view.canvas().node(id).expect("the sample has it");
        bounds.origin
            + point(
                px(pan.x + (node.x + node.width / 2) as f32 * zoom),
                px(pan.y + (node.y + node.height / 2) as f32 * zoom),
            )
    }

    #[gpui::test]
    fn a_node_drags_and_stays(cx: &mut TestAppContext) {
        let (view, mut cx) = open(cx);
        let (at, zoom, origin) = cx.update(|_, cx| {
            let view = view.read(cx);
            let node = view.canvas().node("keys").expect("the sample has it");
            (middle(view, "keys"), view.zoom(), (node.x, node.y))
        });
        let to = at + point(px(60.0), px(40.0));
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        let selected = cx.update(|_, cx| view.read(cx).selected().map(str::to_owned));
        assert_eq!(
            selected.as_deref(),
            Some("keys"),
            "the press missed the node"
        );
        let pan = cx.update(|_, cx| view.read(cx).pan());
        cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
        let (pan_after, mid) = cx.update(|_, cx| {
            let view = view.read(cx);
            let node = view.canvas().node("keys").expect("still there");
            (view.pan(), (node.x, node.y))
        });
        assert_eq!(pan, pan_after, "the move panned instead");
        assert_ne!(mid, origin, "the move never reached the drag");
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let node = cx
            .update(|_, cx| view.read(cx).canvas().node("keys").cloned())
            .expect("still there");
        let moved = ((60.0 / zoom).round() as i64, (40.0 / zoom).round() as i64);
        assert_eq!((node.x, node.y), (origin.0 + moved.0, origin.1 + moved.1));
        assert!(canvas::mindmap::is_pinned(&node));
    }

    #[gpui::test]
    fn tab_adds_after_clicking_empty_space(cx: &mut TestAppContext) {
        let (view, mut cx) = open(cx);
        let bounds = cx.update(|_, cx| view.read(cx).bounds()).expect("painted");
        let before = cx.update(|_, cx| view.read(cx).canvas().nodes.len());
        click(
            bounds.origin + point(px(8.0), bounds.size.height - px(8.0)),
            &mut cx,
        );
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| view.read(cx).canvas().nodes.len()),
            before + 1
        );
    }

    #[gpui::test]
    fn tab_adds_under_a_clicked_node(cx: &mut TestAppContext) {
        let (view, mut cx) = open(cx);
        let at = cx.update(|_, cx| {
            let view = view.read(cx);
            let (bounds, pan, zoom) = (view.bounds().expect("painted"), view.pan(), view.zoom());
            let root = view.canvas().node("root").expect("the sample has a root");
            bounds.origin
                + point(
                    px(pan.x + (root.x + root.width / 2) as f32 * zoom),
                    px(pan.y + (root.y + root.height / 2) as f32 * zoom),
                )
        });
        let before = cx.update(|_, cx| view.read(cx).canvas().nodes.len());
        click(at, &mut cx);
        assert_eq!(
            cx.update(|_, cx| view.read(cx).selected().map(str::to_owned))
                .as_deref(),
            Some("root")
        );
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(
            cx.update(|_, cx| view.read(cx).canvas().nodes.len()),
            before + 1
        );
    }
}
