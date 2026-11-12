//! The surface: a [`CanvasEditor`] painted, and keys and the pointer turned
//! into its commands.
//!
//! Zoom scales the layout rather than transforming paint — gpui has no
//! transform for arbitrary elements — so content re-lays out at each zoom and
//! stays sharp.

use crate::{
    change::Change,
    document::{Document, DocumentChange, DocumentEvent},
    drag::DragHandler,
    edge::{self, EdgeKinds},
    edit::{CanvasEditor, CanvasEvent, Item},
    handle::{Handle, Role, Which},
    kind::{self, Capability, Card, Kinds, Look, PAD, RADIUS, Sizing, color},
    layout::{Arrow, Layout},
    mindmap,
    model::{self, Canvas, Edge, End, Node, Stroke},
    options::{Frame, Mark, Options, Overlays, Style},
    path::{Ends, Path, Rect},
    presentation::{self, Measure},
    snap::Snap,
    tool::{self, Hand, Hit, Pointer, Sketch, Tool, Wish},
    view::{
        editing::{Session, Target},
        geometry::Geometry,
    },
};
use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, ElementId, Entity,
    EventEmitter, ExternalPaths, FocusHandle, Focusable, HitboxBehavior, Hsla, KeyContext,
    KeyDownEvent, KeyUpEvent, Modifiers, ModifiersChangedEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PinchEvent, Pixels, Point, Render, ScrollWheelEvent,
    Subscription, WeakEntity, Window, canvas as painter, div, point, prelude::*, px, size,
};
use motion::{AppExt as _, LAYOUT};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};
use theme::Theme;
use web_time::Instant;

mod body;
mod edge_paths;
mod editing;
mod geometry;
mod labels;
mod paint;
mod pointer;

/// The key context the canvas binds in.
pub const CONTEXT: &str = "BezelCanvas";

/// Claims `tab`, which adds a child here, from `ui::focus` traversal.
fn key_context() -> KeyContext {
    let mut context = KeyContext::default();
    context.add(CONTEXT);
    context.add(ui::focus::CLAIMS_TAB);
    context
}

pub mod keys;

/// Install the canvas key bindings. Call after `editor::init`.
pub fn init(cx: &mut App) {
    ui::input::init(cx);
    cx.bind_keys(keys::bindings());
}

/// Canvas positions, by node id.
type Positions = HashMap<String, (f32, f32)>;

/// Nodes easing from where they were painted toward where the document puts
/// them.
struct Glide {
    from: Positions,
    to: Positions,
    since: Instant,
}

pub struct CanvasView {
    editor: CanvasEditor,
    pub document: Entity<Document>,
    _document: Subscription,
    _blur: Option<Subscription>,
    focus: FocusHandle,
    editing: Option<Session>,
    /// Every gesture, in the order a press is offered to them.
    tools: Vec<Box<dyn Tool>>,
    /// The tool holding the pointer until it comes up.
    holding: Option<usize>,
    modifiers: Modifiers,
    space: bool,
    pub(crate) style: Style,
    pub(crate) exporting: bool,
    pub(crate) overlays: Overlays,
    /// Where each node was painted last frame.
    shown: Positions,
    geometry: Geometry,
    glide: Option<Glide>,
    viewport: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Growing node heights read at prepaint, in canvas units.
    measured: Rc<RefCell<HashMap<String, i64>>>,
    content_heights: Rc<RefCell<HashMap<String, (i64, i64)>>>,
    label_layouts: Rc<RefCell<HashMap<String, labels::Layout>>>,
    /// Where a held press last was, and when its drift last moved the view.
    aim: Option<Point<Pixels>>,
    drifted: Option<Instant>,
}

impl EventEmitter<CanvasEvent> for CanvasView {}

impl Focusable for CanvasView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// An action that runs one editor command.
fn command<A: 'static>(
    cx: &Context<CanvasView>,
    run: impl Fn(&mut CanvasEditor) + 'static,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |this, _: &A, _, cx| this.update_editor(cx, &run))
}

impl CanvasView {
    /// `layout` places the nodes; the kinds are what [`crate::AppExt::set_canvas_kinds`]
    /// named, else the spec's.
    pub fn new(canvas: Canvas, layout: Layout, cx: &mut Context<Self>) -> Self {
        let document = cx.new(|_| Document::new(canvas));
        Self::for_document(document, layout, cx)
    }

