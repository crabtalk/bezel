//! Pele's `diagrams/agentflow/render.ts`.

use crate::svg::theme::node_class_names;
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Write as _},
};

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use super::{
    color_slots::KIND_COUNT,
    db::AgentflowDb,
    graph::{GraphEdge, GraphNode},
    types::{LabelType, Stroke, VertexKind},
};
use crate::{
    Config, Ctx, Error, Host, Link, Rendered,
    config::positive,
    diagrams::{
        Render,
        flowchart::{
            render::{
                EDGE_SEP, GROUP_PAD, LOOP, Layers, NODE_SEP, RANK_SEP, edge_style, frame,
                group_box, hold_loop,
            },
            shapes::NodeShape,
        },
    },
    geom::{Axis, Direction, Point, RoutePoint, Side, Size},
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, compound_layout, level_dir},
        layered::LayeredOptions,
    },
    svg::{
        builder::{Esc, EscText, fragment, num, rewrite},
        cluster::Crossings,
        edges::{Curve, EdgePath, Marker, edge_label_svg, loop_path, route_path},
        root::svg_document,
        shapes::{PAD_X, PAD_Y, Shape, ShapeView},
        theme::{RADIUS, ResolvedStyle, resolve_style_over, series_color},
    },
    text::{Label, Style, decode_entities},
    util::url::{NoteLink, linked},
};

/// The number of series tokens, which is the palette Mermaid's slot rules
/// are applied to.
pub const PALETTE: usize = 8;

const DOTS: f64 = 14.0;

static RE_FILL: Lazy = Lazy::new(r#"[";]fill:"#, "");

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

/// A dot of the collapsed mark, centred on `at`.
fn dot(at: Point) -> impl fmt::Display {
    fragment(move |f| {
        write!(
            f,
            "M{}a1.75,1.75 0 1 0 3.5,0a1.75,1.75 0 1 0 -3.5,0Z",
            Point::new(at.x - 1.75, at.y)
        )
    })
}

/// What a node is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Body {
    Group,
    /// A collapsed flow.
    Collapsed,
    Node(NodeShape),
}

struct NodeView<'g> {
    node: &'g GraphNode,
    style: ResolvedStyle,
    label: Label,
    body: Body,
    /// The drawn box; empty for a group.
    size: Size,
    dy: f64,
}

impl NodeView<'_> {
    fn shape_view(&self) -> ShapeView {
        ShapeView {
            shape: match self.body {
                Body::Node(shape) => shape.shape(),
                _ => Shape::Rect,
            },
            size: self.size,
        }
    }
}

/// An edge between a node and a container around it.
struct Held<'g> {
    edge: &'g GraphEdge,
    label: Label,
    src: usize,
    dst: usize,
}

/// How deep each item sits in the nesting of groups.
struct Nesting {
    depth: Vec<usize>,
}

impl Nesting {
    fn new(cnodes: &[CNode]) -> Self {
        let mut depth: Vec<Option<usize>> = vec![None; cnodes.len()];
        for i in 0..cnodes.len() {
            let mut chain: Vec<usize> = Vec::new();
            let mut j = Some(i);
            while let Some(at) = j
                && depth[at].is_none()
                && chain.len() <= cnodes.len()
            {
                chain.push(at);
                j = cnodes[at].parent;
            }
            let d = j.and_then(|at| depth[at]).map_or(0, |d| d + 1);
            for (d, &k) in (d..).zip(chain.iter().rev()) {
                depth[k] = Some(d);
            }
        }
        Self {
            depth: depth.into_iter().map(Option::unwrap_or_default).collect(),
        }
    }

    /// Whether item `i` is inside group `group`, at any depth.
    fn contains(&self, cnodes: &[CNode], group: usize, i: usize) -> bool {
        let mut j = Some(i);
        let mut d = self.depth[i];
        while d > self.depth[group] {
            let Some(at) = j else {
                return false;
            };
            j = cnodes[at].parent;
            d -= 1;
        }
        j == Some(group) && i != group
    }
}

