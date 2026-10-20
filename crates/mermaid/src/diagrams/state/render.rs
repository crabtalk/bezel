//! Pele's `diagrams/state/render.ts`.

use crate::geom::{Axis, Direction, Point, RoutePoint, Size};
use crate::svg::builder::styled;
use crate::svg::builder::{Esc, EscText, fragment};
use crate::svg::cluster::Crossings;
use crate::svg::edges::Curve;
use crate::svg::edges::Marker;
use crate::{Config, config::positive};
use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::{
    graph::{StateEdge, StateNode},
    types::StateModel,
};
use crate::Ctx;
use crate::{
    Link, Rendered,
    diagrams::common::fit_width::{runs_across, turn_to_fit},
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, compound_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::num,
        edges::{EdgePath, SideReach, edge_label_svg, route_path},
        root::{Accessible, svg_document},
        theme::{ResolvedStyle, class_names, resolve_style},
        title::with_title,
    },
    text::{Host, Label, Style, decode_entities},
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
const CLASSES: [&str; 9] = [
    "node state",
    "node state",
    "node state-start",
    "node state-end",
    "node state-choice",
    "node state-fork",
    "node state-join",
    "note",
    "cluster state-composite",
];

static MERMAID_CLASSES: Lazy = Lazy::new(
    r"(?:^| )statediagram-(?:state|cluster-alt|cluster|note)(?= |$)",
    "g",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

fn kind_of(shape: Option<&str>) -> K {
    match shape {
        Some("rectWithTitle") => K::Described,
        Some("stateStart") => K::Start,
        Some("stateEnd") => K::End,
        Some("choice") => K::Choice,
        Some("fork") => K::Fork,
        Some("join") => K::Join,
        Some("note") => K::Note,
        Some("divider") => K::Hidden,
        _ => K::State,
    }
}

struct View<'a> {
    /// Absent for the unseen helpers the layout needs.
    node: Option<&'a StateNode>,
    kind: K,
    style: Option<ResolvedStyle>,
    label: Label,
    lines: Vec<Label>,
    w: f64,
    h: f64,
    title_h: f64,
}

fn vertical(dir: Option<Direction>) -> bool {
    matches!(dir, Some(Direction::TopToBottom | Direction::BottomToTop))
}

/// Mermaid keeps the quotes of a click statement's strings and strips them
/// when it draws.
fn unquote(text: &str) -> &str {
    text.trim_matches('"')
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64, rest: impl fmt::Display) -> impl fmt::Display {
    fragment(move |f| {
        write!(
            f,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rest}/>",
            num(x),
            num(y),
            num(w),
            num(h)
        )
    })
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

/// The `{PREFIX}-` classes of a kind of state.
fn classes(kind: K) -> impl fmt::Display {
    fragment(move |f| {
        for (k, name) in CLASSES[kind as usize].split(' ').enumerate() {
            if k > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}-{name}", crate::PREFIX)?;
        }
        Ok(())
    })
}

fn group(dir: Direction, pad_x: f64, pad_top: f64, pad_bottom: f64) -> CNode {
    let mut c = CNode::new(Size::default(), None);
    c.is_group = true;
    c.dir = Some(dir);
    c.pad_x = pad_x;
    c.pad_top = pad_top;
    c.pad_bottom = pad_bottom;
    c
}

pub fn render_state(db: &StateModel, config: &Config, ctx: &Ctx<'_, impl Host>) -> Rendered {
    if !runs_across(Some(db.direction())) {
        return draw(db, config, ctx, false);
    }
    turn_to_fit(ctx, |down| draw(db, config, ctx, down))
}

