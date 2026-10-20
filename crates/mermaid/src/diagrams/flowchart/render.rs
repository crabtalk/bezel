//! Pele's `diagrams/flowchart/render.ts`.

use crate::svg::theme::node_class_names;
use std::{borrow::Cow, collections::HashMap, fmt::Write as _};

use super::{
    FlowchartConfig,
    db::FlowDb,
    graph::{FlowGraph, GraphEdge, GraphNode},
    shapes::{IconFrame, NodeShape},
    types::{LabelType, Stroke},
};
use crate::{
    Config, Ctx, Error, Host, Link, Rendered,
    config::positive,
    diagrams::{Render, common::icon::icon_svg},
    geom::{Direction, Point, Size},
    js::{self},
    layout::{
        compound::{CEdge, CNode, compound_layout, level_dir, shift_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{Esc, EscText, TITLE, fragment, num, rewrite},
        cluster::{Crossings, struck_title},
        edges::{EdgePath, Marker, SideReach, edge_label_svg, loop_path, route_path},
        root::svg_document,
        shapes::ShapeView,
        theme::{RADIUS, ResolvedStyle, resolve_style_over},
    },
    text::{Label, Style, decode_entities},
    util::url::{NoteLink, image_url, linked, safe_url},
};

pub(crate) const NODE_SEP: f64 = 40.0;
pub(crate) const EDGE_SEP: f64 = 16.0;
pub(crate) const RANK_SEP: f64 = 48.0;
pub(crate) const GROUP_PAD: f64 = 20.0;
pub(crate) const LOOP: f64 = 26.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE_ATTRS: &str = " stroke=\"var(--_b)\"";

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

/// The layout box of a group titled `label`, `pad` in from its members at
/// the sides and the bottom.
pub(crate) fn group_box(dir: Option<Direction>, label: &Label, pad: f64) -> CNode {
    let mut c = CNode::group(
        dir,
        pad,
        if label.height > 0.0 {
            label.height + 16.0
        } else {
            GROUP_PAD
        },
        pad,
    );
    c.min_w = label.width + 2.0 * pad - 12.0;
    c
}

/// Widens `c`, or lengthens it when the flow runs `sideways`, to hold a
/// self-loop labelled `label` on each side.
pub(crate) fn hold_loop(c: &mut CNode, label: &Label, sideways: bool) {
    if sideways {
        c.size.h += 2.0 * (LOOP + label.spaced_height(8.0));
    } else {
        c.size.w += 2.0
            * (LOOP
                + if label.width > 0.0 {
                    label.width + 12.0
                } else {
                    0.0
                });
    }
}

/// The style an edge's classes and own declarations give it, if any.
pub(crate) fn edge_style(compiled: &[String], own: &[String]) -> Option<ResolvedStyle> {
    (!own.is_empty() || !compiled.is_empty()).then(|| resolve_style_over(compiled, own))
}

/// Moves a laid-out drawing of `layout` below `title` and `pad` in from the
/// edges, centred under a wider title; the size of the whole.
pub(crate) fn frame(
    layout: Size,
    title: &Label,
    pad: f64,
    nodes: &mut [CNode],
    edges: &mut [CEdge],
) -> Size {
    let title_height = title.spaced_height(12.0);
    let inner = layout.w.max(title.width);
    let ox = pad + (inner - layout.w) / 2.0;
    let oy = pad + title_height;
    shift_layout(nodes, edges, Point::new(ox, oy));
    Size::new(
        (inner + 2.0 * pad).ceil(),
        (layout.h + title_height + 2.0 * pad).ceil(),
    )
}

/// The drawn layers of a flowchart-like diagram, each empty or its
/// elements.
#[derive(Default)]
pub(crate) struct Layers {
    pub clusters: String,
    pub edges: String,
    pub struck_titles: String,
    pub labels: String,
    pub nodes: String,
}

impl Layers {
    /// The layers under `title`, in a drawing `width` wide at a margin of
    /// `pad`, with edge labels at `edge_size`.
    pub fn content(&self, title: &Label, width: f64, pad: f64, edge_size: f64) -> String {
        let p = crate::PREFIX;
        let mut content = String::with_capacity(
            self.clusters.len()
                + self.edges.len()
                + self.struck_titles.len()
                + self.labels.len()
                + self.nodes.len()
                + 512,
        );
        let _ = write!(
            content,
            "{}",
            title.svg(Point::new(width / 2.0, pad + title.height / 2.0), TITLE)
        );
        if !self.clusters.is_empty() {
            let _ = write!(content, "<g class=\"{p}-clusters\">{}</g>", self.clusters);
        }
        if !self.edges.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{}</g>",
                self.edges
            );
        }
        if !self.struck_titles.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-cluster-titles\">{}</g>",
                self.struck_titles
            );
        }
        if !self.labels.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-edge-labels\" font-size=\"{}\">{}</g>",
                js::Number(edge_size),
                self.labels
            );
        }
        let _ = write!(content, "<g class=\"{p}-nodes\">{}</g>", self.nodes);
        content
    }
}

