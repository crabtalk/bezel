//! Pele's `diagrams/flowchart/render.ts`.

use super::FlowchartConfig;
use crate::config::positive;
use crate::geom::{Direction, Point, Size};
use crate::svg::builder::rewrite;
use crate::svg::builder::{Esc, EscText, fragment};
use crate::svg::edges::Curve;
use crate::svg::edges::Marker;
use std::{borrow::Cow, collections::HashMap, fmt::Write as _};

use super::{
    db::FlowDb,
    graph::{FlowGraph, GraphEdge, GraphNode, build_flow_graph},
    shapes::canonical_shape,
    types::LabelType,
};
use crate::diagrams::common::icon::icon_svg;
use crate::{Ctx, Host};
use crate::{
    Link, Rendered,
    diagrams::common::fit_width::{runs_across, tighten, title_room, turn_to_fit},
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, compound_layout, shift_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::num,
        cluster::{Crossings, struck_title},
        edges::{EdgePath, SideReach, edge_label_svg, loop_path, route_path},
        root::{Accessible, svg_document},
        shapes::{Shape, ShapeView},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
    },
    text::{Label, Style, decode_entities},
    util::url::{NoteLink, image_url, linked, safe_url},
};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 48.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE_ATTRS: &str = " stroke=\"var(--_b)\"";

static RE_DEFAULT: Lazy = Lazy::new(r"^default\s?", "");
static RE_EDGE_CLASSES: Lazy = Lazy::new(
    r"edge-thickness-normal|edge-pattern-solid|flowchart-link",
    "g",
);

/// A diagram type that is a flowchart with a different layout, which may
/// draw its top-level groups itself.
pub trait FlowVariant {
    fn kind(&self) -> &'static str;
    fn prepare(&self, graph: &mut FlowGraph);
    fn layout(
        &self,
        nodes: &mut [CNode],
        edges: &mut [CEdge],
        dir: Direction,
        opt: &LayeredOptions,
        tight: bool,
    ) -> Size;
    /// Whether [`FlowVariant::draw_group`] draws the top-level groups.
    fn draws_groups(&self) -> bool {
        false
    }
    /// Appends the group to `out`. `id` is the node's id, unescaped.
    #[allow(clippy::too_many_arguments)]
    fn draw_group(
        &self,
        _out: &mut String,
        _c: &CNode,
        _label: &Label,
        _style: &ResolvedStyle,
        _classes: &str,
        _id: &str,
        _dir: Direction,
    ) {
    }
}

fn asset_size(value: Option<f64>, fallback: f64) -> f64 {
    match value {
        Some(value) if value > 0.0 && value <= 4000.0 => value,
        _ => fallback,
    }
}

