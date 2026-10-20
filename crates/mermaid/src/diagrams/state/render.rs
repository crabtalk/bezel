//! Pele's `diagrams/state/render.ts`.

use crate::svg::builder::rect;
use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::{
    graph::{Pattern, StateEdge, StateNode, StateShape},
    types::StateModel,
};
use crate::{
    Config, Ctx, Error, Link, Rendered,
    config::positive,
    diagrams::Render,
    geom::{Axis, Direction, Point, RoutePoint, Size},
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, compound_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{Esc, EscText, fragment, num, styled},
        cluster::Crossings,
        edges::{Curve, EdgePath, Marker, SideReach, edge_label_svg, route_path},
        root::svg_document,
        theme::{ResolvedStyle, class_names, resolve_style_over},
        title::with_title,
    },
    text::{Host, Label, decode_entities},
    util::url::{linked, safe_url},
};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 44.0;
const GROUP_PAD: f64 = 16.0;
const REGION_PAD: f64 = 10.0;
const PAD_X: f64 = 16.0;
const PAD_Y: f64 = 10.0;
const LOOP: f64 = 26.0;
const FOLD: f64 = 8.0;
const NOTE_WRAP: f64 = 320.0;
const CORNER: f64 = 10.0;
const SURFACE: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const RULE: &str = " fill=\"none\" stroke=\"var(--_b)\"";

static MERMAID_CLASSES: Lazy = Lazy::new(
    r"(?:^| )statediagram-(?:state|cluster-alt|cluster|note)(?= |$)",
    "g",
);

/// What an item of the drawing is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum K {
    State,
    Described,
    Start,
    End,
    Choice,
    Fork,
    Join,
    Note,
    Composite,
    Region,
    Hidden,
}

impl K {
    /// What `node` is drawn as; `None` for a note's group.
    fn of(node: &StateNode) -> Option<Self> {
        if node.shape == Some(StateShape::NoteGroup) {
            return None;
        }
        if node.is_group {
            return Some(if node.shape == Some(StateShape::Divider) {
                K::Region
            } else {
                K::Composite
            });
        }
        Some(match node.shape {
            Some(StateShape::RectWithTitle) => K::Described,
            Some(StateShape::StateStart) => K::Start,
            Some(StateShape::StateEnd) => K::End,
            Some(StateShape::Choice) => K::Choice,
            Some(StateShape::Fork) => K::Fork,
            Some(StateShape::Join) => K::Join,
            Some(StateShape::Note) => K::Note,
            Some(StateShape::Divider) => K::Hidden,
            _ => K::State,
        })
    }

    /// The space-separated class names of an item that is drawn; `None` for
    /// one that is not.
    fn classes(self) -> Option<&'static str> {
        Some(match self {
            K::State | K::Described => "node state",
            K::Start => "node state-start",
            K::End => "node state-end",
            K::Choice => "node state-choice",
            K::Fork => "node state-fork",
            K::Join => "node state-join",
            K::Note => "note",
            K::Composite => "cluster state-composite",
            K::Region | K::Hidden => return None,
        })
    }
}

struct View<'a> {
    /// Absent for the unseen helpers the layout needs.
    node: Option<&'a StateNode>,
    kind: K,
    style: Option<ResolvedStyle>,
    label: Label,
    lines: Vec<Label>,
    size: Size,
    title_h: f64,
}

/// The transitions a state makes to itself: how many, their widest label,
/// and their labels' height.
#[derive(Clone, Copy, Default)]
struct Loops {
    count: i32,
    width: f64,
    height: f64,
}

/// The items of a drawing. Each state has a view; once sized, every view
/// has the layout box of the same index.
struct Scene<'a> {
    views: Vec<View<'a>>,
    cnodes: Vec<CNode>,
    no_label: Label,
    /// The margin the layout is drawn at.
    pad: f64,
}

/// Mermaid keeps the quotes of a click statement's strings and strips them
/// when it draws.
fn unquote(text: &str) -> &str {
    text.trim_matches('"')
}

/// Writes `open`, what `body` writes, and `</g>` to `out`; nothing when
/// `body` writes nothing.
fn wrapped(out: &mut String, open: impl fmt::Display, body: impl FnOnce(&mut String)) {
    let mark = out.len();
    let _ = write!(out, "{open}");
    let start = out.len();
    body(out);
    if out.len() == start {
        out.truncate(mark);
    } else {
        out.push_str("</g>");
    }
}

/// Each of the space-separated `names` with the `{PREFIX}-` before it.
fn prefixed(names: &'static str) -> impl fmt::Display {
    fragment(move |f| {
        for (k, name) in names.split(' ').enumerate() {
            if k > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}-{name}", crate::PREFIX)?;
        }
        Ok(())
    })
}

