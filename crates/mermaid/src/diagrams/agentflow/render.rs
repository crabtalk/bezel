//! Pele's `diagrams/agentflow/render.ts`.

use crate::geom::{Axis, Direction, Point, RoutePoint, Size};
use crate::svg::edges::Curve;
use crate::svg::edges::Marker;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use crate::config::positive;
use crate::diagrams::flowchart::FlowchartConfig;
use crate::svg::builder::rewrite;
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::{
    color_slots::{KIND_COUNT, kind_slot},
    db::AgentflowDb,
    graph::{AgentGraph, GraphEdge, GraphNode, build_agent_graph},
    types::LabelType,
};
use crate::{Ctx, Host};
use crate::{
    Link, Rendered,
    diagrams::{
        common::fit_width::{runs_across, turn_to_fit},
        flowchart::shapes::canonical_shape,
    },
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, compound_layout, shift_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{Esc, EscText, fragment, num},
        cluster::Crossings,
        edges::{EdgePath, edge_label_svg, loop_path, route_path},
        root::{Accessible, svg_document},
        shapes::{PAD_X, PAD_Y, Shape, ShapeView},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style, series_color},
    },
    text::{Label, Style, decode_entities},
    util::url::{NoteLink, linked},
};

/// The number of series tokens, which is the palette Mermaid's slot rules
/// are applied to.
pub const PALETTE: usize = 8;

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 48.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const DOTS: f64 = 14.0;
const COLLAPSED: &str = "collapsed";

static RE_FILL: Lazy = Lazy::new(r#"[";]fill:"#, "");
static RE_DEFAULT: Lazy = Lazy::new(r"^default\s?", "");
static RE_EDGE_CLASSES: Lazy = Lazy::new(
    r"edge-thickness-normal|edge-pattern-solid|flowchart-link",
    "g",
);

/// The side a flow runs toward, with sides numbered clockwise from the top.
fn flow_end(dir: Direction) -> usize {
    match dir {
        Direction::TopToBottom => 2,
        Direction::RightToLeft => 3,
        Direction::BottomToTop => 0,
        Direction::LeftToRight => 1,
    }
}

/// Builds the graph of a model. Building it also records the model's shape
/// and containment diagnostics.
pub fn agent_graph(db: &mut AgentflowDb, flow: &FlowchartConfig) -> AgentGraph {
    build_agent_graph(db, PALETTE, flow.curve.as_deref())
}

/// Mermaid's `agentflow` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentflowConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub wrapping_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub min_node_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub node_spacing: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub rank_spacing: Option<f64>,
}

/// A palette color as an outline with a light wash of the same color inside.
/// A fill the author set replaces the wash and stays opaque.
fn tint<'a>(slot: usize, wash: &'a str, style: &ResolvedStyle) -> impl fmt::Display + 'a {
    let color = series_color(slot as i64);
    let filled = RE_FILL.test(&style.shape);
    fragment(move |f| {
        write!(f, " fill=\"{color}\"")?;
        if !filled {
            write!(f, " fill-opacity=\"{wash}\"")?;
        }
        write!(f, " stroke=\"{color}\"")
    })
}

fn dot(x: f64, y: f64) -> impl fmt::Display {
    fragment(move |f| {
        write!(
            f,
            "M{}a1.75,1.75 0 1 0 3.5,0a1.75,1.75 0 1 0 -3.5,0Z",
            Point::new(x - 1.75, y)
        )
    })
}

struct NodeView<'g> {
    node: &'g GraphNode,
    style: ResolvedStyle,
    label: Label,
    shape: String,
    w: f64,
    h: f64,
    dy: f64,
    loops: usize,
}

impl NodeView<'_> {
    fn shape_view(&self) -> ShapeView {
        ShapeView {
            shape: Shape::named(&self.shape),
            size: Size::new(self.w, self.h),
        }
    }
}

struct Held<'g> {
    edge: &'g GraphEdge,
    label: Label,
    src: usize,
    dst: usize,
}