fn truthy(text: &Option<String>) -> bool {
    text.as_deref().is_some_and(|text| !text.is_empty())
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

pub fn render_flowchart(
    db: &FlowDb,
    config: &FlowchartConfig,
    ctx: &Ctx<'_, impl Host>,
    variant: Option<&dyn FlowVariant>,
) -> Rendered {
    let fitted = |down: bool| tighten(ctx, |tight| draw(db, config, ctx, variant, down, tight));
    if runs_across(db.direction.as_deref()) {
        turn_to_fit(ctx, fitted)
    } else {
        fitted(false)
    }
}

fn flows_sideways(cnodes: &[CNode], i: usize, root_dir: Direction) -> bool {
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
    let dir = dir.unwrap_or(root_dir);
    !dir.is_vertical()
}

/// `turned` draws a flowchart that runs across as one that runs down.
/// `tight` sets nodes closer together.
fn draw(
    db: &FlowDb,
    flow: &FlowchartConfig,
    ctx: &Ctx<'_, impl Host>,
    variant: Option<&dyn FlowVariant>,
    turned: bool,
    tight: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let group_pad = if tight { 12.0 } else { GROUP_PAD };
    let curve_option = flow.curve.as_deref();
    let mut graph = build_flow_graph(db, curve_option);
    if let Some(variant) = variant {
        variant.prepare(&mut graph);
    }
    let size = ctx.options.font_size;
    let edge_size = js::round(size * 0.875);
    let measure = ctx.host;
    let wrap_width = positive(flow.wrapping_width, 200.0).min(if variant.is_some() && tight {
        100.0
    } else {
        f64::INFINITY
    });
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
                if variant.is_some() && tight {
                    80f64.max(title_room(ctx) / 2.0)
                } else {
                    4000.0
                },
                text_style,
            );
            c = CNode::new(Size::default(), None);
            c.is_group = true;
            c.dir = node
                .dir
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(Direction::from_name);
            c.pad_x = group_pad;
            c.pad_top = if label.height > 0.0 {
                label.height + 16.0
            } else {
                GROUP_PAD
            };
            c.pad_bottom = group_pad;
            c.min_w = label.width + 2.0 * group_pad - 12.0;
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
        } else {
            let shape = canonical_shape(Some(&node.shape));
            let text = if truthy(&node.img) || truthy(&node.icon) || Shape::named(shape).has_label()
            {
                node.label.as_deref()
            } else {
                None
            };
            let label = Label::layout(text, markdown, measure, font_size, wrap_width, text_style);
            let w: f64;
            let h: f64;
            let mut dy: f64;
            if truthy(&node.img) || truthy(&node.icon) {
                let bx = if truthy(&node.img) { 80.0 } else { 48.0 };
                let aw = asset_size(node.asset_width, bx);
                let ah = asset_size(node.asset_height, bx);
                w = aw.max(label.width + 8.0);
                h = ah
                    + if label.height > 0.0 {
                        label.height + 6.0
                    } else {
                        0.0
                    };
                dy = if node.pos.as_deref() == Some("t") {
                    -ah / 2.0
                } else {
                    ah / 2.0
                };
                if label.height == 0.0 {
                    dy = 0.0;
                }
            } else {
                let s = Shape::named(shape).size(Size::new(label.width, label.height));
                w = s.size.w;
                h = s.size.h;
                dy = s.dy;
            }
            view = NodeView {
                node,
                style,
                label,
                shape: shape.to_string(),
                w,
                h,
                dy,
                loops: 0,
            };
            c = CNode::new(Size::new(w, h), None);
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
        db.direction
            .as_deref()
            .map(Direction::from_name)
            .unwrap_or_default()
    };
    // The edges on one side of a node spread along it; a picture or an icon
    // keeps them at its middle.
    for i in 0..views.len() {
        let view = &views[i];
        if view.node.is_group {
            cnodes[i].span = f64::INFINITY;
        } else if !truthy(&view.node.img) && !truthy(&view.node.icon) {
            let sideways = flows_sideways(&cnodes, i, root_dir);
            cnodes[i].span = Shape::from_name(&view.shape)
                .map_or(0.0, |shape| shape.span(Size::new(view.w, view.h), sideways));
        }
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&GraphEdge> = Vec::new();
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
        if src == dst {
            // A self-loop sits beside the node, across the flow, and the
            // node's box grows to hold it.
            views[src].loops += 1;
            if flows_sideways(&cnodes, src, root_dir) {
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

    let layout_options = LayeredOptions {
        node_sep: positive(flow.node_spacing, NODE_SEP) * if tight { 0.5 } else { 1.0 },
        edge_sep: EDGE_SEP,
        rank_sep: positive(flow.rank_spacing, RANK_SEP) * if tight { 0.75 } else { 1.0 },
        port_sep: 20.0,
        tracks: curve_option.is_some_and(|curve| curve.starts_with("step")),
        straight: curve_option == Some("linear"),
        apart: false,
        head_room: Some(Marker::Arrow.trim()),
    };
    let layout = match variant {
        Some(variant) => variant.layout(&mut cnodes, &mut cedges, root_dir, &layout_options, tight),
        None => compound_layout(&mut cnodes, &mut cedges, root_dir, &layout_options),
    };

    let title = Label::layout(
        db.titles.title.as_deref(),
        false,
        measure,
        size,
        if tight { title_room(ctx) } else { 4000.0 },
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

    let mut crossings: Crossings = Crossings::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            crossings.mark(&e.route);
        }
    }

    let mut clusters = String::new();
    let mut struck_titles = String::new();
    let mut nodes_out = String::new();
    let mut inner = String::new();
    let mut title_attrs_buf = String::new();
    let mut label_attrs_buf = String::new();
    let mut line_attrs = String::new();
    let mut shape_attrs = String::new();
    for i in 0..views.len() {
        let view = &views[i];
        let c = &cnodes[i];
        let node = view.node;
        let x = c.center.x;
        let y = c.center.y;
        let classes = class_names(&RE_DEFAULT.replace(&node.css_classes, |_| String::new()));
        let id = EscText(&node.id);
        if c.is_group
            && let Some(variant) = variant.filter(|v| v.draws_groups())
            && c.parent.is_none()
        {
            variant.draw_group(
                &mut clusters,
                c,
                &view.label,
                &view.style,
                &classes,
                &node.id,
                root_dir,
            );
            continue;
        }
        if c.is_group {
            let title_x = crossings.title_x(c, view.label.width, group_pad);
            let title_y = y - c.size.h / 2.0 + 8.0 + view.label.height / 2.0;
            let title_attrs = rewrite(
                &mut title_attrs_buf,
                format_args!(
                    " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
                    view.style.text
                ),
            );
            let struck = crossings.crosses_title(c, view.label.width, title_x);
            if struck {
                let _ = write!(
                    struck_titles,
                    "{}",
                    struck_title(
                        &view.label,
                        Point::new(title_x, title_y),
                        title_attrs,
                        &id,
                        &cnodes,
                        i
                    )
                );
            }
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}/>{}</g>",
                num(x - c.size.w / 2.0),
                num(y - c.size.h / 2.0),
                num(c.size.w),
                num(c.size.h),
                view.style.shape,
                fragment(|f| if struck {
                    Ok(())
                } else {
                    write!(
                        f,
                        "{}",
                        view.label.svg(Point::new(title_x, title_y), title_attrs)
                    )
                })
            );
            continue;
        }

        let w = view.w;
        let h = view.h;
        inner.clear();
        if let Some(img) = node.img.as_deref().filter(|img| !img.is_empty()) {
            let aw = asset_size(node.asset_width, 80.0);
            let ah = asset_size(node.asset_height, 80.0);
            let top = if view.label.height > 0.0 {
                if node.pos.as_deref() == Some("t") {
                    h / 2.0 - ah
                } else {
                    -h / 2.0
                }
            } else {
                -ah / 2.0
            };
            if let Some(src) = image_url(&safe_url(img), ctx) {
                let _ = write!(
                    inner,
                    "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{}\"/>",
                    Esc(&src),
                    num(-aw / 2.0),
                    num(top),
                    num(aw),
                    num(ah),
                    if node.constraint.as_deref() == Some("on") {
                        "xMidYMid meet"
                    } else {
                        "none"
                    }
                );
            }
        } else if let Some(icon) = node.icon.as_deref().filter(|icon| !icon.is_empty()) {
            let bx = asset_size(node.asset_height, 48.0);
            let top = if view.label.height > 0.0 {
                if node.pos.as_deref() == Some("t") {
                    h / 2.0 - bx
                } else {
                    -h / 2.0
                }
            } else {
                -bx / 2.0
            };
            if node.shape == "iconCircle" {
                let _ = write!(
                    inner,
                    "<circle cy=\"{}\" r=\"{}\"{SHAPE_ATTRS}{}/>",
                    num(top + bx / 2.0),
                    num(bx / 2.0),
                    view.style.shape
                );
            } else if node.shape != "icon" {
                let _ = write!(
                    inner,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{SHAPE_ATTRS}{}/>",
                    num(-bx / 2.0),
                    num(top),
                    num(bx),
                    num(bx),
                    if node.shape == "iconRounded" { 8 } else { 0 },
                    view.style.shape
                );
            }
            let glyph = bx * 0.6;
            let _ = write!(
                inner,
                "{}",
                icon_svg(
                    ctx.host,
                    icon,
                    -glyph / 2.0,
                    top + (bx - glyph) / 2.0,
                    glyph
                )
            );
        } else {
            Shape::named(&view.shape).draw(
                &mut inner,
                Size::new(w, h),
                rewrite(
                    &mut shape_attrs,
                    format_args!("{SHAPE_ATTRS}{}", view.style.shape),
                ),
                rewrite(
                    &mut line_attrs,
                    format_args!("{LINE_ATTRS}{}", view.style.line),
                ),
            );
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
        let _ = write!(
            nodes_out,
            "<g class=\"{p}-node {p}-shape-{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">",
            view.shape,
            num(x),
            num(y)
        );
        match node.link.as_deref().filter(|l| !l.is_empty()) {
            Some(link) => {
                let target = node
                    .link_target
                    .as_deref()
                    .filter(|t| !t.is_empty())
                    .map(Esc);
                let target = fragment(move |f| match target {
                    Some(target) => write!(f, " target=\"{target}\""),
                    None => Ok(()),
                });
                let body = linked(body, Some(link), &node.id, &mut links, ctx, target);
                let _ = write!(nodes_out, "{body}</g>");
            }
            None => {
                let _ = write!(nodes_out, "{body}</g>");
            }
        }
    }

    let mut reach = SideReach::new();
    for (i, ce) in cedges.iter().enumerate() {
        if ce.src == ce.dst
            || drawn[i].thickness.as_deref() == Some("invisible")
            || ce.route.len() < 2
        {
            continue;
        }
        reach.add(
            ce.src,
            cnodes[ce.src].side_of(ce.route[0].at),
            Marker::from_name(&drawn[i].arrow_type_start),
        );
        reach.add(
            ce.dst,
            cnodes[ce.dst].side_of(ce.route[ce.route.len() - 1].at),
            Marker::from_name(&drawn[i].arrow_type_end),
        );
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let style = if !edge.style.is_empty() || !edge.css_compiled_styles.is_empty() {
            let mut all = edge.css_compiled_styles.clone();
            all.extend(edge.style.iter().cloned());
            Some(resolve_style(&all))
        } else {
            None
        };
        if edge.thickness.as_deref() != Some("invisible") {
            let start_marker = Marker::from_name(&edge.arrow_type_start);
            let end_marker = Marker::from_name(&edge.arrow_type_end);
            let (src, dst) = (cedges[i].src, cedges[i].dst);
            let path: EdgePath = if src == dst {
                let view = &views[src];
                let k = loop_seen.get(&src).copied().unwrap_or(0);
                loop_seen.insert(src, k + 1);
                let sideways = flows_sideways(&cnodes, src, root_dir);
                loop_path(
                    &cnodes[src],
                    Size::new(view.w, view.h),
                    k as f64,
                    sideways,
                    LOOP,
                    Marker::trim_of(end_marker),
                    &mut cedges[i],
                )
            } else {
                let mut route = cedges[i].route.clone();
                let last = route.len() - 1;
                let leaves = reach.of(src, cnodes[src].side_of(route[0].at));
                let arrives = reach.of(dst, cnodes[dst].side_of(route[last].at));
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
                    Marker::trim_of(start_marker),
                    Marker::trim_of(end_marker),
                    Some(leaves),
                    Some(arrives),
                )
            };
            let color = style
                .as_ref()
                .map_or(Cow::Borrowed("var(--_l)"), ResolvedStyle::line_color);
            let mut attrs = "";
            if edge.thickness.as_deref() == Some("thick") {
                attrs = " stroke-width=\"2.5\"";
            } else if edge.thickness.as_deref() == Some("dotted") {
                attrs = " stroke-dasharray=\"3 4\"";
            }
            let classes = class_names(&RE_EDGE_CLASSES.replace(&edge.classes, |_| String::new()));
            let _ = write!(
                edges_out,
                "<g class=\"{p}-edge{classes}\" data-id=\"{}\"{}><path d=\"{}\"{attrs}/>{}{}</g>",
                EscText(&edge.id),
                style.as_ref().map_or("", |s| s.line.as_str()),
                path.d,
                Marker::draw_of(start_marker, path.start, path.start_dir, &color),
                Marker::draw_of(end_marker, path.end, path.end_dir, &color),
            );
        }
        let label = &edge_labels[i];
        if label.width > 0.0 {
            let _ = write!(
                labels_out,
                "{}",
                edge_label_svg(
                    &edge.id,
                    label,
                    cedges[i].label_at,
                    style.as_ref().map_or("", |s| s.text.as_str())
                )
            );
        }
    }

    let title_attrs = crate::svg::builder::TITLE;
    let mut content = String::with_capacity(
        clusters.len()
            + edges_out.len()
            + struck_titles.len()
            + labels_out.len()
            + nodes_out.len()
            + 512,
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
    if !struck_titles.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-cluster-titles\">{struck_titles}</g>"
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
        variant.map_or("flowchart", |variant| variant.kind()),
        width,
        height,
        size,
        ctx,
        Accessible {
            acc_title: db.titles.acc_title.as_deref(),
            acc_descr: db.titles.acc_descr.as_deref(),
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