impl Render for StateModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(if self.direction().is_vertical() {
            self.draw(config, ctx, false)
        } else {
            ctx.turn_to_fit(|down| self.draw(config, ctx, down))
        })
    }
}

impl<'a> Scene<'a> {
    /// Adds the view of an item without a layout box yet.
    fn view(&mut self, node: Option<&'a StateNode>, kind: K) -> usize {
        self.views.push(View {
            node,
            kind,
            style: None,
            label: self.no_label.clone(),
            lines: Vec::new(),
            size: Size::default(),
            title_h: 0.0,
        });
        self.views.len() - 1
    }

    /// Adds an unseen helper laid out as `c`.
    fn helper(&mut self, c: CNode) -> usize {
        let at = self.view(None, K::Hidden);
        self.cnodes.push(c);
        at
    }

    fn node(&self, i: usize) -> &'a StateNode {
        self.views[i].node.expect("a state's node")
    }

    /// The direction inside a composite; top to bottom where it sets none,
    /// whatever surrounds it.
    fn inner_dir(&self, composite: usize) -> Direction {
        self.node(composite).dir.unwrap_or_default()
    }

    /// The low and high ends of item `i` along `axis`.
    fn extent(&self, i: usize, axis: Axis) -> (f64, f64) {
        let c = &self.cnodes[i];
        (
            c.center.along(axis) - c.size.along(axis) / 2.0,
            c.center.along(axis) + c.size.along(axis) / 2.0,
        )
    }

    /// Draws the dashed lines between the regions among `items`, in the box
    /// from `min` to `max`.
    fn divide(&self, items: &[usize], min: Point, max: Point, out: &mut String) {
        let p = crate::PREFIX;
        let pad = self.pad;
        let regions: Vec<usize> = items
            .iter()
            .copied()
            .filter(|&i| self.views[i].kind == K::Region)
            .collect();
        if regions.len() < 2 {
            return;
        }
        for axis in [Axis::X, Axis::Y] {
            let lo = |i: usize| self.extent(i, axis).0;
            let hi = |i: usize| self.extent(i, axis).1;
            let mut sorted = regions.clone();
            sorted.sort_by(|&a, &b| js::compare(lo(a) - lo(b)).then(a.cmp(&b)));
            let apart = (1..sorted.len()).all(|k| hi(sorted[k - 1]) <= lo(sorted[k]) + 0.5);
            if !apart {
                continue;
            }
            for k in 1..sorted.len() {
                let at = num((hi(sorted[k - 1]) + lo(sorted[k])) / 2.0 + pad);
                let _ = match axis {
                    Axis::X => write!(
                        out,
                        "<path class=\"{p}-divider\" d=\"M{at},{}V{}\"/>",
                        num(min.y),
                        num(max.y)
                    ),
                    Axis::Y => write!(
                        out,
                        "<path class=\"{p}-divider\" d=\"M{},{at}H{}\"/>",
                        num(min.x),
                        num(max.x)
                    ),
                };
            }
            return;
        }
        // Regions that sit neither side by side nor in a stack each get an
        // outline.
        for &i in &regions {
            let c = &self.cnodes[i];
            let _ = write!(
                out,
                "{}",
                rect(
                    c.corner() + Point::new(pad, pad),
                    c.size,
                    format_args!(" class=\"{p}-divider\" rx=\"{}\"", num(CORNER)),
                )
            );
        }
    }

    /// Moves end `at` of `route`, on the layout box of item `i`, onto the
    /// drawn shape, and spreads the ends along a bar.
    fn attach(&self, route: &mut [RoutePoint], at: usize, i: usize) {
        let c = &self.cnodes[i];
        if c.is_group {
            return;
        }
        let view = &self.views[i];
        let bar = view.kind == K::Fork || view.kind == K::Join;
        let other = route[if at == 0 { 1 } else { at - 1 }].at;
        let end = &mut route[at].at;
        // The end sits on a side that faces along `axis`.
        for axis in [Axis::Y, Axis::X] {
            let across = axis.other();
            let extent = c.size.along(axis);
            let drawn_extent = view.size.along(axis);
            let breadth = view.size.along(across);
            let center = c.center.along(axis) + self.pad;
            let cross = c.center.along(across) + self.pad;
            let delta = end.along(axis) - center;
            if (delta.abs() - extent / 2.0).abs() >= 0.5 {
                continue;
            }
            *end.along_mut(axis) -= (js::sign(delta) * (extent - drawn_extent)) / 2.0;
            if bar && breadth > drawn_extent {
                *end.along_mut(across) = other
                    .along(across)
                    .clamp(cross - breadth / 2.0 + 6.0, cross + breadth / 2.0 - 6.0);
            }
            return;
        }
    }
}