impl Render for AgentflowDb {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let config = &config.agentflow;
        Ok(
            if self
                .store
                .direction
                .is_some_and(|dir| !dir.direction().is_vertical())
            {
                ctx.turn_to_fit(|down| self.draw(config, ctx, down))
            } else {
                self.draw(config, ctx, false)
            },
        )
    }
}

impl AgentflowDb {
    /// `turned` draws a diagram that runs across as one that runs down.
    fn draw(&self, config: &AgentflowConfig, ctx: &Ctx<'_, impl Host>, turned: bool) -> Rendered {
        let p = crate::PREFIX;
        let graph = &self.graph;
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
            let style = resolve_style_over(&node.css_compiled_styles, &node.css_styles);
            let text_style = style.label_style();
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
                c = group_box(node.dir.map(|dir| dir.direction()), &label, GROUP_PAD);
                view = NodeView {
                    node,
                    style,
                    label,
                    body: Body::Group,
                    size: Size::default(),
                    dy: 0.0,
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
                let size = Size::new(
                    label.width.max(min_width) + 2.0 * PAD_X,
                    label.height + 2.0 * PAD_Y + DOTS,
                );
                view = NodeView {
                    node,
                    style,
                    label,
                    body: Body::Collapsed,
                    size,
                    dy: -DOTS / 2.0,
                };
                c = CNode::new(size, None);
            } else {
                let shape = NodeShape::named(Some(&node.shape));
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
                let s = shape.shape().size(Size::new(
                    if shape == NodeShape::Drawn(Shape::Diamond) {
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
                    body: Body::Node(shape),
                    size: s.size,
                    dy: s.dy,
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
            self.store
                .direction
                .map(|dir| dir.direction())
                .unwrap_or_default()
        };
        let sideways = |cnodes: &[CNode], i: usize| !level_dir(cnodes, i, root_dir).is_vertical();

        let nesting = Nesting::new(&cnodes);
        let mut cedges: Vec<CEdge> = Vec::new();
        let mut edge_labels: Vec<Label> = Vec::new();
        let mut drawn: Vec<&GraphEdge> = Vec::new();
        // Edges between a node and a container around it. The layout has no
        // place for them, so they are drawn afterwards, between the node and
        // the container's border.
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
            if nesting.contains(&cnodes, src, dst) || nesting.contains(&cnodes, dst, src) {
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
                let sideways = sideways(&cnodes, src);
                hold_loop(&mut cnodes[src], &label, sideways);
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
            self.store.titles.title.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let Size {
            w: width,
            h: height,
        } = frame(layout, &title, pad, &mut cnodes, &mut cedges);
        let mut links: Vec<Link> = Vec::new();

        // An edge between a node and a container around it runs straight
        // between the node and the container's border: out along the flow, or
        // in against it. If another node is in the way it takes a side
        // instead, and failing that the far end.
        let mut held_paths: Vec<EdgePath> = Vec::new();
        let mut held_labels: Vec<Point> = Vec::new();
        let mut held_routes: Vec<Vec<RoutePoint>> = Vec::new();
        let careful = ((held.len() * cnodes.len()) as f64) < 2e6;
        for h in &held {
            let out = nesting.contains(&cnodes, h.dst, h.src);
            let outer = if out { h.dst } else { h.src };
            let i = if out { h.src } else { h.dst };
            let g = &cnodes[outer];
            let n = &cnodes[i];
            let end = g
                .dir
                .unwrap_or_else(|| level_dir(&cnodes, outer, root_dir))
                .start_side()
                .opposite();
            let first = if out { end } else { end.opposite() };
            let toward = |side: Side| -> Vec<RoutePoint> {
                let upright = side.is_horizontal();
                let sign = if side == Side::Top || side == Side::Left {
                    -1.0
                } else {
                    1.0
                };
                let (inner, border) = if upright {
                    (
                        Point::new(n.center.x, n.center.y + (sign * n.size.h) / 2.0),
                        Point::new(n.center.x, g.center.y + (sign * g.size.h) / 2.0),
                    )
                } else {
                    (
                        Point::new(n.center.x + (sign * n.size.w) / 2.0, n.center.y),
                        Point::new(g.center.x + (sign * g.size.w) / 2.0, n.center.y),
                    )
                };
                let axis = if upright { Axis::Y } else { Axis::X };
                let (inner, border) = (RoutePoint::new(inner, axis), RoutePoint::new(border, axis));
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
                for side in [
                    first.clockwise(),
                    first.counterclockwise(),
                    first.opposite(),
                ] {
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
                Marker::trim_of(h.edge.arrow_type_end),
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

        let mut layers = Layers::default();
        let mut inner = String::new();
        let mut label_attrs_buf = String::new();
        let mut line_attrs = String::new();
        let mut shape_attrs = String::new();
        let mut title_attrs = String::new();
        for (i, view) in views.iter().enumerate() {
            let c = &cnodes[i];
            let node = view.node;
            let Point { x, y } = c.center;
            let classes = node_class_names(&node.css_classes);
            let id = EscText(&node.id);
            let kind = match view.body {
                Body::Group => {
                    let title_x = crossings.title_x(c, view.label.width, GROUP_PAD);
                    let _ = write!(
                        layers.clusters,
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
                Body::Collapsed => None,
                Body::Node(_) => Some(node.kind.unwrap_or(VertexKind::Task)),
            };

            let Size { w, h } = view.size;
            let _ = write!(
                layers.nodes,
                "<g class=\"{p}-node {p}-shape-{}{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">",
                match view.body {
                    Body::Node(shape) => shape.name(),
                    _ => "collapsed",
                },
                fragment(|f| match kind {
                    Some(kind) => write!(f, " {p}-kind-{}", kind.name()),
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
                        dot(Point::new(-9.0, dots)),
                        dot(Point::new(0.0, dots)),
                        dot(Point::new(9.0, dots)),
                    );
                }
                Some(kind) => {
                    let slot = kind.slot();
                    view.shape_view().shape.draw(
                        &mut inner,
                        view.size,
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
                .filter(|label| {
                    !label.is_empty() && ctx.options.links && NoteLink::marked(&classes)
                })
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
            let _ = write!(layers.nodes, "{body}</g>");
        }

        let mut put = |edge: &GraphEdge, label: &Label, path: &EdgePath, at: Point| {
            let style = edge_style(&edge.css_compiled_styles, &edge.style);
            let color = style
                .as_ref()
                .map_or(Cow::Borrowed("var(--_l)"), ResolvedStyle::line_color);
            let dotted = edge.thickness == Some(Stroke::Dotted);
            let kind = if dotted {
                "reference"
            } else if edge.arrow_type_end == Some(Marker::Cross) {
                "failure"
            } else {
                "sequence"
            };
            let _ = write!(
                layers.edges,
                "<g class=\"{p}-edge {p}-edge-{kind}\" data-id=\"{}\"{}><path d=\"{}\"{}/>{}</g>",
                EscText(&edge.id),
                style.as_ref().map_or("", |s| s.line.as_str()),
                path.d,
                if dotted {
                    " stroke-dasharray=\"3 4\""
                } else {
                    ""
                },
                Marker::draw_of(edge.arrow_type_end, path.end, path.end_dir, &color),
            );
            if label.width > 0.0 {
                let _ = write!(
                    layers.labels,
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
            let (src, dst) = (cedges[i].src, cedges[i].dst);
            let path = if src == dst {
                let k = loop_seen.get(&src).copied().unwrap_or(0);
                loop_seen.insert(src, k + 1);
                let sideways = sideways(&cnodes, src);
                loop_path(
                    &cnodes[src],
                    views[src].size,
                    k as f64,
                    sideways,
                    LOOP,
                    Marker::trim_of(edge.arrow_type_end),
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
                    Marker::trim_of(edge.arrow_type_end),
                    None,
                    None,
                )
            };
            put(edge, &edge_labels[i], &path, cedges[i].label_at);
        }
        for (i, h) in held.iter().enumerate() {
            put(h.edge, &h.label, &held_paths[i], held_labels[i]);
        }

        let content = layers.content(&title, width, pad, edge_size);
        let svg = svg_document(
            "agentflow",
            width,
            height,
            size,
            ctx,
            self.store.titles.accessible(),
            &content,
        );
        Rendered {
            svg,
            width,
            height,
            links,
        }
    }
}
