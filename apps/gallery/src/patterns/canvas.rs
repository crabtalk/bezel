//! A mindmap on the canvas, with the JSON Canvas file a save would write
//! beside it.
//!
//! The `session` node is an app's own kind: [`session_kind`] paints fields the spec
//! does not name, edits its title in place, and makes a text note under it by
//! `tab`. The page keeps its root through `with_changes`.
//!
//! **The toolbar is a caller, not a component.** Every control is public
//! `CanvasView` API — adding goes through `submit`, so the filter that keeps
//! the root refuses the toolbar's delete as it refuses `backspace`, and zoom
//! is the entry point its chord takes. Copy this file.

use canvas::{
    Canvas, CanvasView, Change, change,
    drag::{self, DragHandler},
    kind::{self, Field, Kind},
    layout::{self, Layout},
    mindmap,
    model::Node,
};
use gpui::{
    AnyElement, App, Context, Entity, Focusable, Render, ScrollHandle, SharedString, Window, div,
    prelude::*, px,
};
use motion::{Fade, Painter};
use theme::{TextStyle, Theme, Typeset};
use ui::{
    scroll::{self, Axes},
    tooltip::Tooltip,
    widgets::{ButtonStyle, Buttons},
};

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

/// The sample's root, the one node the page will not let go.
const ROOT: &str = "root";

/// Installed with `canvas::set_kinds` under `"session"`.
pub fn session_kind() -> Kind {
    Kind::new(session).edit(Field::new(title, set_title))
}