    pub fn for_document(
        document: Entity<Document>,
        layout: Layout,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut editor =
            CanvasEditor::new(Canvas::default(), layout).with_kinds(kind::installed(cx));
        editor.take_document(document.read(cx).clone());
        let subscription = cx.subscribe(&document, |this, document, event: &DocumentEvent, cx| {
            if event.by == Some(cx.entity_id()) {
                return;
            }
            this.editor.take_document(document.read(cx).clone());
            this.cancel(cx);
            this.editing = None;
            this.glide = None;
            cx.notify();
        });
        Self {
            editor,
            document,
            _document: subscription,
            _blur: None,
            focus: cx.focus_handle(),
            editing: None,
            tools: tool::defaults(),
            holding: None,
            modifiers: Modifiers::default(),
            space: false,
            style: Style::default(),
            exporting: false,
            overlays: Overlays::new(),
            shown: HashMap::new(),
            geometry: Geometry::default(),
            glide: None,
            viewport: Rc::default(),
            measured: Rc::default(),
            content_heights: Rc::default(),
            label_layouts: Rc::default(),
            aim: None,
            drifted: None,
        }
    }

    pub fn with_kinds(mut self, kinds: Kinds) -> Self {
        self.editor.set_kinds(kinds);
        self
    }

    /// What the canvas is tuned by.
    pub fn with_options(mut self, options: Options) -> Self {
        self.editor.set_options(options);
        self
    }

    /// What its own paint measures.
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// What a node wears: its ring, a drop's wash, its handles, and the box it
    /// paints as when it is too far out to read.
    pub fn with_overlays(mut self, overlays: Overlays) -> Self {
        self.overlays = overlays;
        self
    }

    /// What an edge is, by its `type`. [`EdgeKinds::new`] unless an app says
    /// otherwise.
    pub fn with_edge_kinds(mut self, kinds: EdgeKinds) -> Self {
        self.editor.set_edge_kinds(kinds);
        self
    }

    /// Every gesture the canvas answers, in the order a press is offered to
    /// them. [`tool::defaults`] unless an app says otherwise.
    pub fn with_tools(mut self, tools: Vec<Box<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// What dragging a node does, in place of the layout's own.
    pub fn with_drag(mut self, handler: DragHandler) -> Self {
        self.editor.set_drag(handler);
        self
    }

    /// How dragged and resized boxes settle.
    pub fn with_snap(mut self, snap: Snap) -> Self {
        self.editor.set_snap(snap);
        self
    }

    /// See [`CanvasEditor::with_changes`].
    pub fn with_changes(
        mut self,
        filter: impl Fn(&Canvas, Change) -> Option<Change> + 'static,
    ) -> Self {
        self.editor = self.editor.with_changes(filter);
        self
    }

    pub fn editor(&self) -> &CanvasEditor {
        &self.editor
    }

    /// Run commands on the editor, announcing what they did.
    pub fn update_editor<R>(
        &mut self,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut CanvasEditor) -> R,
    ) -> R {
        self.sync_document(cx);
        let answer = update(&mut self.editor);
        self.announce(cx);
        cx.notify();
        answer
    }