struct NodeView<'g> {
    node: &'g GraphNode,
    style: ResolvedStyle,
    label: Label,
    /// The drawn box; empty for a group.
    size: Size,
    dy: f64,
}

impl NodeView<'_> {
    fn shape_view(&self) -> ShapeView {
        ShapeView {
            shape: self.node.shape.shape(),
            size: self.size,
        }
    }
}

impl Render for FlowDb {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(self.render_as(&config.flowchart, ctx, None))
    }
}

impl FlowDb {
    /// The diagram drawn with `config`, as `variant` where there is one.
    pub fn render_as(
        &self,
        config: &FlowchartConfig,
        ctx: &Ctx<'_, impl Host>,
        variant: Option<&dyn FlowVariant>,
    ) -> Rendered {
        let renderer = FlowRenderer {
            db: self,
            config,
            ctx,
            variant,
        };
        let fitted = |down: bool| ctx.tighten(|tight| renderer.draw(down, tight));
        if self
            .direction
            .is_some_and(|dir| !dir.direction().is_vertical())
        {
            ctx.turn_to_fit(fitted)
        } else {
            fitted(false)
        }
    }
}

struct FlowRenderer<'a, H> {
    db: &'a FlowDb,
    config: &'a FlowchartConfig,
    ctx: &'a Ctx<'a, H>,
    variant: Option<&'a dyn FlowVariant>,
}