fn title(node: &Node) -> String {
    node.extra
        .get("title")
        .and_then(|title| title.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn set_title(node: &mut Node, title: String) {
    node.extra.insert("title".into(), title.into());
}

fn session(node: &Node, zoom: f32, _: &mut Window, cx: &mut App) -> AnyElement {
    let theme = Theme::of(cx);
    let turns = node
        .extra
        .get("turns")
        .and_then(|turns| turns.as_u64())
        .unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .child(
            canvas::text_style(div(), TextStyle::Headline, zoom)
                .text_color(theme.text)
                .child(title(node)),
        )
        .child(
            canvas::text_style(div(), TextStyle::Callout, zoom)
                .text_color(theme.text_muted)
                .child(format!("Session · {turns} turns")),
        )
        .into_any_element()
}

/// What dropping a node does, as the toolbar offers it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DragMode {
    Move,
    Reparent,
    Detach,
}

impl DragMode {
    /// Each mode, its key and glyph, and what its tooltip says a drop does.
    const ALL: [(Self, &'static str, &'static [u8], &'static str); 3] = [
        (
            Self::Move,
            "move",
            icons::glyph::Move,
            "Move — the node stays where you drop it",
        ),
        (
            Self::Reparent,
            "reparent",
            icons::glyph::GitFork,
            "Reparent — drop onto a node to hang it there",
        ),
        (
            Self::Detach,
            "detach",
            icons::glyph::Unlink,
            "Detach — dropping cuts its connector",
        ),
    ];

    fn handler(self) -> DragHandler {
        match self {
            Self::Move => drag::pin,
            Self::Reparent => drag::reparent,
            Self::Detach => drag::detach,
        }
    }
}

/// Who places the nodes, as the toolbar offers it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Free,
    Mindmap,
    Balanced,
    Down,
}

impl LayoutMode {
    const ALL: [(Self, &'static str, &'static [u8], &'static str); 4] = [
        (
            Self::Free,
            "free",
            icons::glyph::LayoutGrid,
            "Free — nodes stay where they are put, arrows find the nearest",
        ),
        (
            Self::Mindmap,
            "mindmap",
            icons::glyph::ListTree,
            "Mindmap — the tree grows right",
        ),
        (
            Self::Balanced,
            "balanced",
            icons::glyph::Split,
            "Balanced — the root's branches split both ways",
        ),
        (
            Self::Down,
            "down",
            icons::glyph::Network,
            "Down — the tree grows downward",
        ),
    ];

    fn layout(self) -> Layout {
        match self {
            Self::Free => layout::FREE,
            Self::Mindmap => layout::MINDMAP,
            Self::Balanced => layout::BALANCED,
            Self::Down => layout::DOWN,
        }
    }
}

/// A new node of the page's two kinds, before the canvas gives it an id and a
/// place.
fn fresh(session: bool) -> Node {
    if !session {
        return Node {
            text: Some("Note".into()),
            ..kind::blank(&Node::default())
        };
    }
    let mut node = Node {
        kind: "session".into(),
        width: 200,
        height: 64,
        ..Node::default()
    };
    node.extra.insert("title".into(), "New session".into());
    node.extra.insert("turns".into(), 0u64.into());
    node
}

pub struct CanvasDemo {
    view: Entity<CanvasView>,
    scroll: ScrollHandle,
    drag: DragMode,
    layout: LayoutMode,
}

impl CanvasDemo {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let canvas = Canvas::parse(SOURCE).expect("the sample is a canvas");
        let view = cx.new(|cx| {
            CanvasView::new(canvas, cx).with_changes(|_, change, _| match &change {
                // The page keeps its root.
                Change::RemoveNodes { ids } if ids.iter().any(|id| id == ROOT) => None,
                _ => Some(change),
            })
        });
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        Self {
            view,
            scroll: ScrollHandle::new(),
            drag: DragMode::Move,
            layout: LayoutMode::Mindmap,
        }
    }

    /// A toolbar press hands the keys back to the canvas, so `backspace` after
    /// "Add" removes what was added.
    fn refocus(&self, window: &mut Window, cx: &mut App) {
        let handle = self.view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    /// Under the selection, or a root in the middle of the view — through the
    /// filter, the way `tab` goes.
    fn add(&mut self, session: bool, cx: &mut Context<Self>) {
        self.view.update(cx, |view, cx| {
            let node = fresh(session);
            let changes = match view.selected().map(str::to_owned) {
                Some(parent) => mindmap::child(view.canvas(), &parent, node),
                None => {
                    let (x, y) = view.center();
                    let at = (x - node.width / 2, y - node.height / 2);
                    Some(vec![mindmap::root(view.canvas(), node, at)])
                }
            };
            if let Some(changes) = changes
                && let Some(id) = change::added(&changes).map(str::to_owned)
                && view.submit(changes, cx)
            {
                view.select(Some(id), cx);
            }
        });
    }

    fn toolbar(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let painter = Painter::of(cx);
        let view = self.view.read(cx);
        let removable = view.selected().is_some_and(|id| id != ROOT);
        let (zoom, can_undo, can_redo) = (view.zoom(), view.can_undo(), view.can_redo());
        let button = |key: &'static str, glyph: &'static [u8]| {
            theme
                .icon_button(
                    glyph,
                    ButtonStyle::Ghost,
                    Some(Fade::new(painter, format!("canvas-tool-{key}"))),
                )
                .id(SharedString::from(format!("canvas-tool-{key}")))
        };
        // Lit the way the ribbon lights a mark it applies.
        let lit = |button: gpui::Stateful<gpui::Div>, on: bool| {
            button.when(on, |el| el.bg(theme.element_active).text_color(theme.text))
        };
        let tip = |text: &'static str| {
            move |window: &mut Window, cx: &mut App| Tooltip::text(text, window, cx)
        };
        // The chord comes off the keymap, so a rebound key moves the hint.
        let chord = |label: &'static str, action: Box<dyn gpui::Action>| {
            move |window: &mut Window, cx: &mut App| {
                Tooltip::for_action_in(label, action.as_ref(), canvas::CONTEXT, window, cx)
            }
        };
        let caption = |text: &'static str| {
            div()
                .text_style(TextStyle::Caption2)
                .text_color(theme.text_faint)
                .child(text)
        };

        let add = theme
            .control_group()
            .child(
                button("note", icons::glyph::StickyNote)
                    .tooltip(tip("Add a note — under the selection, or in the middle"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add(false, cx);
                        this.refocus(window, cx);
                    })),
            )
            .child(
                button("session", icons::glyph::MessageSquare)
                    .tooltip(tip("Add a session — under the selection, or in the middle"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add(true, cx);
                        this.refocus(window, cx);
                    })),
            )
            .child(
                button("remove", icons::glyph::Trash)
                    .when(!removable, |button| button.opacity(0.4))
                    .tooltip(chord(
                        "Remove the selection",
                        Box::new(canvas::keys::Remove),
                    ))
                    .when(removable, |button| {
                        button.on_click(cx.listener(|this, _, window, cx| {
                            this.view.update(cx, |view, cx| view.remove_selected(cx));
                            this.refocus(window, cx);
                        }))
                    }),
            );

        let history = theme
            .control_group()
            .child(
                button("undo", icons::glyph::Undo2)
                    .when(!can_undo, |button| button.opacity(0.4))
                    .tooltip(chord("Undo", Box::new(canvas::keys::Undo)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view.update(cx, |view, cx| view.undo(cx));
                        this.refocus(window, cx);
                    })),
            )
            .child(
                button("redo", icons::glyph::Redo2)
                    .when(!can_redo, |button| button.opacity(0.4))
                    .tooltip(chord("Redo", Box::new(canvas::keys::Redo)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view.update(cx, |view, cx| view.redo(cx));
                        this.refocus(window, cx);
                    })),
            );

        let drags =
            theme
                .control_group()
                .children(DragMode::ALL.map(|(mode, key, glyph, text)| {
                    lit(button(key, glyph), self.drag == mode)
                        .tooltip(tip(text))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.drag = mode;
                            this.view
                                .update(cx, |view, _| view.set_drag(mode.handler()));
                            this.refocus(window, cx);
                            cx.notify();
                        }))
                }));

        let layouts =
            theme
                .control_group()
                .children(LayoutMode::ALL.map(|(mode, key, glyph, text)| {
                    lit(button(key, glyph), self.layout == mode)
                        .tooltip(tip(text))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.layout = mode;
                            this.view
                                .update(cx, |view, cx| view.set_layout(mode.layout(), cx));
                            this.refocus(window, cx);
                            cx.notify();
                        }))
                }));

        let zooms = theme
            .control_group()
            .child(
                button("zoom-out", icons::glyph::ZoomOut)
                    .tooltip(chord("Zoom out", Box::new(canvas::keys::ZoomOut)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view.update(cx, |view, cx| view.zoom_out(cx));
                        this.refocus(window, cx);
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(px(4.0))
                    .font_family(theme.font_mono.clone())
                    .text_style(TextStyle::Caption2)
                    .text_color(theme.text_muted)
                    .child(format!("{:.0}%", zoom * 100.0)),
            )
            .child(
                button("zoom-in", icons::glyph::ZoomIn)
                    .tooltip(chord("Zoom in", Box::new(canvas::keys::ZoomIn)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view.update(cx, |view, cx| view.zoom_in(cx));
                        this.refocus(window, cx);
                    })),
            )
            .child(
                button("fit", icons::glyph::Scan)
                    .tooltip(tip("Fit — centre the document again"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view.update(cx, |view, cx| view.fit(cx));
                        this.refocus(window, cx);
                    })),
            );

        // Docked, like the ribbon: it sits above the canvas rather than over
        // it, and wraps when the pane is narrow.
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(12.0))
            .pb(px(12.0))
            .border_b_1()
            .border_color(theme.hairline(0.10))
            .child(add)
            .child(history)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(caption("Drag"))
                    .child(drags),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(caption("Layout"))
                    .child(layouts),
            )
            .child(div().flex_1())
            .child(zooms)
    }
}

