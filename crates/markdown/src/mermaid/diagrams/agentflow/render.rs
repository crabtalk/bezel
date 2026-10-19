//! Pele's `diagrams/agentflow/render.ts`.

use std::{collections::HashMap, fmt::Write as _};

use super::{
    color_slots::{KIND_COUNT, kind_slot},
    db::AgentflowDb,
    graph::{AgentGraph, GraphEdge, GraphNode, build_agent_graph},
    types::LabelType,
};
use crate::mermaid::{
    Config, Link, Options, Rendered,
    config::Value,
    diagrams::{
        common::fit_width::{runs_across, turn_to_fit},
        flowchart::shapes::canonical_shape,
    },
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, Dir, cnode, compound_layout, direction, shift_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{esc, esc_text, label_svg, num},
        cluster::{Crossings, cluster_title_x, mark_crossings},
        edges::{EdgePath, edge_label_svg, loop_path, marker, marker_trim, route_path},
        root::{Accessible, svg_document},
        shapes::{PAD_X, PAD_Y, ShapeView, draw_shape, inset_route, shape_size},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style, series_color},
    },
    text::{Label, Style, decode_entities, layout_label},
    util::url::{link_url, linked, rel_attr, sanitize_url},
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
static RE_INTERNAL: Lazy = Lazy::new(r" internal-link(?: |$)", "");
static RE_EDGE_CLASSES: Lazy = Lazy::new(
    r"edge-thickness-normal|edge-pattern-solid|flowchart-link",
    "g",
);

/// The side a flow runs toward, with sides numbered clockwise from the top.
fn flow_end(dir: Dir) -> usize {
    match dir {
        Dir::TB => 2,
        Dir::RL => 3,
        Dir::BT => 0,
        Dir::LR => 1,
    }
}

/// Builds the graph of a model. Building it also records the model's shape
/// and containment diagnostics.
pub fn agent_graph(db: &mut AgentflowDb, config: &Config) -> AgentGraph {
    let curve = config
        .section("flowchart")
        .and_then(|flow| flow.get("curve"))
        .and_then(Value::as_str);
    build_agent_graph(db, PALETTE, curve)
}

fn number_option(config: &Config, key: &str, fallback: f64) -> f64 {
    config
        .section("agentflow")
        .map_or(fallback, |section| section.number_option(key, fallback))
}

/// A palette color as an outline with a light wash of the same color inside.
/// A fill the author set replaces the wash and stays opaque.
fn tint(slot: usize, wash: &str, style: &ResolvedStyle) -> String {
    let color = series_color(slot as i64);
    let opacity = if RE_FILL.test(&style.shape) {
        String::new()
    } else {
        format!(" fill-opacity=\"{wash}\"")
    };
    format!(" fill=\"{color}\"{opacity} stroke=\"{color}\"")
}

fn dot(x: f64, y: f64) -> String {
    format!(
        "M{},{}a1.75,1.75 0 1 0 3.5,0a1.75,1.75 0 1 0 -3.5,0Z",
        num(x - 1.75),
        num(y)
    )
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
    fn shape_view(&self) -> ShapeView<'_> {
        ShapeView {
            shape: &self.shape,
            w: self.w,
            h: self.h,
        }
    }
}

struct Held<'g> {
    edge: &'g GraphEdge,
    label: Label,
    src: usize,
    dst: usize,
}