impl<H: Host> FlowRenderer<'_, H> {
    /// `turned` draws a flowchart that runs across as one that runs down.
    /// `tight` sets nodes closer together.
    fn draw(&self, turned: bool, tight: bool) -> Rendered {
        let (db, flow, ctx, variant) = (self.db, self.config, self.ctx, self.variant);
        let p = crate::PREFIX;
        let group_pad = if tight { 12.0 } else { GROUP_PAD };
        let curve_option = flow.curve.as_deref();
        let mut graph = db.graph(curve_option);
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
                    if variant.is_some() && tight {
                        80f64.max(ctx.title_room() / 2.0)
                    } else {
                        4000.0
                    },
                    text_style,
                );
                c = group_box(node.dir.map(|dir| dir.direction()), &label, group_pad);
                view = NodeView {
                    node,
                    style,
                    label,
                    size: Size::default(),
                    dy: 0.0,
                };
            } else {
                let assets = &node.assets;
                let pictured = assets.image().is_some() || assets.icon().is_some();
                let text = if pictured || node.shape.shape().has_label() {
                    node.label.as_deref()
                } else {
                    None
                };
                let label =
                    Label::layout(text, markdown, measure, font_size, wrap_width, text_style);
                let size: Size;
                let mut dy: f64;
                if pictured {
                    let asset = assets.size(if assets.image().is_some() { 80.0 } else { 48.0 });
                    size = Size::new(
                        asset.w.max(label.width + 8.0),
                        asset.h + label.spaced_height(6.0),
                    );
                    dy = if assets.on_top() {
                        -asset.h / 2.0
                    } else {
                        asset.h / 2.0
                    };
                    if label.height == 0.0 {
                        dy = 0.0;
                    }
                } else {
                    let s = node
                        .shape
                        .shape()
                        .size(Size::new(label.width, label.height));
                    size = s.size;
                    dy = s.dy;
                }
                view = NodeView {
                    node,
                    style,
                    label,
                    size,
                    dy,
                };
                c = CNode::new(size, None);
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
            db.direction.map(|dir| dir.direction()).unwrap_or_default()
        };
        let sideways = |cnodes: &[CNode], i: usize| !level_dir(cnodes, i, root_dir).is_vertical();
        // The edges on one side of a node spread along it; a picture or an
        // icon keeps them at its middle.
        for (i, view) in views.iter().enumerate() {
            let node = view.node;
            if node.is_group {
                cnodes[i].span = f64::INFINITY;
            } else if node.assets.image().is_none() && node.assets.icon().is_none() {
                cnodes[i].span = node.shape.span(view.size, sideways(&cnodes, i));
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
            Some(variant) => {
                variant.layout(&mut cnodes, &mut cedges, root_dir, &layout_options, tight)
            }
            None => compound_layout(&mut cnodes, &mut cedges, root_dir, &layout_options),
        };

        let title = Label::layout(
            db.titles.title.as_deref(),
            false,
            measure,
            size,
            if tight { ctx.title_room() } else { 4000.0 },
            Style::BOLD,
        );
        let Size {
            w: width,
            h: height,
        } = frame(layout, &title, pad, &mut cnodes, &mut cedges);
        let mut links: Vec<Link> = Vec::new();

        let mut crossings: Crossings = Crossings::new();
        if cnodes.iter().any(|c| c.is_group) {
            for e in &cedges {
                crossings.mark(&e.route);
            }
        }

        let mut layers = Layers::default();
        let mut inner = String::new();
        let mut title_attrs_buf = String::new();
        let mut label_attrs_buf = String::new();
        let mut line_attrs = String::new();
        let mut shape_attrs = String::new();
        for (i, view) in views.iter().enumerate() {
            let c = &cnodes[i];
            let node = view.node;
            let Point { x, y } = c.center;
            let classes = node_class_names(&node.css_classes);
            let id = EscText(&node.id);
            if c.is_group
                && let Some(variant) = variant.filter(|v| v.draws_groups())
                && c.parent.is_none()
            {
                variant.draw_group(
                    &mut layers.clusters,
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
                        layers.struck_titles,
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
                    layers.clusters,
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

            let h = view.size.h;
            let assets = &node.assets;
            inner.clear();
            if let Some(img) = assets.image() {
                let asset = assets.size(80.0);
                let top = if view.label.height > 0.0 {
                    if assets.on_top() {
                        h / 2.0 - asset.h
                    } else {
                        -h / 2.0
                    }
                } else {
                    -asset.h / 2.0
                };
                if let Some(src) = image_url(&safe_url(img), ctx) {
                    let _ = write!(
                        inner,
                        "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{}\"/>",
                        Esc(&src),
                        num(-asset.w / 2.0),
                        num(top),
                        num(asset.w),
                        num(asset.h),
                        if assets.constraint.as_deref() == Some("on") {
                            "xMidYMid meet"
                        } else {
                            "none"
                        }
                    );
                }
            } else if let Some(icon) = assets.icon() {
                let bx = assets.size(48.0).h;
                let top = if view.label.height > 0.0 {
                    if assets.on_top() {
                        h / 2.0 - bx
                    } else {
                        -h / 2.0
                    }
                } else {
                    -bx / 2.0
                };
                match node.shape {
                    NodeShape::Icon(IconFrame::Circle) => {
                        let _ = write!(
                            inner,
                            "<circle cy=\"{}\" r=\"{}\"{SHAPE_ATTRS}{}/>",
                            num(top + bx / 2.0),
                            num(bx / 2.0),
                            view.style.shape
                        );
                    }
                    NodeShape::Icon(IconFrame::None) => {}
                    shape => {
                        let _ = write!(
                            inner,
                            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{SHAPE_ATTRS}{}/>",
                            num(-bx / 2.0),
                            num(top),
                            num(bx),
                            num(bx),
                            if shape == NodeShape::Icon(IconFrame::Rounded) {
                                8
                            } else {
                                0
                            },
                            view.style.shape
                        );
                    }
                }
                let glyph = bx * 0.6;
                let _ = write!(
                    inner,
                    "{}",
                    icon_svg(
                        ctx.host,
                        icon,
                        Point::new(-glyph / 2.0, top + (bx - glyph) / 2.0),
                        glyph
                    )
                );
            } else {
                node.shape.shape().draw(
                    &mut inner,
                    view.size,
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
                .filter(|label| {
                    !label.is_empty() && ctx.options.links && NoteLink::marked(&classes)
                })
                .map(|label| NoteLink::new(decode_entities(label), &node.id, &mut links, ctx));
            let body = fragment(|f| {
                if let Some(tooltip) = &node.tooltip {
                    write!(f, "<title>{}</title>", EscText(tooltip))?;
                }
                f.write_str(&inner)?;
                match &note {
                    Some(note) => write!(f, "{}", note.around(text, ctx)),
                    None => write!(f, "{text}"),
                }
            });
            let _ = write!(
                layers.nodes,
                "<g class=\"{p}-node {p}-shape-{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">",
                node.shape.name(),
                num(x),
                num(y)
            );
            let target = node.link_target.as_deref().map(Esc);
            let target = fragment(move |f| match target {
                Some(target) => write!(f, " target=\"{target}\""),
                None => Ok(()),
            });
            let body = linked(
                body,
                node.link.as_deref(),
                &node.id,
                &mut links,
                ctx,
                target,
            );
            let _ = write!(layers.nodes, "{body}</g>");
        }

        let mut reach = SideReach::new();
        for (i, ce) in cedges.iter().enumerate() {
            if ce.src == ce.dst
                || drawn[i].thickness == Some(Stroke::Invisible)
                || ce.route.len() < 2
            {
                continue;
            }
            reach.add(
                ce.src,
                cnodes[ce.src].side_of(ce.route[0].at),
                drawn[i].arrow_type_start,
            );
            reach.add(
                ce.dst,
                cnodes[ce.dst].side_of(ce.route[ce.route.len() - 1].at),
                drawn[i].arrow_type_end,
            );
        }

        let mut loop_seen: HashMap<usize, usize> = HashMap::new();
        for i in 0..cedges.len() {
            let edge = drawn[i];
            let style = edge_style(&edge.css_compiled_styles, &edge.style);
            if edge.thickness != Some(Stroke::Invisible) {
                let start_marker = edge.arrow_type_start;
                let end_marker = edge.arrow_type_end;
                let (src, dst) = (cedges[i].src, cedges[i].dst);
                let path: EdgePath = if src == dst {
                    let k = loop_seen.get(&src).copied().unwrap_or(0);
                    loop_seen.insert(src, k + 1);
                    let sideways = sideways(&cnodes, src);
                    loop_path(
                        &cnodes[src],
                        views[src].size,
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
                        edge.curve,
                        Marker::trim_of(start_marker),
                        Marker::trim_of(end_marker),
                        Some(leaves),
                        Some(arrives),
                    )
                };
                let color = style
                    .as_ref()
                    .map_or(Cow::Borrowed("var(--_l)"), ResolvedStyle::line_color);
                let attrs = match edge.thickness {
                    Some(Stroke::Thick) => " stroke-width=\"2.5\"",
                    Some(Stroke::Dotted) => " stroke-dasharray=\"3 4\"",
                    _ => "",
                };
                let _ = write!(
                    layers.edges,
                    "<g class=\"{p}-edge\" data-id=\"{}\"{}><path d=\"{}\"{attrs}/>{}{}</g>",
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
                    layers.labels,
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

        let content = layers.content(&title, width, pad, edge_size);
        let svg = svg_document(
            variant.map_or("flowchart", |variant| variant.kind()),
            width,
            height,
            size,
            ctx,
            db.titles.accessible(),
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