impl Render for CanvasDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let json = self.view.read(cx).canvas().to_json();
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.toolbar(&theme, cx))
                    .child(div().flex_1().min_h_0().child(self.view.clone())),
            )
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

    use super::{CanvasDemo, ROOT};
    use crate::Gallery;

    fn open(
        cx: &mut TestAppContext,
    ) -> (Entity<CanvasDemo>, Entity<CanvasView>, VisualTestContext) {
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
        let demo = cx.update(|_, cx| gallery.read(cx).canvas.clone());
        let view = cx.update(|_, cx| demo.read(cx).view.clone());
        (demo, view, cx)
    }

    #[gpui::test]
    fn a_session_the_toolbar_adds_can_be_removed(cx: &mut TestAppContext) {
        let (demo, view, mut cx) = open(cx);
        cx.update(|_, cx| demo.update(cx, |demo, cx| demo.add(true, cx)));
        let added = cx
            .update(|_, cx| view.read(cx).selected().map(str::to_owned))
            .expect("adding selects");
        cx.update(|_, cx| view.update(cx, |view, cx| view.remove_selected(cx)));
        assert!(cx.update(|_, cx| view.read(cx).canvas().node(&added).is_none()));

        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.select(Some(ROOT.into()), cx);
                view.remove_selected(cx);
            })
        });
        assert!(cx.update(|_, cx| view.read(cx).canvas().node(ROOT).is_some()));
    }

    fn click(at: Point<Pixels>, cx: &mut VisualTestContext) {
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    }

    #[gpui::test]
    fn the_background_drags(cx: &mut TestAppContext) {
        let (_, view, mut cx) = open(cx);
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
        let (_, view, mut cx) = open(cx);
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
        let (_, view, mut cx) = open(cx);
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
        let (_, view, mut cx) = open(cx);
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