pub fn render_agentflow(db: &AgentflowDb, config: &Config, options: &Options) -> Rendered {
    if !runs_across(db.direction.as_deref()) {
        return draw(db, config, options, false);
    }
    turn_to_fit(options, |down| draw(db, config, options, down))
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(db: &AgentflowDb, config: &Config, options: &Options, turned: bool) -> Rendered {
    let p = crate::mermaid::PREFIX;
    let graph = &db.graph;
    let size = options.font_size.unwrap_or(16.0);
    let edge_size = js::round(size * 0.875);
    let measure = options.measurer();
    let wrap_width = number_option(config, "wrappingWidth", 120.0);
    let min_width = number_option(config, "minNodeWidth", 120.0);
    let pad = options.padding.unwrap_or(8.0);

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
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                &*measure,
                font_size,
                4000.0,
                text_style,
            );
            c = cnode(0.0, 0.0, -1);
            c.is_group = true;
            c.dir = node
                .dir
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(|d| direction(Some(d)));
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
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                &*measure,
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
            c = cnode(w, h, -1);
        } else {
            let shape = canonical_shape(Some(&node.shape));
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                &*measure,
                font_size,
                wrap_width,
                text_style,
            );
            // A diamond grows twice as fast as its label, so it keeps the
            // label's own width.
            let s = shape_size(
                shape,
                if shape == "diam" {
                    label.width
                } else {
                    label.width.max(min_width)
                },
                label.height,
            );
            view = NodeView {
                node,
                style,
                label,
                shape: shape.to_string(),
                w: s.w,
                h: s.h,
                dy: s.dy,
                loops: 0,
            };
            c = cnode(s.w, s.h, -1);
        }
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = match &node.parent_id {
            Some(parent) => index.get(parent.as_str()).map_or(-1, |&at| at as i32),
            None => -1,
        };
    }

    let root_dir = if turned {
        Dir::TB
    } else {
        direction(db.direction.as_deref())
    };
    // The direction of the level a node sits in.
    let flow_of = |cnodes: &[CNode], i: usize| -> Dir {
        let mut dir: Option<Dir> = None;
        let mut p = cnodes[i].parent;
        let mut hops = 0;
        while p >= 0 && dir.is_none() && hops < 10000 {
            dir = cnodes[p as usize].dir;
            p = cnodes[p as usize].parent;
            hops += 1;
        }
        dir.unwrap_or(root_dir)
    };
    let flows_sideways = |cnodes: &[CNode], i: usize| -> bool {
        let dir = flow_of(cnodes, i);
        dir == Dir::LR || dir == Dir::RL
    };

    let mut depth: Vec<i32> = vec![-1; cnodes.len()];
    for i in 0..cnodes.len() {
        let mut chain: Vec<usize> = Vec::new();
        let mut j = i as i32;
        while j >= 0 && depth[j as usize] < 0 && chain.len() <= cnodes.len() {
            chain.push(j as usize);
            j = cnodes[j as usize].parent;
        }
        let d = if j >= 0 { depth[j as usize] + 1 } else { 0 };
        for (d, &k) in (d..).zip(chain.iter().rev()) {
            depth[k] = d;
        }
    }
    let contains = |cnodes: &[CNode], group: usize, i: usize| -> bool {
        let mut j = i as i32;
        let mut d = depth[i];
        while d > depth[group] {
            if j < 0 {
                return false;
            }
            j = cnodes[j as usize].parent;
            d -= 1;
        }
        j == group as i32 && i != group
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
        let label = layout_label(
            Some(&edge.label),
            edge.label_type == LabelType::Markdown,
            &*measure,
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
                cnodes[src].h += 2.0
                    * (LOOP
                        + if label.height > 0.0 {
                            label.height + 8.0
                        } else {
                            0.0
                        });
            } else {
                cnodes[src].w += 2.0
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
            label_w: if label.width > 0.0 {
                label.width + 12.0
            } else {
                0.0
            },
            label_h: if label.height > 0.0 {
                label.height + 4.0
            } else {
                0.0
            },
            route: Vec::new(),
            label_x: 0.0,
            label_y: 0.0,
        });
        edge_labels.push(label);
    }

    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: number_option(config, "nodeSpacing", NODE_SEP),
            edge_sep: EDGE_SEP,
            rank_sep: number_option(config, "rankSpacing", RANK_SEP),
            port_sep: 20.0,
            ..LayeredOptions::default()
        },
    );

    let title = layout_label(
        db.title.as_deref(),
        false,
        &*measure,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let inner = layout.width.max(title.width);
    let ox = pad + (inner - layout.width) / 2.0;
    let oy = pad + title_height;
    shift_layout(&mut cnodes, &mut cedges, ox, oy);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();
    let mut links: Vec<Link> = Vec::new();

    // An edge between a node and a container around it runs straight between
    // the node and the container's border: out along the flow, or in against
    // it. If another node is in the way it takes a side instead, and failing
    // that the far end.
    let mut held_paths: Vec<EdgePath> = Vec::new();
    let mut held_labels: Vec<f64> = Vec::new();
    let mut held_routes: Vec<Vec<f64>> = Vec::new();
    let careful = ((held.len() * cnodes.len()) as f64) < 2e6;
    for h in &held {
        let out = contains(&cnodes, h.dst, h.src);
        let outer = if out { h.dst } else { h.src };
        let i = if out { h.src } else { h.dst };
        let g = &cnodes[outer];
        let n = &cnodes[i];
        let end = flow_end(g.dir.unwrap_or_else(|| flow_of(&cnodes, outer)));
        let first = if out { end } else { (end + 2) & 3 };
        let toward = |side: usize| -> Vec<f64> {
            let upright = (side & 1) == 0;
            let sign = if side == 0 || side == 3 { -1.0 } else { 1.0 };
            let nx = if upright {
                n.x
            } else {
                n.x + (sign * n.w) / 2.0
            };
            let ny = if upright {
                n.y + (sign * n.h) / 2.0
            } else {
                n.y
            };
            let bx = if upright {
                n.x
            } else {
                g.x + (sign * g.w) / 2.0
            };
            let by = if upright {
                g.y + (sign * g.h) / 2.0
            } else {
                n.y
            };
            let axis = if upright { 0.0 } else { 1.0 };
            if out {
                vec![nx, ny, axis, bx, by, axis]
            } else {
                vec![bx, by, axis, nx, ny, axis]
            }
        };
        let blocked = |r: &[f64]| -> bool {
            cnodes.iter().enumerate().any(|(k, c)| {
                k != i
                    && !c.is_group
                    && c.x + c.w / 2.0 > r[0].min(r[3]) - 1.0
                    && c.x - c.w / 2.0 < r[0].max(r[3]) + 1.0
                    && c.y + c.h / 2.0 > r[1].min(r[4]) - 1.0
                    && c.y - c.h / 2.0 < r[1].max(r[4]) + 1.0
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
        held_labels.push((route[0] + route[3]) / 2.0);
        held_labels.push((route[1] + route[4]) / 2.0);
        let mut drawn_route = route.clone();
        inset_route(
            &mut drawn_route,
            if out { 0 } else { 3 },
            &views[i].shape_view(),
            n,
        );
        held_paths.push(route_path(
            &drawn_route,
            Some("linear"),
            0.0,
            marker_trim(&h.edge.arrow_type_end),
            None,
            None,
        ));
        held_routes.push(route);
    }

    let mut crossings: Crossings = HashMap::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
        for route in &held_routes {
            mark_crossings(&mut crossings, route);
        }
    }

    let mut clusters = String::new();
    let mut nodes_out = String::new();
    for i in 0..views.len() {
        let view = &views[i];
        let c = &cnodes[i];
        let node = view.node;
        let x = c.x;
        let y = c.y;
        let classes = class_names(&RE_DEFAULT.replace(&node.css_classes, |_| String::new()));
        let id = esc_text(&node.id);
        if c.is_group {
            let title_x = cluster_title_x(c, view.label.width, &crossings, GROUP_PAD);
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster {p}-flow{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{}{}/>{}</g>",
                num(x - c.w / 2.0),
                num(y - c.h / 2.0),
                num(c.w),
                num(c.h),
                tint(node.color_index.unwrap_or(KIND_COUNT), "0.06", &view.style),
                view.style.shape,
                label_svg(
                    &view.label,
                    title_x,
                    y - c.h / 2.0 + 8.0 + view.label.height / 2.0,
                    &format!(
                        " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
                        view.style.text
                    ),
                ),
            );
            continue;
        }

        let w = view.w;
        let h = view.h;
        let mut kind = String::new();
        let shape: String;
        if view.shape == COLLAPSED {
            let slot = node.color_index.unwrap_or(KIND_COUNT);
            let color = series_color(slot as i64);
            let line = h / 2.0 - DOTS - 2.0;
            let dots = line + (DOTS + 2.0) / 2.0;
            shape = format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{}{}/><path d=\"M{},{}H{}\" fill=\"none\" stroke=\"{color}\" stroke-dasharray=\"3 3\"{}/><path class=\"{p}-collapsed-mark\" d=\"{}{}{}\" fill=\"{color}\" stroke=\"none\"/>",
                num(-w / 2.0),
                num(-h / 2.0),
                num(w),
                num(h),
                tint(slot, "0.06", &view.style),
                view.style.shape,
                num(-w / 2.0 + 10.0),
                num(line),
                num(w / 2.0 - 10.0),
                view.style.line,
                dot(-9.0, dots),
                dot(0.0, dots),
                dot(9.0, dots),
            );
        } else {
            let name = node.kind.unwrap_or("task");
            let slot = kind_slot(name).unwrap_or(1);
            kind = format!(" {p}-kind-{name}");
            shape = draw_shape(
                &view.shape,
                w,
                h,
                &(tint(slot, "0.14", &view.style) + &view.style.shape),
                &format!(
                    " stroke=\"{}\"{}",
                    series_color(slot as i64),
                    view.style.line
                ),
            );
        }

        let mut text = label_svg(
            &view.label,
            0.0,
            view.dy,
            &format!(" class=\"{p}-label\"{}", view.style.text),
        );
        let internal = RE_INTERNAL.test(&format!("{classes} "));
        if internal
            && let Some(label) = node.label.as_deref().filter(|label| !label.is_empty())
            && options.links != Some(false)
        {
            // The label is a note name, but it still ends up in an href, so
            // it gets the same check as a URL.
            let name = decode_entities(label);
            let safe = link_url(&sanitize_url(&name), options).is_some();
            let target = esc(&name);
            text = format!(
                "<a class=\"internal-link\"{} data-href=\"{target}\">{text}</a>",
                if safe {
                    format!(" href=\"{target}\"{}", rel_attr(options))
                } else {
                    String::new()
                }
            );
            links.push(Link {
                id: node.id.clone(),
                href: name,
                internal: true,
            });
        }
        let mut body = shape + &text;
        if let Some(tooltip) = node.tooltip.as_deref().filter(|t| !t.is_empty()) {
            body = format!("<title>{}</title>{body}", esc_text(tooltip));
        }
        if let Some(link) = node.link.as_deref().filter(|l| !l.is_empty()) {
            let target = match node.link_target.as_deref().filter(|t| !t.is_empty()) {
                Some(target) => format!(" target=\"{}\"", esc(target)),
                None => String::new(),
            };
            body = linked(&body, link, &node.id, &mut links, options, &target);
        }
        let _ = write!(
            nodes_out,
            "<g class=\"{p}-node {p}-shape-{}{kind}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">{body}</g>",
            view.shape,
            num(x),
            num(y)
        );
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut put = |edge: &GraphEdge, label: &Label, path: &EdgePath, x: f64, y: f64| {
        let style = if !edge.style.is_empty() || !edge.css_compiled_styles.is_empty() {
            let mut all = edge.css_compiled_styles.clone();
            all.extend(edge.style.iter().cloned());
            Some(resolve_style(&all))
        } else {
            None
        };
        let end_type = edge.arrow_type_end.as_str();
        let color = match style.as_ref().and_then(|s| s.stroke.as_deref()) {
            Some(stroke) if !stroke.is_empty() => esc(stroke),
            _ => "var(--_l)".to_string(),
        };
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
            esc_text(&edge.id),
            style.as_ref().map_or("", |s| s.line.as_str()),
            path.d,
            if dotted {
                " stroke-dasharray=\"3 4\""
            } else {
                ""
            },
            marker(end_type, path.ex, path.ey, path.edx, path.edy, &color),
        );
        if label.width > 0.0 {
            labels_out.push_str(&edge_label_svg(
                &edge.id,
                label,
                x,
                y,
                style.as_ref().map_or("", |s| s.text.as_str()),
            ));
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
                view.w,
                view.h,
                k as f64,
                sideways,
                LOOP,
                marker_trim(end_type),
                &mut cedges[i],
            )
        } else {
            let mut route = cedges[i].route.clone();
            let at_end = route.len() - 3;
            inset_route(&mut route, 0, &views[src].shape_view(), &cnodes[src]);
            inset_route(&mut route, at_end, &views[dst].shape_view(), &cnodes[dst]);
            route_path(
                &route,
                edge.curve.as_deref(),
                0.0,
                marker_trim(end_type),
                None,
                None,
            )
        };
        put(
            edge,
            &edge_labels[i],
            &path,
            cedges[i].label_x,
            cedges[i].label_y,
        );
    }
    for (i, h) in held.iter().enumerate() {
        put(
            h.edge,
            &h.label,
            &held_paths[i],
            held_labels[2 * i],
            held_labels[2 * i + 1],
        );
    }

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
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
            js::num_str(edge_size)
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");

    let svg = svg_document(
        "agentflow",
        width,
        height,
        size,
        options,
        Accessible {
            acc_title: db.acc_title.as_deref(),
            acc_descr: db.acc_descr.as_deref(),
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