    /// Where the view painted last frame, in window coordinates.
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        self.viewport.get()
    }

    /// What `cmd-c` does.
    pub fn copy(&self, cx: &mut App) {
        if let Some(json) = self.editor.copy() {
            cx.write_to_clipboard(ClipboardItem::new_string(json));
        }
    }

    /// What `cmd-x` does.
    pub fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        self.update_editor(cx, CanvasEditor::remove_selected);
    }

    /// What `cmd-v` does.
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.update_editor(cx, |editor| editor.paste(&text));
        }
    }

    fn sync_document(&mut self, cx: &mut Context<Self>) {
        let document = self.document.read(cx);
        if self.editor.document.revision != document.revision
            || !Rc::ptr_eq(&self.editor.document.canvas, &document.canvas)
        {
            self.editor.take_document(document.clone());
            self.editing = None;
            self.cancel(cx);
            self.glide = None;
        }
    }

    /// Emit what the editor did, and close an editor whose target is gone or
    /// was taken back.
    fn announce(&mut self, cx: &mut Context<Self>) {
        let canvas = self.editor.canvas();
        let gone = self
            .editing
            .as_ref()
            .is_some_and(|session| match &session.target {
                Target::Node(id) => canvas.node(id).is_none(),
                Target::Edge(id) => canvas.edge(id).is_none(),
                Target::New { .. } => false,
            });
        if self.editor.take_rewound() || gone || self.editor.options().read_only {
            self.editing = None;
            self.editor.group = None;
        }
        self.sync_input(cx);
        let events = self.editor.take_events();
        let changes: Vec<Change> = events
            .iter()
            .filter_map(|event| match event {
                CanvasEvent::Changed(changes) => Some(changes.iter().cloned()),
                _ => None,
            })
            .flatten()
            .collect();
        if !Rc::ptr_eq(&self.editor.document.canvas, &self.document.read(cx).canvas) {
            let snapshot = self.editor.document.clone();
            let by = Some(cx.entity_id());
            self.document.update(cx, |document, cx| {
                *document = snapshot;
                cx.emit(DocumentEvent {
                    by,
                    change: DocumentChange::Edited(changes),
                });
                cx.notify();
            });
        }
        for event in events {
            cx.emit(event);
        }
    }

    fn local(&self, position: Point<Pixels>) -> Point<f32> {
        let origin = self.viewport.get().map_or(Point::default(), |b| b.origin);
        point(
            (position.x - origin.x).as_f32(),
            (position.y - origin.y).as_f32(),
        )
    }

    pub fn canvas_point(&self, position: Point<Pixels>) -> Point<f32> {
        let (local, pan, zoom) = (self.local(position), self.editor.pan(), self.editor.zoom());
        point((local.x - pan.x) / zoom, (local.y - pan.y) / zoom)
    }

    /// The node a drag has in hand, once it has moved.
    fn held(&self) -> Option<&str> {
        self.holding.and_then(|ix| self.tools[ix].held())
    }

    /// What the tool holding the pointer draws.
    fn sketch(&self) -> Option<Sketch> {
        self.holding.and_then(|ix| self.tools[ix].sketch())
    }

    /// Edit what a command just added, its typing joining the add.
    pub fn edit_added(
        &mut self,
        added: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = added {
            self.grant(Wish::EditAdded(id), window, cx);
        }
    }

    pub fn can_fit_content(&self) -> bool {
        !self.editor.options().read_only
            && self.editor.selected_nodes().iter().any(|id| {
                self.editor.canvas().node(id).is_some_and(|node| {
                    self.editor.node_can(id, Capability::Resizable)
                        && self
                            .content_heights
                            .borrow()
                            .get(*id)
                            .is_some_and(|&(width, _)| width == node.width)
                })
            })
    }

    pub fn fit_to_content(&mut self, cx: &mut Context<Self>) {
        self.sync_document(cx);
        if self.editor.options().read_only {
            return;
        }
        let heights = self.content_heights.borrow();
        let changes: Vec<_> = self
            .editor
            .selected_nodes()
            .into_iter()
            .filter_map(|id| {
                let mut node = self.editor.canvas().node(id)?.clone();
                let &(width, height) = heights.get(id)?;
                if width != node.width || !self.editor.node_can(id, Capability::Resizable) {
                    return None;
                }
                node.height = height.max(self.editor.options().min_size.1);
                Some(Change::UpdateNode { node })
            })
            .collect();
        drop(heights);
        self.update_editor(cx, |editor| {
            editor.submit(changes);
        });
    }
}