/// Draws the dashed lines between the regions among a composite's members.
#[allow(clippy::too_many_arguments)]
fn divide(
    items: &[usize],
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    cnodes: &[CNode],
    views: &[View],
    pad: f64,
    out: &mut String,
) {
    let p = crate::PREFIX;
    let regions: Vec<usize> = items
        .iter()
        .copied()
        .filter(|&i| views[i].kind == K::Region)
        .collect();
    if regions.len() < 2 {
        return;
    }
    let lo = |i: usize, x: bool| {
        if x {
            cnodes[i].center.x - cnodes[i].size.w / 2.0
        } else {
            cnodes[i].center.y - cnodes[i].size.h / 2.0
        }
    };
    let hi = |i: usize, x: bool| {
        if x {
            cnodes[i].center.x + cnodes[i].size.w / 2.0
        } else {
            cnodes[i].center.y + cnodes[i].size.h / 2.0
        }
    };
    for x in [true, false] {
        let mut sorted = regions.clone();
        sorted.sort_by(|&a, &b| js::compare(lo(a, x) - lo(b, x)).then(a.cmp(&b)));
        let apart = (1..sorted.len()).all(|k| hi(sorted[k - 1], x) <= lo(sorted[k], x) + 0.5);
        if !apart {
            continue;
        }
        for k in 1..sorted.len() {
            let at = num((hi(sorted[k - 1], x) + lo(sorted[k], x)) / 2.0 + pad);
            let _ = if x {
                write!(
                    out,
                    "<path class=\"{p}-divider\" d=\"M{at},{}V{}\"/>",
                    num(top),
                    num(bottom)
                )
            } else {
                write!(
                    out,
                    "<path class=\"{p}-divider\" d=\"M{},{at}H{}\"/>",
                    num(left),
                    num(right)
                )
            };
        }
        return;
    }
    // Regions that sit neither side by side nor in a stack each get an
    // outline.
    for &i in &regions {
        let c = &cnodes[i];
        let _ = write!(
            out,
            "{}",
            rect(
                c.center.x - c.size.w / 2.0 + pad,
                c.center.y - c.size.h / 2.0 + pad,
                c.size.w,
                c.size.h,
                format_args!(" class=\"{p}-divider\" rx=\"{}\"", num(CORNER)),
            )
        );
    }
}