impl StateModel {
    /// `turned` draws a diagram that runs across as one that runs down.
    fn draw(&self, config: &Config, ctx: &Ctx<'_, impl Host>, turned: bool) -> Rendered {
        let p = crate::PREFIX;
        let graph = &self.graph;
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let measure = ctx.host;
        let wrap_width = positive(config.wrapping_width, 200.0);
        let pad = ctx.options.padding.unwrap_or(8.0);
        let root_dir = if turned {
            Direction::TopToBottom
        } else {
            self.direction()
        };
        let text = |raw: Option<&str>, font_size: f64, wrap: f64, style: u8| {
            Label::layout(raw, true, measure, font_size, wrap, style)
        };

        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut scene = Scene {
            views: Vec::new(),
            cnodes: Vec::new(),
            no_label: Label::layout(None, false, measure, size, 0.0, 0),
            pad,
        };
        for node in &graph.nodes {
            let Some(kind) = K::of(node) else {
                continue;
            };
            let at = scene.view(Some(node), kind);
            index.insert(node.id.as_str(), at);
        }
        let states = scene.views.len();

        // The composite each state sits in.
        let mut logical: Vec<Option<usize>> = (0..states)
            .map(|i| {
                scene
                    .node(i)
                    .parent_id
                    .as_deref()
                    .and_then(|id| index.get(id).copied())
                    .filter(|&parent| parent != i && scene.node(parent).is_group)
            })
            .collect();
        // Composites that are redeclared inside each other form a ring with no
        // way in from the root. One member of each ring moves to the root,
        // which leaves a tree.
        {
            let mut kids: Vec<Vec<usize>> = vec![Vec::new(); states];
            let mut queue: Vec<usize> = Vec::new();
            let mut placed = vec![false; states];
            for (i, parent) in logical.iter().enumerate() {
                match parent {
                    None => {
                        placed[i] = true;
                        queue.push(i);
                    }
                    Some(parent) => kids[*parent].push(i),
                }
            }
            let mut done = 0;
            let settle = |queue: &mut Vec<usize>, placed: &mut Vec<bool>, done: &mut usize| {
                while *done < queue.len() {
                    for &kid in &kids[queue[*done]] {
                        if placed[kid] {
                            continue;
                        }
                        placed[kid] = true;
                        queue.push(kid);
                    }
                    *done += 1;
                }
            };
            settle(&mut queue, &mut placed, &mut done);
            let mut seen: Vec<Option<usize>> = vec![None; states];
            for i in 0..states {
                if placed[i] {
                    continue;
                }
                let mut ring = i;
                while seen[ring] != Some(i) {
                    seen[ring] = Some(i);
                    ring = logical[ring].expect("a state outside the tree has a parent");
                }
                logical[ring] = None;
                placed[ring] = true;
                queue.push(ring);
                settle(&mut queue, &mut placed, &mut done);
            }
        }
        // The state each note is for.
        let mut owners: Vec<Option<usize>> = vec![None; states];
        for i in 0..states {
            let owner = scene
                .node(i)
                .note_for
                .as_deref()
                .and_then(|id| index.get(id).copied());
            let Some(owner) = owner else {
                continue;
            };
            owners[i] = Some(owner);
            logical[i] = logical[owner];
        }

        // Whether the flow around a state runs top to bottom or bottom to top.
        let vertical = |scene: &Scene, i: usize| {
            logical[i]
                .map_or(root_dir, |composite| scene.inner_dir(composite))
                .is_vertical()
        };

        let mut noted = vec![false; states];
        // When each state first takes part in a transition. Layout follows this
        // order rather than the order of declaration, so a state declared up
        // front does not become the top of the flow.
        let mut appear: Vec<f64> = vec![f64::INFINITY; states];
        let mut appeared = 0.0;
        let mut outs = vec![0i32; states];
        let mut ins = vec![0i32; states];
        for edge in &graph.edges {
            let (Some(&src), Some(&dst)) =
                (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
            else {
                continue;
            };
            if edge.pattern == Some(Pattern::Dashed) {
                noted[if scene.views[src].kind == K::Note {
                    dst
                } else {
                    src
                }] = true;
                continue;
            }
            outs[src] += 1;
            ins[dst] += 1;
            if appear[src] == f64::INFINITY {
                appear[src] = appeared;
                appeared += 1.0;
            }
            if appear[dst] == f64::INFINITY {
                appear[dst] = appeared;
                appeared += 1.0;
            }
        }

        for i in 0..states {
            let node = scene.node(i);
            let style = resolve_style_over(&node.css_compiled_styles, &node.css_styles);
            let text_style = style.label_style();
            let font_size = style.font_size.unwrap_or(size);
            let is_vertical = vertical(&scene, i);
            let inner = node.is_group.then(|| scene.inner_dir(i));
            let view = &mut scene.views[i];
            view.style = Some(style);
            let mut c: Option<CNode> = None;
            match view.kind {
                K::Composite => {
                    view.label = text(node.label.as_deref(), font_size, 4000.0, text_style);
                    view.title_h = if view.label.height > 0.0 {
                        view.label.height + 12.0
                    } else {
                        0.0
                    };
                    let mut g = CNode::group(
                        Some(inner.expect("a composite's direction")),
                        GROUP_PAD,
                        view.title_h + GROUP_PAD,
                        GROUP_PAD,
                    );
                    g.min_w = view.label.width + 2.0 * GROUP_PAD;
                    c = Some(g);
                }
                K::Region => {
                    c = Some(CNode::group(
                        Some(inner.expect("a region's direction")),
                        REGION_PAD,
                        REGION_PAD,
                        REGION_PAD,
                    ));
                }
                K::Start => view.size = Size::new(14.0, 14.0),
                K::End => view.size = Size::new(18.0, 18.0),
                K::Choice => view.size = Size::new(30.0, 30.0),
                K::Fork | K::Join => {
                    let degree = outs[i].max(ins[i]);
                    let length = (40.0 * f64::from(degree) + 16.0).clamp(72.0, 320.0);
                    view.size = if is_vertical {
                        Size::new(length, 8.0)
                    } else {
                        Size::new(8.0, length)
                    };
                }
                K::Hidden => {}
                K::Note => {
                    view.label = text(node.label.as_deref(), small, wrap_width.max(NOTE_WRAP), 0);
                    view.size = Size::new(view.label.width + 24.0, view.label.height + 16.0);
                }
                K::State | K::Described => {
                    view.label = text(node.label.as_deref(), font_size, wrap_width, text_style);
                    let mut width = view.label.width;
                    let mut height = view.label.height + 2.0 * PAD_Y;
                    if view.kind == K::Described {
                        height += 14.0;
                        for line in &node.description {
                            let label = text(
                                Some(if line.is_empty() { " " } else { line }),
                                small,
                                wrap_width,
                                text_style,
                            );
                            if label.width > width {
                                width = label.width;
                            }
                            height += label.height;
                            view.lines.push(label);
                        }
                    }
                    view.size = Size::new((width + 2.0 * PAD_X).max(56.0), height);
                }
            }
            let mut c = c.unwrap_or_else(|| CNode::new(view.size, None));
            // The transitions on one side of a state's box spread along it,
            // clear of its corners.
            if view.kind == K::State || view.kind == K::Described {
                c.span = view.size.along(if is_vertical { Axis::X } else { Axis::Y }) - 24.0;
            }
            scene.cnodes.push(c);
        }

        let mut cedges: Vec<CEdge> = Vec::new();
        let mut edge_labels: Vec<Label> = Vec::new();
        let mut drawn: Vec<&StateEdge> = Vec::new();
        let mut loops = vec![Loops::default(); states];
        // A transition between a composite and a state inside it runs between
        // that state and a point just inside the composite, and is then drawn
        // on to the composite's border.
        let mut anchors: HashMap<usize, usize> = HashMap::new();
        let encloses = |cnodes: &[CNode], composite: usize, item: usize| {
            cnodes[composite].is_group
                && std::iter::successors(logical[item], |&p| logical[p]).any(|p| p == composite)
        };
        for edge in &graph.edges {
            let (Some(&src0), Some(&dst0)) =
                (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
            else {
                continue;
            };
            let (mut src, mut dst) = (src0, dst0);
            let label = text(Some(&edge.label), small, wrap_width, 0);
            let room = CEdge::label_room(&label);
            if src == dst {
                let l = &mut loops[src];
                l.count += 1;
                l.width = l.width.max(room.w);
                l.height += label.height;
            } else if encloses(&scene.cnodes, src, dst) {
                anchors.insert(cedges.len(), src);
                appear.push(-1.0);
                src = scene.helper(CNode::new(Size::default(), Some(src)));
            } else if encloses(&scene.cnodes, dst, src) {
                anchors.insert(cedges.len(), dst);
                appear.push(f64::INFINITY);
                dst = scene.helper(CNode::new(Size::default(), Some(dst)));
            }
            drawn.push(edge);
            cedges.push(CEdge {
                src,
                dst,
                minlen: 1,
                label: room,
                ..CEdge::default()
            });
            edge_labels.push(label);
        }

        // A state with notes, or a composite with self-transitions, sits in an
        // unseen group of its own: the notes go beside the state, across the
        // flow, without disturbing its transitions.
        for i in 0..states {
            if owners[i].is_some() {
                continue;
            }
            let is_vertical = vertical(&scene, i);
            let l = loops[i];
            // Loops hang off the right, or below when the flow is horizontal,
            // with their labels beyond them.
            let mut room = 0.0;
            if l.count > 0 {
                let reach = LOOP + f64::from(l.count - 1) * 8.0;
                room = if is_vertical {
                    reach + l.width
                } else {
                    reach * 0.75 + 4.0 + l.height
                };
                let item = &mut scene.cnodes[i];
                if !item.is_group {
                    if is_vertical {
                        item.size.w += 2.0 * room;
                        item.size.h = item.size.h.max(l.height);
                    } else {
                        item.size.h += 2.0 * room;
                        item.size.w = item.size.w.max(l.width);
                    }
                    room = 0.0;
                }
            }
            if !noted[i] && room == 0.0 {
                scene.cnodes[i].parent = logical[i];
                continue;
            }
            let mut c = if is_vertical {
                CNode::group(Some(Direction::LeftToRight), room, 0.0, 0.0)
            } else {
                CNode::group(Some(Direction::TopToBottom), 0.0, room, room)
            };
            c.parent = logical[i];
            appear.push(appear[i]);
            scene.cnodes[i].parent = Some(scene.helper(c));
        }
        for (i, owner) in owners.iter().enumerate() {
            if let Some(owner) = *owner {
                scene.cnodes[i].parent = scene.cnodes[owner].parent;
            }
        }

        {
            // Numbers the tree so that, at every level, initial states come
            // first and the rest follow in the order the transitions introduce
            // them.
            let mut members: Vec<Vec<usize>> = vec![Vec::new(); scene.cnodes.len()];
            let mut top: Vec<usize> = Vec::new();
            for (i, c) in scene.cnodes.iter().enumerate() {
                match c.parent {
                    Some(p) => members[p].push(i),
                    None => top.push(i),
                }
            }
            let initial = |i: usize| {
                if scene.views[i].kind == K::Start {
                    0
                } else {
                    1
                }
            };
            let by_flow = |a: &usize, b: &usize| {
                let (a, b) = (*a, *b);
                initial(a)
                    .cmp(&initial(b))
                    .then(
                        appear[a]
                            .partial_cmp(&appear[b])
                            .unwrap_or(Ordering::Greater),
                    )
                    .then(a.cmp(&b))
            };
            let mut seq = 0.0;
            top.sort_by(by_flow);
            top.reverse();
            let mut pending = top;
            while let Some(i) = pending.pop() {
                scene.cnodes[i].seq = seq;
                seq += 1.0;
                let mut inside = std::mem::take(&mut members[i]);
                inside.sort_by(by_flow);
                for &k in inside.iter().rev() {
                    pending.push(k);
                }
            }
        }

        let layout = compound_layout(
            &mut scene.cnodes,
            &mut cedges,
            root_dir,
            &LayeredOptions {
                node_sep: positive(config.node_spacing, NODE_SEP),
                edge_sep: EDGE_SEP,
                rank_sep: positive(config.rank_spacing, RANK_SEP),
                port_sep: 20.0,
                head_room: Some(Marker::Arrow.trim()),
                ..LayeredOptions::default()
            },
        );

        let width = (layout.w + 2.0 * pad).ceil();
        let height = (layout.h + 2.0 * pad).ceil();
        let mut links: Vec<Link> = Vec::new();

        // Route points by rounded y, to find the edges that cross a
        // composite's title bar.
        let mut crossings = Crossings::new();
        for e in &cedges {
            crossings.mark_bends(&e.route);
        }

        let mut children: Vec<Vec<usize>> = vec![Vec::new(); scene.cnodes.len()];
        let mut order: Vec<usize> = Vec::new();
        for (i, c) in scene.cnodes.iter().enumerate() {
            match c.parent {
                Some(p) => children[p].push(i),
                None => order.push(i),
            }
        }
        let roots = order.clone();
        let mut q = 0;
        while q < order.len() {
            for &child in &children[order[q]] {
                order.push(child);
            }
            q += 1;
        }

        let mut content = String::new();
        let mut nodes_out = String::new();
        let mut notes_out = String::new();
        wrapped(
            &mut content,
            format_args!("<g class=\"{p}-clusters\">"),
            |clusters| {
                for &i in &order {
                    let view = &scene.views[i];
                    let (Some(node), Some(style)) = (view.node, view.style.as_ref()) else {
                        continue;
                    };
                    let Some(kind_classes) = view.kind.classes() else {
                        continue;
                    };
                    let c = &scene.cnodes[i];
                    let out = match view.kind {
                        K::Composite => &mut *clusters,
                        K::Note => &mut notes_out,
                        _ => &mut nodes_out,
                    };
                    let _ = write!(
                        out,
                        "<g class=\"{}{}\" data-id=\"{}\"",
                        prefixed(kind_classes),
                        class_names(&MERMAID_CLASSES.replace(&node.css_classes, |_| String::new())),
                        EscText(&node.id)
                    );
                    if view.kind != K::Composite {
                        let _ = write!(
                            out,
                            " transform=\"translate({},{})\"",
                            num(c.center.x + pad),
                            num(c.center.y + pad)
                        );
                    }
                    out.push('>');
                    let body = view.body(c, style, &crossings, pad, small);
                    if let Some(link) = self.links.get(&node.id) {
                        // Entity codes are resolved first, so the URL is
                        // checked in the form a browser would follow.
                        let tooltip = decode_entities(unquote(&link.tooltip));
                        let mut inner = String::new();
                        if !tooltip.is_empty() {
                            let _ = write!(inner, "<title>{}</title>", Esc(&tooltip));
                        }
                        let _ = write!(inner, "{body}");
                        let _ = write!(
                            out,
                            "{}",
                            linked(
                                &inner,
                                Some(&safe_url(unquote(&link.url))),
                                &node.id,
                                &mut links,
                                ctx,
                                " target=\"_blank\"",
                            )
                        );
                    } else {
                        let _ = write!(out, "{body}");
                    }
                    out.push_str("</g>");
                }
            },
        );
        wrapped(
            &mut content,
            format_args!("<g class=\"{p}-dividers\" stroke-dasharray=\"4 4\"{RULE}>"),
            |out| {
                scene.divide(
                    &roots,
                    Point::new(pad / 2.0, pad / 2.0),
                    Point::new(width - pad / 2.0, height - pad / 2.0),
                    out,
                );
                for &i in &order {
                    let view = &scene.views[i];
                    if view.kind != K::Composite {
                        continue;
                    }
                    let c = &scene.cnodes[i];
                    let left = c.center.x + pad - c.size.w / 2.0;
                    let top = c.center.y + pad - c.size.h / 2.0;
                    scene.divide(
                        &children[i],
                        Point::new(left, top + view.title_h),
                        Point::new(left + c.size.w, top + c.size.h),
                        out,
                    );
                }
            },
        );

        let mut reach = SideReach::new();
        for (i, ce) in cedges.iter().enumerate() {
            if ce.src == ce.dst || ce.route.len() < 2 || drawn[i].pattern == Some(Pattern::Dashed) {
                continue;
            }
            reach.add(
                ce.dst,
                scene.cnodes[ce.dst].side_of(ce.route[ce.route.len() - 1].at),
                Some(Marker::Arrow),
            );
        }

        let mut loop_seen = vec![0i32; states];
        let mut loop_text = vec![0.0f64; states];
        wrapped(
            &mut content,
            format_args!(
                "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">"
            ),
            |out| {
                for i in 0..cedges.len() {
                    let edge = drawn[i];
                    let label = &edge_labels[i];
                    let note = edge.pattern == Some(Pattern::Dashed);
                    let end_marker = if note { None } else { Some(Marker::Arrow) };
                    let path: EdgePath;
                    let (src, dst) = (cedges[i].src, cedges[i].dst);
                    if src == dst {
                        let a = &scene.cnodes[src];
                        let view = &scene.views[src];
                        let k = f64::from(loop_seen[src]);
                        loop_seen[src] += 1;
                        // The loop hangs off a side the flow does not use: the
                        // right, or below when the flow is horizontal.
                        let side = vertical(&scene, src);
                        let Size { w, h } = if a.is_group { a.size } else { view.size };
                        let out = (if side { w } else { h }) / 2.0;
                        let spread = ((if side { h } else { w }) / 2.0 - 4.0)
                            .min(8.0 + k * 6.0)
                            .max(2.0);
                        let reach = LOOP + k * 8.0;
                        let trim = Marker::trim_of(end_marker) / js::hypot(reach, spread);
                        // Offsets are given for a loop on the right and turned
                        // a quarter for one below.
                        let Point { x: ax, y: ay } = a.center;
                        let px = |dx: f64, dy: f64| ax + pad + if side { out + dx } else { -dy };
                        let py = |dx: f64, dy: f64| ay + pad + if side { dy } else { out + dx };
                        let at = |dx: f64, dy: f64| Point::new(px(dx, dy), py(dx, dy));
                        let mut lp = EdgePath {
                            d: format!(
                                "M{}C{} {} {}",
                                at(0.0, -spread),
                                at(reach, -2.0 * spread),
                                at(reach, 2.0 * spread),
                                at(reach * trim, spread + spread * trim)
                            ),
                            start: Point::default(),
                            start_dir: Point::default(),
                            end: at(0.0, spread),
                            end_dir: Point::new(
                                px(-reach, -spread) - px(0.0, 0.0),
                                py(-reach, -spread) - py(0.0, 0.0),
                            ),
                        };
                        let len = js::or_one(js::hypot(lp.end_dir.x, lp.end_dir.y));
                        lp.end_dir.x /= len;
                        lp.end_dir.y /= len;
                        path = lp;
                        // Labels stack beyond the outermost loop, in the order
                        // of their transitions.
                        let l = loops[src];
                        let furthest = LOOP + f64::from(l.count - 1) * 8.0;
                        let below = loop_text[src] + label.height / 2.0;
                        loop_text[src] += label.height;
                        let ce = &mut cedges[i];
                        ce.label_at = if side {
                            Point::new(
                                ax + out + furthest + 4.0 + ce.label.w / 2.0,
                                ay - l.height / 2.0 + below,
                            )
                        } else {
                            Point::new(ax, ay + out + furthest * 0.75 + 4.0 + below)
                        };
                    } else {
                        let mut route = cedges[i].route.clone();
                        let leaves = reach.of(src, scene.cnodes[src].side_of(route[0].at));
                        let arrives =
                            reach.of(dst, scene.cnodes[dst].side_of(route[route.len() - 1].at));
                        for point in &mut route {
                            point.at += Point::new(pad, pad);
                        }
                        scene.attach(&mut route, 0, src);
                        let last = route.len() - 1;
                        scene.attach(&mut route, last, dst);
                        if let Some(&composite) = anchors.get(&i)
                            && route.len() >= 2
                        {
                            let g = &scene.cnodes[composite];
                            let inward = scene.views[src].kind == K::Hidden;
                            let at = route[if inward { 0 } else { route.len() - 1 }].at;
                            let near = matches!(
                                g.dir,
                                Some(Direction::TopToBottom | Direction::LeftToRight)
                            ) == inward;
                            let point = if g.dir.is_some_and(Direction::is_vertical) {
                                RoutePoint::new(
                                    Point::new(
                                        at.x,
                                        g.center.y
                                            + pad
                                            + if near {
                                                scene.views[composite].title_h - g.size.h / 2.0
                                            } else {
                                                g.size.h / 2.0
                                            },
                                    ),
                                    Axis::Y,
                                )
                            } else {
                                RoutePoint::new(
                                    Point::new(
                                        g.center.x
                                            + pad
                                            + if near {
                                                -g.size.w / 2.0
                                            } else {
                                                g.size.w / 2.0
                                            },
                                        at.y,
                                    ),
                                    Axis::X,
                                )
                            };
                            if inward {
                                route.insert(0, point);
                            } else {
                                route.push(point);
                            }
                        }
                        path = route_path(
                            &route,
                            Curve::Smooth,
                            0.0,
                            Marker::trim_of(end_marker),
                            Some(leaves),
                            Some(arrives),
                        );
                    }
                    let _ = write!(
                        out,
                        "<g class=\"{p}-edge {p}-{}\" data-id=\"{}\"><path d=\"{}\"/>{}</g>",
                        if note {
                            "note-edge\" stroke=\"var(--_b)\" stroke-dasharray=\"3 4"
                        } else {
                            "transition"
                        },
                        EscText(&edge.id),
                        path.d,
                        Marker::draw_of(end_marker, path.end, path.end_dir, "var(--_l)")
                    );
                }
            },
        );
        wrapped(
            &mut content,
            format_args!("<g class=\"{p}-edge-labels\" font-size=\"{}\">", num(small)),
            |out| {
                for (i, ce) in cedges.iter().enumerate() {
                    let label = &edge_labels[i];
                    if label.width > 0.0 {
                        let _ = write!(
                            out,
                            "{}",
                            edge_label_svg(
                                &drawn[i].id,
                                label,
                                ce.label_at + Point::new(pad, pad),
                                "",
                            )
                        );
                    }
                }
            },
        );
        let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}{notes_out}</g>");
        let titled = with_title(
            self.titles.title.as_deref(),
            measure,
            size,
            pad,
            width,
            height,
            content,
        );
        let svg = svg_document(
            "state",
            titled.width,
            titled.height,
            size,
            ctx,
            self.titles.accessible(),
            &titled.content,
        );

        Rendered {
            svg,
            width: titled.width,
            height: titled.height,
            links,
        }
    }
}

impl View<'_> {
    /// The drawing of a state or note, centred on its origin, or of a
    /// composite in place, laid out as `c` and drawn at a margin of `pad`.
    fn body<'v>(
        &'v self,
        c: &'v CNode,
        style: &'v ResolvedStyle,
        crossings: &'v Crossings,
        pad: f64,
        small: f64,
    ) -> impl fmt::Display + 'v {
        let p = crate::PREFIX;
        let Size { w, h } = self.size;
        fragment(move |f| match self.kind {
            K::Composite => {
                let left = c.center.x + pad - c.size.w / 2.0;
                let top = c.center.y + pad - c.size.h / 2.0;
                write!(
                    f,
                    "{}",
                    rect(
                        Point::new(left, top),
                        c.size,
                        format_args!(
                            " rx=\"{}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}",
                            num(CORNER),
                            style.shape
                        ),
                    )
                )?;
                if self.title_h > 0.0 {
                    write!(
                        f,
                        "<path d=\"M{},{}H{}\"{RULE}{}/>{}",
                        num(left),
                        num(top + self.title_h),
                        num(left + c.size.w),
                        style.line,
                        self.label.svg(
                            Point::new(
                                crossings.centred_title_x(c, self.label.width, GROUP_PAD) + pad,
                                top + self.title_h / 2.0
                            ),
                            &styled(crate::class!("cluster-label"), &style.text)
                        )
                    )?;
                }
                Ok(())
            }
            K::Start => write!(f, "<circle r=\"7\" fill=\"var(--_l)\"{}/>", style.shape),
            K::End => write!(
                f,
                "<circle r=\"8.5\" fill=\"var(--_bg)\" stroke=\"var(--_l)\"{}/><circle r=\"5\" fill=\"var(--_l)\"{}/>",
                style.line, style.shape
            ),
            K::Choice => write!(
                f,
                "<polygon points=\"0,{} {},0 0,{} {},0\"{SURFACE}{}/>",
                num(-h / 2.0),
                num(w / 2.0),
                num(h / 2.0),
                num(-w / 2.0),
                style.shape
            ),
            K::Fork | K::Join => write!(
                f,
                "{}",
                rect(
                    Point::new(-w / 2.0, -h / 2.0),
                    self.size,
                    format_args!(" rx=\"2\" fill=\"var(--_l)\"{}", style.shape),
                )
            ),
            K::Note => {
                let r = num(w / 2.0);
                let b = num(h / 2.0);
                let fold = num(w / 2.0 - FOLD);
                let corner = num(FOLD - h / 2.0);
                write!(
                    f,
                    "<path d=\"M-{r},-{b}H{fold}L{r},{corner}V{b}H-{r}Z\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"/><path d=\"M{fold},-{b}V{corner}H{r}\"{RULE}/>{}",
                    self.label.svg(
                        Point::new(0.0, 0.0),
                        &format!(
                            " class=\"{p}-label\" font-size=\"{}\" fill=\"var(--_m)\"",
                            num(small)
                        )
                    )
                )
            }
            _ => {
                write!(
                    f,
                    "{}",
                    rect(
                        Point::new(-w / 2.0, -h / 2.0),
                        self.size,
                        format_args!(
                            " rx=\"{}\"{SURFACE}{}",
                            num(CORNER.min(h / 2.0)),
                            style.shape
                        ),
                    )
                )?;
                let label_attrs = styled(crate::class!("label"), &style.text);
                if self.kind == K::State {
                    return write!(f, "{}", self.label.svg(Point::new(0.0, 0.0), &label_attrs));
                }
                let mut at = PAD_Y - h / 2.0 + self.label.height;
                write!(
                    f,
                    "<path d=\"M{},{}H{}\"{RULE}{}/>{}",
                    num(-w / 2.0),
                    num(at + 6.0),
                    num(w / 2.0),
                    style.line,
                    self.label
                        .svg(Point::new(0.0, at - self.label.height / 2.0), &label_attrs)
                )?;
                at += 14.0;
                let line_attrs = format!(
                    " class=\"{p}-state-description\" font-size=\"{}\"{}",
                    num(small),
                    style.text
                );
                for line in &self.lines {
                    write!(
                        f,
                        "{}",
                        line.svg(Point::new(0.0, at + line.height / 2.0), &line_attrs)
                    )?;
                    at += line.height;
                }
                Ok(())
            }
        })
    }
}