pub fn render_agentflow(
    db: &AgentflowDb,
    config: &AgentflowConfig,
    ctx: &Ctx<'_, impl Host>,
) -> Rendered {
    if !runs_across(db.store.direction.as_deref()) {
        return draw(db, config, ctx, false);
    }
    turn_to_fit(ctx, |down| draw(db, config, ctx, down))
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(
    db: &AgentflowDb,
    config: &AgentflowConfig,
    ctx: &Ctx<'_, impl Host>,
    turned: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let graph = &db.graph;
    let size = ctx.options.font_size;
    let edge_size = js::round(size * 0.875);
    let measure = ctx.host;
    let wrap_width = positive(config.wrapping_width, 120.0);
    let min_width = positive(config.min_node_width, 120.0);
    let pad = ctx.options.padding.unwrap_or(8.0);

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<NodeView> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        index.insert(&node.id, i);
    }

    for node in &graph.nodes {
        let style = if node.css_styles.is_empty() {
            resolve_style(&node.css_compiled_styles)
        } else {
            let mut all = node.css_compiled_styles.clone();
            all.extend(node.css_styles.iter().cloned());
            resolve_style(&all)
        };
        let text_style = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let markdown = node.label_type == LabelType::Markdown;
        let font_size = style.font_size.unwrap_or(size);
        let view: NodeView;
        let mut c: CNode;
        if node.is_group {
            let label = Label::layout(
                node.label.as_deref(),
                markdown,
                measure,
                font_size,
                4000.0,
                text_style,
            );
            c = CNode::new(Size::default(), None);
            c.is_group = true;
            c.dir = node
                .dir
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(Direction::from_name);
            c.pad_x = GROUP_PAD;
            c.pad_top = if label.height > 0.0 {
                label.height + 16.0
            } else {
                GROUP_PAD
            };
            c.pad_bottom = GROUP_PAD;
            c.min_w = label.width + 2.0 * GROUP_PAD - 12.0;
            view = NodeView {
                node,
                style,
                label,
                shape: "group".into(),
                w: 0.0,
                h: 0.0,
                dy: 0.0,
                loops: 0,
            };
        } else if node.shape == "collapsedGroup" {
            let label = Label::layout(
                node.label.as_deref(),
                markdown,
                measure,
                font_size,
                wrap_width,
                text_style,
            );
            let w = label.width.max(min_width) + 2.0 * PAD_X;
            let h = label.height + 2.0 * PAD_Y + DOTS;
            view = NodeView {
                node,
                style,
                label,
                shape: COLLAPSED.into(),
                w,
                h,
                dy: -DOTS / 2.0,
                loops: 0,
            };
            c = CNode::new(Size::new(w, h), None);
        } else {
            let shape = canonical_shape(Some(&node.shape));
            let label = Label::layout(
                node.label.as_deref(),
                markdown,
                measure,
                font_size,
                wrap_width,
                text_style,
            );
            // A diamond grows twice as fast as its label, so it keeps the
            // label's own width.
            let s = Shape::named(shape).size(Size::new(
                if shape == "diam" {
                    label.width
                } else {
                    label.width.max(min_width)
                },
                label.height,
            ));
            view = NodeView {
                node,
                style,
                label,
                shape: shape.to_string(),
                w: s.size.w,
                h: s.size.h,
                dy: s.dy,
                loops: 0,
            };
            c = CNode::new(s.size, None);
        }
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = node
            .parent_id
            .as_ref()
            .and_then(|parent| index.get(parent.as_str()).copied());
    }

    let root_dir = if turned {
        Direction::TopToBottom
    } else {
        db.store
            .direction
            .as_deref()
            .map(Direction::from_name)
            .unwrap_or_default()
    };
    // The direction of the level a node sits in.
    let flow_of = |cnodes: &[CNode], i: usize| -> Direction {
        let mut dir: Option<Direction> = None;
        let mut p = cnodes[i].parent;
        let mut hops = 0;
        while let Some(at) = p
            && dir.is_none()
            && hops < 10000
        {
            dir = cnodes[at].dir;
            p = cnodes[at].parent;
            hops += 1;
        }
        dir.unwrap_or(root_dir)
    };
    let flows_sideways = |cnodes: &[CNode], i: usize| -> bool {
        let dir = flow_of(cnodes, i);
        !dir.is_vertical()
    };

    let mut depth: Vec<i32> = vec![-1; cnodes.len()];
    for i in 0..cnodes.len() {
        let mut chain: Vec<usize> = Vec::new();
        let mut j = Some(i);
        while let Some(at) = j
            && depth[at] < 0
            && chain.len() <= cnodes.len()
        {
            chain.push(at);
            j = cnodes[at].parent;
        }
        let d = j.map_or(0, |at| depth[at] + 1);
        for (d, &k) in (d..).zip(chain.iter().rev()) {
            depth[k] = d;
        }
    }
    let contains = |cnodes: &[CNode], group: usize, i: usize| -> bool {
        let mut j = Some(i);
        let mut d = depth[i];
        while d > depth[group] {
            let Some(at) = j else {
                return false;
            };
            j = cnodes[at].parent;
            d -= 1;
        }
        j == Some(group) && i != group
    };

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&GraphEdge> = Vec::new();
    // Edges between a node and a container around it. The layout has no
    // place for them, so they are drawn afterwards, between the node and the
    // container's border.
    let mut held: Vec<Held> = Vec::new();
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        let label = Label::layout(
            Some(&edge.label),
            edge.label_type == LabelType::Markdown,
            measure,
            edge_size,
            wrap_width,
            0,
        );
        if contains(&cnodes, src, dst) || contains(&cnodes, dst, src) {
            held.push(Held {
                edge,
                label,
                src,
                dst,
            });
            continue;
        }
        if src == dst {
            // A self-loop sits beside the node, across the flow, and the
            // node's box grows to hold it.
            views[src].loops += 1;
            if flows_sideways(&cnodes, src) {
                cnodes[src].size.h += 2.0
                    * (LOOP
                        + if label.height > 0.0 {
                            label.height + 8.0
                        } else {
                            0.0
                        });
            } else {
                cnodes[src].size.w += 2.0
                    * (LOOP
                        + if label.width > 0.0 {
                            label.width + 12.0
                        } else {
                            0.0
                        });
            }
        }
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: edge.minlen.unwrap_or(1),
            label: CEdge::label_room(&label),
            ..CEdge::default()
        });
        edge_labels.push(label);
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
            ..LayeredOptions::default()
        },
    );

    let title = Label::layout(
        db.store.titles.title.as_deref(),
        false,
        measure,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = layout.w.max(title.width);
    let ox = pad + (inner - layout.w) / 2.0;
    let oy = pad + title_height;
    shift_layout(&mut cnodes, &mut cedges, Point::new(ox, oy));
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.h + title_height + 2.0 * pad).ceil();
    let mut links: Vec<Link> = Vec::new();

    // An edge between a node and a container around it runs straight between
    // the node and the container's border: out along the flow, or in against
    // it. If another node is in the way it takes a side instead, and failing
    // that the far end.
    let mut held_paths: Vec<EdgePath> = Vec::new();
    let mut held_labels: Vec<Point> = Vec::new();
    let mut held_routes: Vec<Vec<RoutePoint>> = Vec::new();
    let careful = ((held.len() * cnodes.len()) as f64) < 2e6;
    for h in &held {
        let out = contains(&cnodes, h.dst, h.src);
        let outer = if out { h.dst } else { h.src };
        let i = if out { h.src } else { h.dst };
        let g = &cnodes[outer];
        let n = &cnodes[i];
        let end = flow_end(g.dir.unwrap_or_else(|| flow_of(&cnodes, outer)));
        let first = if out { end } else { (end + 2) & 3 };
        let toward = |side: usize| -> Vec<RoutePoint> {
            let upright = (side & 1) == 0;
            let sign = if side == 0 || side == 3 { -1.0 } else { 1.0 };
            let nx = if upright {
                n.center.x
            } else {
                n.center.x + (sign * n.size.w) / 2.0
            };
            let ny = if upright {
                n.center.y + (sign * n.size.h) / 2.0
            } else {
                n.center.y
            };
            let bx = if upright {
                n.center.x
            } else {
                g.center.x + (sign * g.size.w) / 2.0
            };
            let by = if upright {
                g.center.y + (sign * g.size.h) / 2.0
            } else {
                n.center.y
            };
            let axis = if upright { Axis::Y } else { Axis::X };
            let (inner, border) = (
                RoutePoint::new(Point::new(nx, ny), axis),
                RoutePoint::new(Point::new(bx, by), axis),
            );
            if out {
                vec![inner, border]
            } else {
                vec![border, inner]
            }
        };
        let blocked = |r: &[RoutePoint]| -> bool {
            let (a, b) = (r[0].at, r[1].at);
            cnodes.iter().enumerate().any(|(k, c)| {
                k != i
                    && !c.is_group
                    && c.center.x + c.size.w / 2.0 > a.x.min(b.x) - 1.0
                    && c.center.x - c.size.w / 2.0 < a.x.max(b.x) + 1.0
                    && c.center.y + c.size.h / 2.0 > a.y.min(b.y) - 1.0
                    && c.center.y - c.size.h / 2.0 < a.y.max(b.y) + 1.0
            })
        };
        let mut route = toward(first);
        if careful && blocked(&route) {
            for side in [(first + 1) & 3, (first + 3) & 3, (first + 2) & 3] {
                let other = toward(side);
                if !blocked(&other) {
                    route = other;
                    break;
                }
            }
        }
        held_labels.push(route[0].at.midpoint(route[1].at));
        let mut drawn_route = route.clone();
        views[i]
            .shape_view()
            .inset_end(n, &mut drawn_route[if out { 0 } else { 1 }].at);
        held_paths.push(route_path(
            &drawn_route,
            Curve::Linear,
            0.0,
            Marker::trim_of(Marker::from_name(&h.edge.arrow_type_end)),
            None,
            None,
        ));
        held_routes.push(route);
    }

    let mut crossings: Crossings = Crossings::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            crossings.mark(&e.route);
        }
        for route in &held_routes {
            crossings.mark(route);
        }
    }

    let mut clusters = String::new();
    let mut nodes_out = String::new();
    let mut inner = String::new();
    let mut label_attrs_buf = String::new();
    let mut line_attrs = String::new();
    let mut shape_attrs = String::new();
    let mut title_attrs = String::new();
    for i in 0..views.len() {
        let view = &views[i];
        let c = &cnodes[i];
        let node = view.node;
        let x = c.center.x;
        let y = c.center.y;
        let classes = class_names(&RE_DEFAULT.replace(&node.css_classes, |_| String::new()));
        let id = EscText(&node.id);
        if c.is_group {
            let title_x = crossings.title_x(c, view.label.width, GROUP_PAD);
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster {p}-flow{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{}{}/>{}</g>",
                num(x - c.size.w / 2.0),
                num(y - c.size.h / 2.0),
                num(c.size.w),
                num(c.size.h),
                tint(node.color_index.unwrap_or(KIND_COUNT), "0.06", &view.style),
                view.style.shape,
                view.label.svg(
                    Point::new(title_x, y - c.size.h / 2.0 + 8.0 + view.label.height / 2.0),
                    rewrite(
                        &mut title_attrs,
                        format_args!(
                            " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
                            view.style.text
                        )
                    )
                ),
            );
            continue;
        }

        let w = view.w;
        let h = view.h;
        let kind = (view.shape != COLLAPSED).then(|| node.kind.unwrap_or("task"));
        let _ = write!(
            nodes_out,
            "<g class=\"{p}-node {p}-shape-{}{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">",
            view.shape,
            fragment(|f| match kind {
                Some(name) => write!(f, " {p}-kind-{name}"),
                None => Ok(()),
            }),
            num(x),
            num(y)
        );
        inner.clear();
        match kind {
            None => {
                let slot = node.color_index.unwrap_or(KIND_COUNT);
                let color = series_color(slot as i64);
                let line = h / 2.0 - DOTS - 2.0;
                let dots = line + (DOTS + 2.0) / 2.0;
                let _ = write!(
                    inner,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{}{}/><path d=\"M{}H{}\" fill=\"none\" stroke=\"{color}\" stroke-dasharray=\"3 3\"{}/><path class=\"{p}-collapsed-mark\" d=\"{}{}{}\" fill=\"{color}\" stroke=\"none\"/>",
                    num(-w / 2.0),
                    num(-h / 2.0),
                    num(w),
                    num(h),
                    tint(slot, "0.06", &view.style),
                    view.style.shape,
                    Point::new(-w / 2.0 + 10.0, line),
                    num(w / 2.0 - 10.0),
                    view.style.line,
                    dot(-9.0, dots),
                    dot(0.0, dots),
                    dot(9.0, dots),
                );
            }
            Some(name) => {
                let slot = kind_slot(name).unwrap_or(1);
                Shape::named(&view.shape).draw(
                    &mut inner,
                    Size::new(w, h),
                    rewrite(
                        &mut shape_attrs,
                        format_args!("{}{}", tint(slot, "0.14", &view.style), view.style.shape),
                    ),
                    rewrite(
                        &mut line_attrs,
                        format_args!(
                            " stroke=\"{}\"{}",
                            series_color(slot as i64),
                            view.style.line
                        ),
                    ),
                );
            }
        }

        let label_attrs = rewrite(
            &mut label_attrs_buf,
            format_args!(" class=\"{p}-label\"{}", view.style.text),
        );
        let text = view.label.svg(Point::new(0.0, view.dy), label_attrs);
        let note = node
            .label
            .as_deref()
            .filter(|label| !label.is_empty() && ctx.options.links && NoteLink::marked(&classes))
            .map(|label| NoteLink::new(decode_entities(label), &node.id, &mut links, ctx));
        let tooltip = node.tooltip.as_deref().filter(|t| !t.is_empty());
        let body = fragment(|f| {
            if let Some(tooltip) = tooltip {
                write!(f, "<title>{}</title>", EscText(tooltip))?;
            }
            f.write_str(&inner)?;
            match &note {
                Some(note) => write!(f, "{}", note.around(text, ctx)),
                None => write!(f, "{text}"),
            }
        });
        let target = node
            .link_target
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(Esc);
        let target = fragment(move |f| match target {
            Some(target) => write!(f, " target=\"{target}\""),
            None => Ok(()),
        });
        let link = node.link.as_deref().filter(|l| !l.is_empty());
        let body = linked(body, link, &node.id, &mut links, ctx, target);
        let _ = write!(nodes_out, "{body}</g>");
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut put = |edge: &GraphEdge, label: &Label, path: &EdgePath, at: Point| {
        let style = if !edge.style.is_empty() || !edge.css_compiled_styles.is_empty() {
            let mut all = edge.css_compiled_styles.clone();
            all.extend(edge.style.iter().cloned());
            Some(resolve_style(&all))
        } else {
            None
        };
        let end_type = edge.arrow_type_end.as_str();
        let color = style
            .as_ref()
            .map_or(Cow::Borrowed("var(--_l)"), ResolvedStyle::line_color);
        let dotted = edge.thickness.as_deref() == Some("dotted");
        let kind = if dotted {
            "reference"
        } else if end_type == "arrow_cross" {
            "failure"
        } else {
            "sequence"
        };
        let classes = class_names(&RE_EDGE_CLASSES.replace(&edge.classes, |_| String::new()));
        let _ = write!(
            edges_out,
            "<g class=\"{p}-edge {p}-edge-{kind}{classes}\" data-id=\"{}\"{}><path d=\"{}\"{}/>{}</g>",
            EscText(&edge.id),
            style.as_ref().map_or("", |s| s.line.as_str()),
            path.d,
            if dotted {
                " stroke-dasharray=\"3 4\""
            } else {
                ""
            },
            Marker::draw_of(Marker::from_name(end_type), path.end, path.end_dir, &color),
        );
        if label.width > 0.0 {
            let _ = write!(
                labels_out,
                "{}",
                edge_label_svg(
                    &edge.id,
                    label,
                    at,
                    style.as_ref().map_or("", |s| s.text.as_str())
                )
            );
        }
    };

    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let end_type = edge.arrow_type_end.as_str();
        let (src, dst) = (cedges[i].src, cedges[i].dst);
        let path = if src == dst {
            let view = &views[src];
            let k = loop_seen.get(&src).copied().unwrap_or(0);
            loop_seen.insert(src, k + 1);
            let sideways = flows_sideways(&cnodes, src);
            loop_path(
                &cnodes[src],
                Size::new(view.w, view.h),
                k as f64,
                sideways,
                LOOP,
                Marker::trim_of(Marker::from_name(end_type)),
                &mut cedges[i],
            )
        } else {
            let mut route = cedges[i].route.clone();
            let last = route.len() - 1;
            views[src]
                .shape_view()
                .inset_end(&cnodes[src], &mut route[0].at);
            views[dst]
                .shape_view()
                .inset_end(&cnodes[dst], &mut route[last].at);
            route_path(
                &route,
                edge.curve
                    .as_deref()
                    .map_or(Curve::Smooth, Curve::from_name),
                0.0,
                Marker::trim_of(Marker::from_name(end_type)),
                None,
                None,
            )
        };
        put(edge, &edge_labels[i], &path, cedges[i].label_at);
    }
    for (i, h) in held.iter().enumerate() {
        put(h.edge, &h.label, &held_paths[i], held_labels[i]);
    }

    let title_attrs = crate::svg::builder::TITLE;
    let mut content = String::with_capacity(
        clusters.len() + edges_out.len() + labels_out.len() + nodes_out.len() + 512,
    );
    let _ = write!(
        content,
        "{}",
        title.svg(
            Point::new(width / 2.0, pad + title.height / 2.0),
            title_attrs
        )
    );
    if !clusters.is_empty() {
        let _ = write!(content, "<g class=\"{p}-clusters\">{clusters}</g>");
    }
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edge-labels\" font-size=\"{}\">{labels_out}</g>",
            js::Number(edge_size)
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");

    let svg = svg_document(
        "agentflow",
        width,
        height,
        size,
        ctx,
        Accessible {
            acc_title: db.store.titles.acc_title.as_deref(),
            acc_descr: db.store.titles.acc_descr.as_deref(),
        },
        &content,
    );
    Rendered {
        svg,
        width,
        height,
        links,
    }
}