/// Moves a route end from the layout box onto the drawn shape, and spreads
/// the ends along a bar.
fn attach(
    route: &mut [RoutePoint],
    at: usize,
    i: usize,
    cnodes: &[CNode],
    views: &[View],
    pad: f64,
) {
    let c = &cnodes[i];
    if c.is_group {
        return;
    }
    let view = &views[i];
    let bar = view.kind == K::Fork || view.kind == K::Join;
    let other = route[if at == 0 { 1 } else { at - 1 }].at;
    let end = &mut route[at].at;
    // The end sits on a side that faces along `axis`.
    for axis in [Axis::Y, Axis::X] {
        let y = axis == Axis::Y;
        let extent = if y { c.size.h } else { c.size.w };
        let drawn_extent = if y { view.h } else { view.w };
        let breadth = if y { view.w } else { view.h };
        let center = c.center.along(axis) + pad;
        let cross = c.center.along(axis.other()) + pad;
        let delta = end.along(axis) - center;
        if (delta.abs() - extent / 2.0).abs() >= 0.5 {
            continue;
        }
        *end.along_mut(axis) -= (js::sign(delta) * (extent - drawn_extent)) / 2.0;
        if bar && breadth > drawn_extent {
            *end.along_mut(axis.other()) = clamp(
                other.along(axis.other()),
                cross - breadth / 2.0 + 6.0,
                cross + breadth / 2.0 - 6.0,
            );
        }
        return;
    }
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(db: &StateModel, config: &Config, ctx: &Ctx<'_, impl Host>, turned: bool) -> Rendered {
    let p = crate::PREFIX;
    let graph = &db.graph;
    let size = ctx.options.font_size;
    let small = js::round(size * 0.875);
    let measure_rc = ctx.host;
    let measure = measure_rc;
    let wrap_width = positive(config.wrapping_width, 200.0);
    let pad = ctx.options.padding.unwrap_or(8.0);
    let root_dir = if turned {
        Direction::TopToBottom
    } else {
        Direction::from_name(db.direction())
    };
    let no_label = Label::layout(None, false, measure, size, 0.0, 0);
    let text = |raw: Option<&str>, font_size: f64, wrap: f64, style: u8| {
        Label::layout(raw, true, measure, font_size, wrap, style)
    };

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<View> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();
    fn add<'a>(
        views: &mut Vec<View<'a>>,
        cnodes: &mut Vec<CNode>,
        node: Option<&'a StateNode>,
        kind: K,
        c: Option<CNode>,
        no_label: &Label,
    ) -> usize {
        views.push(View {
            node,
            kind,
            style: None,
            label: no_label.clone(),
            lines: Vec::new(),
            w: 0.0,
            h: 0.0,
            title_h: 0.0,
        });
        if let Some(c) = c {
            cnodes.push(c);
        }
        views.len() - 1
    }
    for node in &graph.nodes {
        if node.shape.as_deref() == Some("noteGroup") {
            continue;
        }
        let kind = if node.is_group {
            if node.shape.as_deref() == Some("divider") {
                K::Region
            } else {
                K::Composite
            }
        } else {
            kind_of(node.shape.as_deref())
        };
        let at = add(&mut views, &mut cnodes, Some(node), kind, None, &no_label);
        index.insert(node.id.as_str(), at);
    }
    let states = views.len();

    let mut logical: Vec<i64> = Vec::with_capacity(states);
    for i in 0..states {
        let parent = node_of(&views, i)
            .parent_id
            .as_deref()
            .and_then(|id| index.get(id).copied());
        logical.push(match parent {
            Some(parent) if parent != i && node_of(&views, parent).is_group => parent as i64,
            _ => -1,
        });
    }
    // Composites that are redeclared inside each other form a ring with no
    // way in from the root. One member of each ring moves to the root, which
    // leaves a tree.
    {
        let mut kids: Vec<Vec<usize>> = vec![Vec::new(); states];
        let mut queue: Vec<usize> = Vec::new();
        let mut placed = vec![false; states];
        for i in 0..states {
            if logical[i] == -1 {
                placed[i] = true;
                queue.push(i);
            } else {
                kids[logical[i] as usize].push(i);
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
        let mut seen = vec![usize::MAX; states];
        for i in 0..states {
            if placed[i] {
                continue;
            }
            let mut ring = i;
            while seen[ring] != i {
                seen[ring] = i;
                ring = logical[ring] as usize;
            }
            logical[ring] = -1;
            placed[ring] = true;
            queue.push(ring);
            settle(&mut queue, &mut placed, &mut done);
        }
    }
    let mut owners: Vec<i64> = vec![-1; states];
    for i in 0..states {
        let owner = node_of(&views, i)
            .note_for
            .as_deref()
            .and_then(|id| index.get(id).copied());
        let Some(owner) = owner else {
            continue;
        };
        owners[i] = owner as i64;
        logical[i] = logical[owner];
    }

    // As in Mermaid, a composite that sets no direction runs top to bottom
    // whatever surrounds it.
    let inner_dir = |views: &[View], composite: usize| {
        node_of(views, composite)
            .dir
            .as_deref()
            .map(Direction::from_name)
            .unwrap_or_default()
    };
    // Whether the flow around a state runs top to bottom or bottom to top.
    let across = |views: &[View], logical: &[i64], i: usize| {
        vertical(Some(if logical[i] == -1 {
            root_dir
        } else {
            inner_dir(views, logical[i] as usize)
        }))
    };

    let mut degree = vec![0i32; states];
    let mut noted = vec![false; states];
    // When each state first takes part in a transition. Layout follows this
    // order rather than the order of declaration, so a state declared up
    // front does not become the top of the flow.
    let mut appear: Vec<f64> = vec![f64::INFINITY; states];
    let mut appeared = 0.0;
    let mut counts = vec![0i32; 2 * states];
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        if edge.pattern == Some("dashed") {
            noted[if views[src].kind == K::Note { dst } else { src }] = true;
            continue;
        }
        counts[2 * src] += 1;
        degree[src] = degree[src].max(counts[2 * src]);
        counts[2 * dst + 1] += 1;
        degree[dst] = degree[dst].max(counts[2 * dst + 1]);
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
        let node = node_of(&views, i);
        let style = if node.css_styles.is_empty() {
            resolve_style(&node.css_compiled_styles)
        } else {
            let mut all = node.css_compiled_styles.clone();
            all.extend(node.css_styles.iter().cloned());
            resolve_style(&all)
        };
        let text_style = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let font_size = style.font_size.unwrap_or(size);
        let is_across = across(&views, &logical, i);
        let inner = if node.is_group {
            Some(inner_dir(&views, i))
        } else {
            None
        };
        let view = &mut views[i];
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
                let mut g = group(
                    inner.expect("a composite's direction"),
                    GROUP_PAD,
                    view.title_h + GROUP_PAD,
                    GROUP_PAD,
                );
                g.min_w = view.label.width + 2.0 * GROUP_PAD;
                c = Some(g);
            }
            K::Region => {
                c = Some(group(
                    inner.expect("a region's direction"),
                    REGION_PAD,
                    REGION_PAD,
                    REGION_PAD,
                ));
            }
            K::Start => {
                view.w = 14.0;
                view.h = 14.0;
            }
            K::End => {
                view.w = 18.0;
                view.h = 18.0;
            }
            K::Choice => {
                view.w = 30.0;
                view.h = 30.0;
            }
            K::Fork | K::Join => {
                let length = clamp(40.0 * f64::from(degree[i]) + 16.0, 72.0, 320.0);
                view.w = if is_across { length } else { 8.0 };
                view.h = if is_across { 8.0 } else { length };
            }
            K::Hidden => {}
            K::Note => {
                view.label = text(node.label.as_deref(), small, wrap_width.max(NOTE_WRAP), 0);
                view.w = view.label.width + 24.0;
                view.h = view.label.height + 16.0;
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
                view.w = (width + 2.0 * PAD_X).max(56.0);
                view.h = height;
            }
        }
        let mut c = c.unwrap_or_else(|| CNode::new(Size::new(view.w, view.h), None));
        // The transitions on one side of a state's box spread along it, clear
        // of its corners.
        if view.kind == K::State || view.kind == K::Described {
            c.span = (if is_across { view.w } else { view.h }) - 24.0;
        }
        cnodes.push(c);
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&StateEdge> = Vec::new();
    // The transitions each state makes to itself: how many, their widest
    // label, and their labels' height.
    let mut loop_count = vec![0i32; states];
    let mut loop_width = vec![0.0f64; states];
    let mut loop_height = vec![0.0f64; states];
    // A transition between a composite and a state inside it runs between
    // that state and a point just inside the composite, and is then drawn on
    // to the composite's border.
    let mut anchors: HashMap<usize, usize> = HashMap::new();
    let encloses = |cnodes: &[CNode], composite: usize, item: usize| {
        if !cnodes[composite].is_group {
            return false;
        }
        let mut p = logical[item];
        while p != -1 {
            if p as usize == composite {
                return true;
            }
            p = logical[p as usize];
        }
        false
    };
    for edge in &graph.edges {
        let (Some(&src0), Some(&dst0)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        let (mut src, mut dst) = (src0, dst0);
        let label = text(Some(&edge.label), small, wrap_width, 0);
        let label_w = if label.width > 0.0 {
            label.width + 12.0
        } else {
            0.0
        };
        if src == dst {
            loop_count[src] += 1;
            loop_width[src] = loop_width[src].max(label_w);
            loop_height[src] += label.height;
        } else if encloses(&cnodes, src, dst) {
            anchors.insert(cedges.len(), src);
            appear.push(-1.0);
            src = add(
                &mut views,
                &mut cnodes,
                None,
                K::Hidden,
                Some(CNode::new(Size::default(), Some(src))),
                &no_label,
            );
        } else if encloses(&cnodes, dst, src) {
            anchors.insert(cedges.len(), dst);
            appear.push(f64::INFINITY);
            dst = add(
                &mut views,
                &mut cnodes,
                None,
                K::Hidden,
                Some(CNode::new(Size::default(), Some(dst))),
                &no_label,
            );
        }
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1,
            label: Size::new(
                label_w,
                if label.height > 0.0 {
                    label.height + 4.0
                } else {
                    0.0
                },
            ),
            ..CEdge::default()
        });
        edge_labels.push(label);
    }

    // A state with notes, or a composite with self-transitions, sits in an
    // unseen group of its own: the notes go beside the state, across the
    // flow, without disturbing its transitions.
    for i in 0..states {
        if owners[i] != -1 {
            continue;
        }
        let is_across = across(&views, &logical, i);
        // Loops hang off the right, or below when the flow is horizontal,
        // with their labels beyond them.
        let mut room = 0.0;
        if loop_count[i] > 0 {
            let reach = LOOP + f64::from(loop_count[i] - 1) * 8.0;
            room = if is_across {
                reach + loop_width[i]
            } else {
                reach * 0.75 + 4.0 + loop_height[i]
            };
            let item = &mut cnodes[i];
            if !item.is_group {
                if is_across {
                    item.size.w += 2.0 * room;
                    item.size.h = item.size.h.max(loop_height[i]);
                } else {
                    item.size.h += 2.0 * room;
                    item.size.w = item.size.w.max(loop_width[i]);
                }
                room = 0.0;
            }
        }
        if !noted[i] && room == 0.0 {
            cnodes[i].parent = usize::try_from(logical[i]).ok();
            continue;
        }
        let mut c = if is_across {
            group(Direction::LeftToRight, room, 0.0, 0.0)
        } else {
            group(Direction::TopToBottom, 0.0, room, room)
        };
        c.parent = usize::try_from(logical[i]).ok();
        appear.push(appear[i]);
        cnodes[i].parent = Some(add(
            &mut views,
            &mut cnodes,
            None,
            K::Hidden,
            Some(c),
            &no_label,
        ));
    }
    for i in 0..states {
        if owners[i] != -1 {
            cnodes[i].parent = cnodes[owners[i] as usize].parent;
        }
    }

    {
        // Numbers the tree so that, at every level, initial states come first
        // and the rest follow in the order the transitions introduce them.
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); cnodes.len()];
        let mut top: Vec<usize> = Vec::new();
        for (i, c) in cnodes.iter().enumerate() {
            match c.parent {
                Some(p) => members[p].push(i),
                None => top.push(i),
            }
        }
        let initial = |i: usize| if views[i].kind == K::Start { 0 } else { 1 };
        let by_flow = |a: &usize, b: &usize| {
            let (a, b) = (*a, *b);
            initial(a)
                .cmp(&initial(b))
                .then(if appear[a] == appear[b] {
                    std::cmp::Ordering::Equal
                } else if appear[a] < appear[b] {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                })
                .then(a.cmp(&b))
        };
        let mut seq = 0.0;
        top.sort_by(by_flow);
        top.reverse();
        let mut pending = top;
        while let Some(i) = pending.pop() {
            cnodes[i].seq = seq;
            seq += 1.0;
            let mut inside = std::mem::take(&mut members[i]);
            inside.sort_by(by_flow);
            for &k in inside.iter().rev() {
                pending.push(k);
            }
        }
    }

    let layout = compound_layout(
        &mut cnodes,
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

    // Route points by rounded y, to find the edges that cross a composite's
    // title bar.
    let mut crossings = Crossings::new();
    for e in &cedges {
        crossings.mark_bends(&e.route);
    }

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); cnodes.len()];
    let mut order: Vec<usize> = Vec::new();
    for (i, c) in cnodes.iter().enumerate() {
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
                let view = &views[i];
                let (Some(node), Some(style)) = (view.node, view.style.as_ref()) else {
                    continue;
                };
                if view.kind >= K::Region {
                    continue;
                }
                let c = &cnodes[i];
                let out = match view.kind {
                    K::Composite => &mut *clusters,
                    K::Note => &mut notes_out,
                    _ => &mut nodes_out,
                };
                let _ = write!(
                    out,
                    "<g class=\"{}{}\" data-id=\"{}\"",
                    classes(view.kind),
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
                let body = node_body(view, c, style, &crossings, pad, small);
                if let Some(link) = db.links.get(&node.id) {
                    // Entity codes are resolved first, so the URL is checked in the
                    // form a browser would follow.
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
            divide(
                &roots,
                pad / 2.0,
                pad / 2.0,
                width - pad / 2.0,
                height - pad / 2.0,
                &cnodes,
                &views,
                pad,
                out,
            );
            for &i in &order {
                let view = &views[i];
                if view.kind != K::Composite {
                    continue;
                }
                let c = &cnodes[i];
                let left = c.center.x + pad - c.size.w / 2.0;
                let top = c.center.y + pad - c.size.h / 2.0;
                divide(
                    &children[i],
                    left,
                    top + view.title_h,
                    left + c.size.w,
                    top + c.size.h,
                    &cnodes,
                    &views,
                    pad,
                    out,
                );
            }
        },
    );

    let mut reach = SideReach::new();
    for (i, ce) in cedges.iter().enumerate() {
        if ce.src == ce.dst || ce.route.len() < 2 || drawn[i].pattern == Some("dashed") {
            continue;
        }
        reach.add(
            ce.dst,
            cnodes[ce.dst].side_of(ce.route[ce.route.len() - 1].at),
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
                let note = edge.pattern == Some("dashed");
                let end_marker = if note { None } else { Some(Marker::Arrow) };
                let path: EdgePath;
                let (src, dst) = (cedges[i].src, cedges[i].dst);
                if src == dst {
                    let a = &cnodes[src];
                    let view = &views[src];
                    let k = f64::from(loop_seen[src]);
                    loop_seen[src] += 1;
                    // The loop hangs off a side the flow does not use: the right, or
                    // below when the flow is horizontal.
                    let side = across(&views, &logical, src);
                    let w = if a.is_group { a.size.w } else { view.w };
                    let h = if a.is_group { a.size.h } else { view.h };
                    let out = (if side { w } else { h }) / 2.0;
                    let spread = ((if side { h } else { w }) / 2.0 - 4.0)
                        .min(8.0 + k * 6.0)
                        .max(2.0);
                    let reach = LOOP + k * 8.0;
                    let trim = Marker::trim_of(end_marker) / js::or_one(js::hypot(reach, spread));
                    // Offsets are given for a loop on the right and turned a quarter
                    // for one below.
                    let (ax, ay) = (a.center.x, a.center.y);
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
                    // Labels stack beyond the outermost loop, in the order of their
                    // transitions.
                    let furthest = LOOP + f64::from(loop_count[src] - 1) * 8.0;
                    let below = loop_text[src] + label.height / 2.0;
                    loop_text[src] += label.height;
                    let ce = &mut cedges[i];
                    ce.label_at = if side {
                        Point::new(
                            ax + out + furthest + 4.0 + ce.label.w / 2.0,
                            ay - loop_height[src] / 2.0 + below,
                        )
                    } else {
                        Point::new(ax, ay + out + furthest * 0.75 + 4.0 + below)
                    };
                } else {
                    let mut route = cedges[i].route.clone();
                    let leaves = reach.of(src, cnodes[src].side_of(route[0].at));
                    let arrives = reach.of(dst, cnodes[dst].side_of(route[route.len() - 1].at));
                    for point in &mut route {
                        point.at += Point::new(pad, pad);
                    }
                    attach(&mut route, 0, src, &cnodes, &views, pad);
                    let last = route.len() - 1;
                    attach(&mut route, last, dst, &cnodes, &views, pad);
                    if let Some(&composite) = anchors.get(&i)
                        && route.len() >= 2
                    {
                        let g = &cnodes[composite];
                        let inward = views[src].kind == K::Hidden;
                        let at = route[if inward { 0 } else { route.len() - 1 }].at;
                        let near =
                            matches!(g.dir, Some(Direction::TopToBottom | Direction::LeftToRight))
                                == inward;
                        let point = if vertical(g.dir) {
                            RoutePoint::new(
                                Point::new(
                                    at.x,
                                    g.center.y
                                        + pad
                                        + if near {
                                            views[composite].title_h - g.size.h / 2.0
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
                            Point::new(ce.label_at.x + pad, ce.label_at.y + pad),
                            "",
                        )
                    );
                }
            }
        },
    );
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}{notes_out}</g>");
    let titled = with_title(
        db.titles.title.as_deref(),
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
        Accessible {
            acc_title: db.titles.acc_title.as_deref(),
            acc_descr: db.titles.acc_descr.as_deref(),
        },
        &titled.content,
    );

    Rendered {
        svg,
        width: titled.width,
        height: titled.height,
        links,
    }
}

/// The drawing of a state or note, centred on its origin, or of a composite
/// in place.
fn node_body<'a>(
    view: &'a View,
    c: &'a CNode,
    style: &'a ResolvedStyle,
    crossings: &'a Crossings,
    pad: f64,
    small: f64,
) -> impl fmt::Display + 'a {
    let p = crate::PREFIX;
    let (w, h) = (view.w, view.h);
    fragment(move |f| match view.kind {
        K::Composite => {
            let left = c.center.x + pad - c.size.w / 2.0;
            let top = c.center.y + pad - c.size.h / 2.0;
            write!(
                f,
                "{}",
                rect(
                    left,
                    top,
                    c.size.w,
                    c.size.h,
                    format_args!(
                        " rx=\"{}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}",
                        num(CORNER),
                        style.shape
                    ),
                )
            )?;
            if view.title_h > 0.0 {
                write!(
                    f,
                    "<path d=\"M{},{}H{}\"{RULE}{}/>{}",
                    num(left),
                    num(top + view.title_h),
                    num(left + c.size.w),
                    style.line,
                    view.label.svg(
                        Point::new(
                            title_x(c, view.label.width, crossings) + pad,
                            top + view.title_h / 2.0
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
                -w / 2.0,
                -h / 2.0,
                w,
                h,
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
                view.label.svg(
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
                    -w / 2.0,
                    -h / 2.0,
                    w,
                    h,
                    format_args!(
                        " rx=\"{}\"{SURFACE}{}",
                        num(CORNER.min(h / 2.0)),
                        style.shape
                    ),
                )
            )?;
            let label_attrs = styled(crate::class!("label"), &style.text);
            if view.kind == K::State {
                return write!(f, "{}", view.label.svg(Point::new(0.0, 0.0), &label_attrs));
            }
            let mut at = PAD_Y - h / 2.0 + view.label.height;
            write!(
                f,
                "<path d=\"M{},{}H{}\"{RULE}{}/>{}",
                num(-w / 2.0),
                num(at + 6.0),
                num(w / 2.0),
                style.line,
                view.label
                    .svg(Point::new(0.0, at - view.label.height / 2.0), &label_attrs)
            )?;
            at += 14.0;
            let line_attrs = format!(
                " class=\"{p}-state-description\" font-size=\"{}\"{}",
                num(small),
                style.text
            );
            for line in &view.lines {
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

fn node_of<'a>(views: &[View<'a>], i: usize) -> &'a StateNode {
    views[i].node.expect("a state's node")
}

/// Picks where a composite's title sits in its title bar so that edges
/// entering from above do not cross it.
fn title_x(c: &CNode, width: f64, crossings: &Crossings) -> f64 {
    let Some(xs) = crossings.at_top(c) else {
        return c.center.x;
    };
    if width == 0.0 {
        return c.center.x;
    }
    let left = c.center.x - c.size.w / 2.0;
    let half = width / 2.0;
    let mut best = c.center.x;
    let mut best_gap = -1.0;
    for x in [
        c.center.x,
        left + GROUP_PAD + half,
        left + c.size.w - GROUP_PAD - half,
    ] {
        let mut gap = f64::INFINITY;
        for &cx in xs {
            if cx > left && cx < left + c.size.w {
                gap = gap.min((cx - x).abs() - half);
            }
        }
        if gap >= 6.0 {
            return x;
        }
        if gap > best_gap + 0.5 {
            best_gap = gap;
            best = x;
        }
    }
    best
}
