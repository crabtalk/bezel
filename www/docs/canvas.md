---
title: Canvas
description: A mindmap over JSON Canvas — an app's own node kinds, and every edit handed to the app as a change it can refuse or rewrite.
---

```rust
editor::init(cx);
canvas::init(cx);                                            // after editor::init

let view = cx.new(|cx| {
    CanvasView::new(Canvas::parse(json)?, canvas::layout::MINDMAP, cx)
        .with_kinds(Kinds::new().with("session", session(store)))  // optional
});
cx.subscribe(&view, |_, view, event, cx| {
    if let CanvasEvent::Changed(_) = event {
        save(view.read(cx).editor().canvas().to_json());
    }
}).detach();
```

The document is [JSON Canvas 1.0](https://jsoncanvas.org/spec/1.0/). Fields the spec does not name are kept and written back, so a file another app wrote survives a save.

## Your own kinds

```rust
fn session(store: Store) -> Kind {
    // Everything inside the box is yours: dress it or not, and place the
    // editor while f2 is open.
    Kind::new(move |node, look, window, cx| {
        let body = look.editor.unwrap_or_else(|| store.view(&node.id).into_any_element());
        kind::chrome(Chrome::Card, node, look.zoom, cx).child(body).into_any_element()
    })
    .sizing(Sizing::Fixed)               // or Grows: as tall as the content
    .edit(Field::new(title, set_title))  // what f2 edits
    .open(|node, cx| resume(node, cx))   // what a double-click does instead
    .child(kind::blank)                  // what tab makes under it
    .holds()                             // what sits in its box is held
}

Kinds::new().with("session", session(store)).with_fresh(|| new_session());  // what the canvas makes from nothing
```

A kind is keyed by the node's `type`, and holds what it needs. Kinds belong to a view (`with_kinds`); `canvas::set_kinds(cx, kinds)` names them for every view that names none. A kind's `rules` — sizing, edit field, child, holds — are what the editor reads without a window. The canvas owns the node's box — position, size, the selection ring, the handles, dragging and resizing — and the kind paints everything inside it; `kind::chrome` dresses a box as the spec's kinds do. The spec's four — `kind::text()`, `file()`, `link()`, `group()` — are replaceable the same way. `Kinds::with_root(dir)` finds files and group backgrounds under `dir`, so images preview. A link opens on a double-click. `with_fresh` names the node made from nothing — a double-click on empty canvas, `tab` on an empty one, pasted text, written through its kind's edit field — a blank text node unless an app says.

## Containers

Any node holds others. A node names its container in `"container"` (`contain::hold`); one that names none sits in the smallest node around it whose kind `holds`, as a group does. What a node holds, however deep, moves, copies and duplicates with it and paints above it; removing a container leaves what it held. A node carried out of the container it names lets it go, and a kind that holds is never a drop target.

State stays with the app: look the view up by the node's id. gpui cannot transform an element, so `zoom` is a scale the content applies itself — `canvas::text_style(div(), TextStyle::Callout, zoom)` sets size, leading and weight together, and `Typography::scaled` does a whole document. A node of fixed size clips what it paints.

## Changes

```rust
CanvasView::new(doc, layout, cx).with_changes(move |canvas, change| match &change {
    Change::RemoveNodes { ids } if ids.iter().any(|id| running.contains(id)) => None, // refuse
    _ => Some(change),                                                                // or rewrite it
})
```

Every edit — a key, a drop, typing in a node — is a batch of graph changes: `AddNode`, `AddEdge`, `RemoveNodes`, `RemoveEdges`, `MoveNodes`, `Resize`, `UpdateNode`, `UpdateEdge`. Each goes through the filter, and one refused refuses the batch. The filter sees only the document and the change; what it needs from the app, it captures. `CanvasEvent::Changed` carries each batch that landed, the view's own measured heights and layout included, so saving on it misses nothing. `editor.apply(changes)` lands the app's own past the filter.

Tree edits live in `canvas::mindmap` and answer the batch they make: `child`, `sibling`, `remove` (a branch), `reparent`, `detach`, `carry`. An edge with `"tree": false` is a cross link, never a branch.

## Layouts

```rust
CanvasView::new(doc, canvas::layout::DOWN, cx);

let radial = Layout { walk: my_walk, ..Layout::free(my_arrange) };
```

Every canvas names a layout; there is no default. A `Layout` answers where nodes go after every change, and what its edits mean: `reach` (what acting on a node touches), `walk` (where an arrow goes), `link` and `extend` (what a connector makes), `paste_under` and `duplicate_under`, whether a move `pins`, and the `drag` it moves with. `Layout::free` fills them for a canvas where nodes stay put, `Layout::tree` for one whose edges are branches; replace a field for your own.

`layout::MINDMAP` grows trees right, `BALANCED` splits a root's branches both ways, `DOWN` grows them down, and `FREE` leaves nodes where they are put. Under a tree, arrows walk it, `backspace` takes a branch, a paste hangs under the selection, a connector between two nodes is a cross link, and a drop pins. On a free canvas, arrows go to the nearest node, `backspace` takes one node, and a connector makes a node where it is let go.

## Dragging a node

```rust
CanvasView::new(doc, layout::FREE, cx).with_drag(canvas::drag::reparent);

fn my_drag(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    // drag.id, drag.with (the rest of the selection), drag.contents (what the held nodes hold), drag.to(), drag.over, drag.phase (Move…, then one Drop)
}
```

On a move, the handler's `MoveNodes` paint over the document (`editor.painted()`) and the rest are drawn as what the drop would do: a ring on a node an `AddEdge` reaches, the connector it would make, faded connectors a `RemoveEdges` would cut. `editor.canvas()` stays as it was until the drop lands as one batch through the filter, so a refused drop leaves everything where it was. A resize in hand paints the same way. Nodes a layout moves glide there. A layout brings its own — `drag::pin` for a tree, which leaves the node where it lands and marks it `"pinned": true`, and `drag::moves` for a free canvas, which pins nothing — and `with_drag` replaces it. `drag::reparent` hangs the node under the one it is dropped on; `drag::detach` cuts its edges in.

## Selection, undo and the clipboard

Shift- or cmd-click adds to the selection, a shift-drag on empty canvas selects what the box touches, `cmd-a` selects all and `escape` none. The keys act from the primary — the last chosen — and removing, nudging, dragging, copying and duplicating take the whole selection, each node with its branch under a tree layout.

`cmd-z` and `cmd-shift-z` undo and redo what landed through `submit` or `apply`; an add and the typing into it are one step. Copy writes JSON Canvas; paste mints fresh ids and lands under the selection in a tree, or in the middle of the view, and plain text pastes as a text node. `cmd-d` duplicates.

## Edges and boxes

Click an edge to pick it: `backspace` removes it, and a double-click or `f2` edits its label. A picked node shows a handle on each side — drag one onto a node to connect them, or onto nothing to make a node there, a child under a tree — and a corner that resizes it. A box that grows with its content is pulled to a least height, kept as `"minHeight"`, and stays as tall as its content. A connector drawn under a tree layout is a cross link.

## Finding your way

`shift-1` fits the whole document in view and `shift-2` the selection, and a node a key moves to is panned into view. A drag held near the view's edge pans it. `CanvasView::with_snap(Snap { grid: Some(20), guides: true })` lands dragged and resized boxes on a grid, drawn as dots, and on lines other nodes share, drawn as guides. `canvas::minimap(&view, cx)` maps the whole canvas where an app places it; press or drag in it to look there. Far out, nodes paint as their boxes.

## Keys

`tab` adds a child (under the first root when nothing is selected), `enter` a sibling, `backspace` removes, `f2` or a double-click edits, arrows move the selection, `shift`-arrows nudge it, `escape` leaves a node. A double-click on nothing adds a node there. `cmd-=`, `cmd--` and `cmd-0` zoom, `shift-1` fits and `shift-2` zooms to the selection; a pinch or a `cmd`-wheel zooms at the pointer, and a drag, a middle-button drag or a wheel pans.

## What a node lets you do

```rust
Kind::new(paint).can(Capabilities { draggable: false, ..Capabilities::ALL })
```

A kind says what its nodes allow — `draggable`, `selectable`, `connectable`, `resizable`, `deletable` — and a node or an edge refuses any of them with a field of its own (`"draggable": false`). `Capabilities::READ_ONLY` allows only selecting. Every command honours it, so a key, a toolbar and a gesture stop at the same place: what cannot be selected is never selected, what cannot be deleted stays behind when the rest goes, what cannot be dragged does not nudge, and a node that cannot connect or resize paints no handle for it. What a container holds moves with it either way. `editor.can(&Item::Node(id), Capability::Draggable)` asks.

## Gestures

```rust
CanvasView::new(doc, layout, cx).with_tools(my_tools());  // tool::defaults() unless you say
```

Each gesture is a `Tool`. The view says what a press landed on — a node, an edge, a handle, or nothing — and the first tool to take it holds the pointer until it comes up. `tool::defaults()` are `Connect`, `Resize`, `PickEdge`, `Marquee`, `Create`, `Select` and `Pan`, in that order; drop one, reorder them, or write your own. A tool reads and commands a `CanvasEditor`, says what it draws as a `Sketch` for the view to paint, and asks the view for what only it can do — opening a node, or typing in one — with a `Wish`. `escape` gives up the gesture in hand.

## Edges

```rust
CanvasView::new(doc, layout, cx).with_edge_kinds(EdgeKinds::new().with("straight", edge::line()))
```

An edge's `type` names its kind, as a node's does. An `EdgeKind` says where it runs, how thick it paints, how near a press must come to pick it (`reach`), what its label edits (`EdgeField`), what it lets the reader do, and what handles it declares. `edge::curve()` is the spec's and `edge::line()` a straight one; an app's own is a function from `Ends` — the boxes it joins and the sides it leaves — to a `Path` of quadratic segments. The geometry is pure (`canvas::path`), so a press is measured against the same line the paint draws, and a label sits at its middle.

## Handles

```rust
Kind::new(paint).handles(|_| vec![Handle::connect(Side::Right), Handle::corner()])
```

A kind declares what a picked node or edge paints, and what dragging each one does. A `Handle` is an id, a `Spot` and a `Role`: a node's spots are `Side { side, at }` and `Corner`, an edge's are `End(Which)` and `Along(t)`; the roles are `Connect`, `Resize` and `Reconnect`. The canvas works out where each lands from the box or the path, so a press finds it without waiting for a frame to be laid out, and a kind that declares none has none. Dragging an edge's end carries it onto another node. A connector remembers the handle it left from in our own `fromHandle`, and the edge leaves exactly where that handle sits — a quarter of the way along a side, if that is what the kind declared. The side it leaves is still written to `fromSide`, so a reader that knows only the spec sees the edge on the right face.

## Tuning and overlays

```rust
CanvasView::new(doc, layout, cx)
    .with_options(Options { max_zoom: 8.0, ..Options::default() })
    .with_overlays(Overlays { ring: Rc::new(my_ring), ..Overlays::new() })
```

`Options` is what the canvas is tuned by — the zoom's limits and step, the drag threshold, the nudge, how far a duplicate sits, how many undo steps are kept, the smallest box a corner pulls to, the room `fit` leaves, the snap's reach, the drift step — read by the editor and by the tools. `Style` is what the canvas's own paint measures: the ring, handles, arrowheads, an edge label's room, the washes, the zoom a node stops being read at, and the grid's dots. `Overlays` is everything the canvas paints beside the kinds: a node's `ring`, the `drop` wash over one a connector would land on, each `handle`, the `placeholder` a node too far out paints as, the `grid` behind them all, and the `guides` a drag catches on. Each is replaceable; the marquee and the connector belong to the tools that draw them.

## The editor

```rust
let mut editor = CanvasEditor::new(Canvas::parse(json)?).with_layout(layout::FREE);
editor.select(Some("a".into()));
editor.remove_selected();
editor.undo();

view.update(cx, |view, cx| view.update_editor(cx, |editor| editor.fit())); // a toolbar's way in
```

`CanvasEditor` is the canvas without a window: the document, its kinds and layout, the selection, history and the part in view, behind the commands the keys run. `CanvasView` paints one and turns keys and the pointer into its commands; `view.editor()` reads it, and `update_editor` runs commands and announces what they did as `CanvasEvent`s. The selection is `Item`s, nodes or one edge.

Headless callers should regularly call `editor.take_events()` to drain pending events and release retained changes. The view drains them automatically.

## API

```rust
impl CanvasView {
    /// `layout` places the nodes; the kinds are what `set_kinds` named, else the spec's.
    pub fn new(canvas: Canvas, layout: Layout, cx: &mut Context<Self>) -> Self;
    pub fn with_kinds(self, kinds: Kinds) -> Self;
    pub fn with_edge_kinds(self, kinds: EdgeKinds) -> Self;
    /// Every gesture, in the order a press is offered to them.
    pub fn with_tools(self, tools: Vec<Box<dyn Tool>>) -> Self;
    pub fn with_drag(self, handler: DragHandler) -> Self;
    /// What it is tuned by, what its paint measures, and what a node wears.
    pub fn with_options(self, options: Options) -> Self;
    pub fn with_style(self, style: Style) -> Self;
    pub fn with_overlays(self, overlays: Overlays) -> Self;
    pub fn with_snap(self, snap: Snap) -> Self;
    pub fn with_changes(self, filter: impl Fn(&Canvas, Change) -> Option<Change> + 'static) -> Self;
    pub fn editor(&self) -> &CanvasEditor;
    pub fn update_editor<R>(&mut self, cx: &mut Context<Self>, update: impl FnOnce(&mut CanvasEditor) -> R) -> R;
    /// Where the view painted last frame, in window coordinates.
    pub fn bounds(&self) -> Option<Bounds<Pixels>>;
    /// Through the clipboard.
    pub fn copy(&self, cx: &mut App);
    pub fn cut(&mut self, cx: &mut Context<Self>);
    pub fn paste(&mut self, cx: &mut Context<Self>);
}

impl CanvasEditor {
    pub fn new(canvas: Canvas, layout: Layout) -> Self;
    pub fn with_kinds(self, kinds: Kinds) -> Self;
    pub fn with_drag(self, handler: DragHandler) -> Self;
    pub fn with_snap(self, snap: Snap) -> Self;
    pub fn with_changes(self, filter: impl Fn(&Canvas, Change) -> Option<Change> + 'static) -> Self;
    /// The document as a save would write it.
    pub fn canvas(&self) -> &Canvas;
    /// The document with the gesture in hand over it.
    pub fn painted(&self) -> &Canvas;
    pub fn set_canvas(&mut self, canvas: Canvas);
    pub fn kinds(&self) -> &Kinds;
    pub fn set_kinds(&mut self, kinds: Kinds);
    pub fn edge_kinds(&self) -> &EdgeKinds;
    pub fn set_edge_kinds(&mut self, kinds: EdgeKinds);
    /// What it is tuned by; the tools read it too.
    pub fn options(&self) -> Options;
    pub fn set_options(&mut self, options: Options);
    /// Through the filter, as if the reader made it.
    pub fn submit(&mut self, changes: impl IntoIterator<Item = Change>) -> bool;
    /// Past the filter.
    pub fn apply(&mut self, changes: impl IntoIterator<Item = Change>);
    pub fn undo(&mut self) -> bool;
    pub fn redo(&mut self) -> bool;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    /// Whether an item lets the reader do this; every command honours it.
    pub fn can(&self, item: &Item, what: Capability) -> bool;
    pub fn node_can(&self, id: &str, what: Capability) -> bool;
    pub fn edge_can(&self, id: &str, what: Capability) -> bool;
    /// Nodes, or one edge; the primary last.
    pub fn selection(&self) -> &[Item];
    pub fn selected_nodes(&self) -> Vec<&str>;
    /// The primary selection.
    pub fn selected(&self) -> Option<&str>;
    pub fn selected_edge(&self) -> Option<&str>;
    pub fn select(&mut self, id: Option<String>);
    pub fn set_selection(&mut self, ids: Vec<String>);
    /// Selecting an edge lets the nodes go, and nodes the edge.
    pub fn select_edge(&mut self, id: Option<String>);
    pub fn select_all(&mut self);
    /// What `backspace` does.
    pub fn remove_selected(&mut self);
    /// JSON Canvas, for the clipboard.
    pub fn copy(&self) -> Option<String>;
    pub fn cut(&mut self) -> Option<String>;
    pub fn paste(&mut self, text: &str);
    pub fn duplicate(&mut self);
    /// What the arrows and `shift`-arrows do.
    pub fn select_toward(&mut self, arrow: Arrow);
    pub fn nudge(&mut self, arrow: Arrow);
    /// What `tab`, `enter`, a double-click on nothing and a connector let go do; each answers the node added.
    pub fn add_child(&mut self) -> Option<String>;
    pub fn add_sibling(&mut self) -> Option<String>;
    pub fn add_root(&mut self, at: (i64, i64)) -> Option<String>;
    pub fn connect(&mut self, from: &str, handle: &Handle, at: (i64, i64)) -> Option<String>;
    pub fn zoom(&self) -> f32;
    /// Where the canvas origin sits, in pixels from the view's top left.
    pub fn pan(&self) -> Point<f32>;
    pub fn pan_by(&mut self, dx: f32, dy: f32);
    pub fn set_zoom(&mut self, zoom: f32);
    pub fn zoom_in(&mut self);
    pub fn zoom_out(&mut self);
    pub fn zoom_about(&mut self, zoom: f32, anchor: Point<f32>);
    /// The view's size in pixels, which the view sets as it paints.
    pub fn viewport(&self) -> Option<Size<f32>>;
    pub fn set_viewport(&mut self, size: Size<f32>);
    /// The whole document in view, no closer than 100%.
    pub fn fit(&mut self);
    pub fn zoom_to_selection(&mut self);
    /// In canvas units: left, top, width, height.
    pub fn visible(&self) -> Option<(f32, f32, f32, f32)>;
    pub fn center_on(&mut self, at: (f32, f32));
    /// The canvas point under the middle of the view.
    pub fn center(&self) -> (i64, i64);
    pub fn layout(&self) -> Layout;
    /// Switching to a tree that grows another way drops every pin.
    pub fn set_layout(&mut self, layout: Layout);
    pub fn drag(&self) -> DragHandler;
    pub fn set_drag(&mut self, handler: DragHandler);
    pub fn snap(&self) -> Snap;
    pub fn set_snap(&mut self, snap: Snap);
    /// Settle measured heights and the layout, `held` staying put. The view calls it, then `frame`, each frame.
    pub fn reflow(&mut self, measured: impl IntoIterator<Item = (String, i64)>, held: Option<&str>);
    pub fn frame(&mut self);
}

// canvas::mindmap — pure; the builders answer a Change to submit
pub fn layout(canvas: &mut Canvas);
/// Where layout would move each node, those already there left out.
pub fn arrange(canvas: &Canvas, held: Option<&str>, flow: Flow) -> Vec<(String, (i64, i64))>;
pub fn walk(canvas: &Canvas, id: &str, flow: Flow, arrow: Arrow) -> Option<String>;
pub fn child(canvas: &Canvas, parent: &str, node: Node) -> Option<Vec<Change>>;
pub fn sibling(canvas: &Canvas, of: &str, node: Node) -> Option<Vec<Change>>;
pub fn root(canvas: &Canvas, node: Node, at: (i64, i64)) -> Change;
pub fn remove(canvas: &Canvas, id: &str) -> Change;
pub fn reparent(canvas: &Canvas, ids: &[String], parent: &str) -> Option<Vec<Change>>;
pub fn detach(canvas: &Canvas, ids: &[String]) -> Option<Change>;
pub fn carry(canvas: &Canvas, ids: &[String], by: (i64, i64), pin: bool) -> Vec<Change>;

// canvas::snap — pure
pub fn settle(canvas: &Canvas, moving: &[String], to: (i64, i64), size: (i64, i64), snap: Snap, reach: i64) -> ((i64, i64), Vec<Guide>);

pub fn minimap(view: &Entity<CanvasView>, cx: &App) -> impl IntoElement;

// canvas::clip — pure
pub fn fragment(canvas: &Canvas, ids: &[String]) -> Canvas;
pub fn paste(canvas: &Canvas, fragment: &Canvas, at: (i64, i64), under: Option<&str>) -> Vec<Change>;

// canvas::change — pure
/// Answers the changes that undo it.
pub fn apply(canvas: &mut Canvas, change: &Change) -> Vec<Change>;
pub fn apply_all(canvas: &mut Canvas, changes: &[Change]) -> Vec<Change>;
/// The first node a batch adds.
pub fn added(changes: &[Change]) -> Option<&str>;
```

The source is at `apps/gallery/src/patterns/canvas.rs`. Copy the file.