impl Render for CanvasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self._blur.is_none() {
            self._blur = Some(cx.on_focus_out(&self.focus, window, |this, _, _, cx| {
                this.space = false;
                this.modifiers = Modifiers::default();
                this.cancel(cx);
                this.end_editing(cx);
            }));
        }
        self.sync_document(cx);
        if let Some(bounds) = self.viewport.get() {
            let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
            self.editor.set_viewport(size(w, h));
        }
        self.drift(window, cx);
        let measured: Vec<(String, i64)> = self.measured.borrow_mut().drain().collect();
        let held = self.held().map(str::to_owned);
        if !self.exporting {
            self.editor.reflow(measured, held.as_deref());
        }
        self.editor.frame();
        self.announce(cx);
        let changed = self.geometry.refresh(&self.editor, held.as_deref());
        let animated = self.positions(changed, self.exporting || cx.reduced_motion(), window);
        if changed || animated {
            self.geometry.paths.refresh(&self.editor, &self.shown);
        }
        let shown = &self.shown;
        let theme = Theme::of(cx).clone();
        let edges = self.edge_layer(&theme, cx.entity().downgrade());
        let canvas = self.editor.painted();
        let connecting = self.connect_target();
        let picked = self.editor.selected_nodes();
        let nodes: Vec<AnyElement> = self
            .geometry
            .order
            .iter()
            .copied()
            .filter_map(|ix| {
                let at = shown[&canvas.nodes[ix].id];
                self.paint_node(ix, at, connecting.as_deref(), &picked, window, cx)
            })
            .collect();
        let labels = self.labels(&theme, window, cx);
        let edge_handles = self.edge_handles(&theme, cx);
        let marquee = match self.sketch() {
            Some(Sketch::Box { from, to }) => {
                let (pan, zoom) = (self.editor.pan(), self.editor.zoom());
                let a = point(pan.x + from.x * zoom, pan.y + from.y * zoom);
                let b = point(pan.x + to.x * zoom, pan.y + to.y * zoom);
                Some(
                    div()
                        .absolute()
                        .left(px(a.x.min(b.x)))
                        .top(px(a.y.min(b.y)))
                        .w(px((a.x - b.x).abs()))
                        .h(px((a.y - b.y).abs()))
                        .border_1()
                        .border_color(theme.border_strong)
                        .bg(theme.accent.opacity(self.style.marquee_wash)),
                )
            }
            _ => None,
        };

        let prompt = self.edit_prompt(window, cx);
        div()
            .id("bezel-canvas")
            .bg(theme.canvas_bg)
            .key_context(key_context())
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .on_action(cx.listener(|this, _: &keys::AddChild, window, cx| {
                let added = this.update_editor(cx, CanvasEditor::add_child);
                this.edit_added(added, window, cx);
            }))
            .on_action(cx.listener(|this, _: &keys::AddSibling, window, cx| {
                let added = this.update_editor(cx, CanvasEditor::add_sibling);
                this.edit_added(added, window, cx);
            }))
            .on_action(command::<keys::Remove>(cx, CanvasEditor::remove_selected))
            .on_action(cx.listener(|this, _: &keys::Edit, window, cx| {
                this.edit_selected(window, cx);
            }))
            .on_action(cx.listener(|this, _: &keys::ConfirmEditing, window, cx| {
                this.confirm_editing(window, cx)
            }))
            .on_action(command::<keys::SelectAll>(cx, CanvasEditor::select_all))
            .on_action(cx.listener(|this, _: &keys::Deselect, _, cx| {
                this.cancel(cx);
                this.update_editor(cx, |editor| {
                    editor.select(None);
                    editor.select_edge(None);
                });
            }))
            .on_action(command::<keys::Undo>(cx, |editor| {
                editor.undo();
            }))
            .on_action(command::<keys::Redo>(cx, |editor| {
                editor.redo();
            }))
            .on_action(cx.listener(|this, _: &keys::Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &keys::Cut, _, cx| this.cut(cx)))
            .on_action(cx.listener(|this, _: &keys::Paste, _, cx| this.paste(cx)))
            .on_action(command::<keys::Duplicate>(cx, CanvasEditor::duplicate))
            .on_action(
                cx.listener(|this, _: &keys::StopEditing, window, cx| {
                    this.stop_editing(window, cx)
                }),
            )
            .on_action(command::<keys::SelectLeft>(cx, |e| {
                e.select_toward(Arrow::Left)
            }))
            .on_action(command::<keys::SelectRight>(cx, |e| {
                e.select_toward(Arrow::Right)
            }))
            .on_action(command::<keys::SelectUp>(cx, |e| {
                e.select_toward(Arrow::Up)
            }))
            .on_action(command::<keys::SelectDown>(cx, |e| {
                e.select_toward(Arrow::Down)
            }))
            .on_action(command::<keys::NudgeLeft>(cx, |e| e.nudge(Arrow::Left)))
            .on_action(command::<keys::NudgeRight>(cx, |e| e.nudge(Arrow::Right)))
            .on_action(command::<keys::NudgeUp>(cx, |e| e.nudge(Arrow::Up)))
            .on_action(command::<keys::NudgeDown>(cx, |e| e.nudge(Arrow::Down)))
            .on_action(command::<keys::ZoomIn>(cx, CanvasEditor::zoom_in))
            .on_action(command::<keys::ZoomOut>(cx, CanvasEditor::zoom_out))
            .on_action(command::<keys::ResetZoom>(cx, |e| e.set_zoom(1.0)))
            .on_action(command::<keys::Fit>(cx, CanvasEditor::fit))
            .on_action(command::<keys::ZoomToSelection>(
                cx,
                CanvasEditor::zoom_to_selection,
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    let hit = this.hit_at(event.position);
                    this.press(event, hit, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.press(event, Hit::Nothing, window, cx);
                }),
            )
            .on_mouse_down(MouseButton::Right, cx.listener(Self::request_context_menu))
            .when(self.editor.options().external_drops, |element| {
                element.on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    cx.emit(CanvasEvent::FilesDropped {
                        at: this.canvas_point(window.mouse_position()),
                        paths: paths.paths().to_vec(),
                    });
                    cx.stop_propagation();
                }))
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "space" && this.editing.is_none() {
                    this.space = true;
                    cx.stop_propagation();
                }
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                this.modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "space" {
                    this.space = false;
                }
            }))
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                this.modifiers = event.modifiers;
                if let Some(position) = this.aim {
                    this.feed(position, cx);
                }
            }))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_pinch(cx.listener(Self::pinch))
            .child(edges)
            .children(labels)
            .children(nodes)
            .children(edge_handles)
            .children(marquee)
            .children(prompt)
    }
}
