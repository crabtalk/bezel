//! Pele's `diagrams/requirement/render.ts`.

use crate::geom::{Direction, Point, Side, Size};
use crate::svg::builder::rewrite;
use crate::svg::edges::Curve;
use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use indexmap::IndexMap;

use super::{
    db::RequirementModel,
    graph::{GraphEdge, GraphNode, build_requirement_graph},
};
use crate::Ctx;
use crate::{
    Config, Rendered,
    diagrams::common::fit_width::{runs_across, turn_to_fit},
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, compound_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{Esc, EscText, fragment, num, span_style, text_style},
        edges::{EdgePath, edge_label_svg, route_path},
        root::{Accessible, svg_document},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
    },
    text::{Host, Label, Span, Style},
};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 56.0;
const PAD_X: f64 = 14.0;
const PAD_Y: f64 = 10.0;
const ROW_GAP: f64 = 2.0;
const COLUMN_GAP: f64 = 12.0;
const MIN_WIDTH: f64 = 120.0;
const NAME_WRAP: f64 = 280.0;
const VALUE_WRAP: f64 = 240.0;
const LOOP: f64 = 26.0;
const CONTAINS_RADIUS: f64 = 6.0;
const ARROW_LENGTH: f64 = 9.0;
const ARROW_HALF: f64 = 4.5;
const END_STEP: f64 = 28.0;
const END_MARGIN: f64 = 14.0;

static RE_DEFAULT: Lazy = Lazy::new(r"^default\s?", "");

struct Row {
    key: Label,
    value: Label,
    height: f64,
}

struct NodeView<'g, 'a> {
    node: &'g GraphNode<'a>,
    style: ResolvedStyle,
    kind: Label,
    name: Label,
    rows: Vec<Row>,
    key_width: f64,
    head_height: f64,
    w: f64,
    h: f64,
}

/// Adds bold or italic to a whole label after its markdown is read, since
/// emphasis markers only toggle styles the label does not start with.
fn restyle(label: Label, bits: u8, measure: &impl Host) -> Label {
    if bits == 0 || label.lines.is_empty() {
        return label;
    }
    let mut width: f64 = 0.0;
    let mut widths = Vec::new();
    let lines: Vec<Vec<Span>> = label
        .lines
        .iter()
        .map(|line| {
            let mut total = 0.0;
            let spans: Vec<Span> = line
                .iter()
                .map(|span| {
                    let style = span.style | bits;
                    let w = if span.icon.is_some() {
                        span.width
                    } else {
                        measure.width(&span.text, label.size, style)
                    };
                    total += w;
                    Span {
                        style,
                        width: w,
                        ..span.clone()
                    }
                })
                .collect();
            widths.push(total);
            if total > width {
                width = total;
            }
            spans
        })
        .collect();
    Label {
        lines,
        widths,
        width,
        ..label
    }
}

/// A measured label with its lines starting at `x`, the first line's box
/// starting at `top`.
struct StartText<'a, A>(&'a Label, f64, f64, A);

impl<A: fmt::Display> fmt::Display for StartText<'_, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let StartText(label, x, top, ref attrs) = *self;
        let spans = || {
            label
                .lines
                .iter()
                .flatten()
                .any(|span| span.icon.is_none() && !span.text.is_empty())
        };
        if !spans() {
            return Ok(());
        }
        let shared = label.shared_style();
        let attrs = attrs.to_string();
        write!(
            f,
            "<text{} text-anchor=\"start\" xml:space=\"preserve\">",
            text_style(&attrs, shared)
        )?;
        let mut y = top + label.line_height / 2.0 + label.size * 0.35;
        for line in &label.lines {
            let mut first = true;
            for span in line {
                if span.icon.is_some() || span.text.is_empty() {
                    continue;
                }
                f.write_str("<tspan")?;
                if first {
                    write!(f, " x=\"{}\" y=\"{}\"", num(x), num(y))?;
                }
                write!(
                    f,
                    "{}>{}</tspan>",
                    span_style(span.style & !shared),
                    Esc(&span.text)
                )?;
                first = false;
            }
            y += label.line_height;
        }
        f.write_str("</text>")
    }
}

/// The containment mark: a circled plus whose rim touches (`x`, `y`), where
/// (`dx`, `dy`) points at the node.
fn contains_marker(x: f64, y: f64, dx: f64, dy: f64) -> impl fmt::Display {
    fragment(move |f| {
        let p = crate::PREFIX;
        let r = CONTAINS_RADIUS;
        let cx = x - dx * r;
        let cy = y - dy * r;
        write!(
            f,
            "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\"/><path class=\"{p}-marker\" d=\"M{}L{}M{}L{}\"/>",
            num(cx),
            num(cy),
            js::Number(r),
            Point::new(cx - dx * r, cy - dy * r),
            Point::new(cx + dx * r, cy + dy * r),
            Point::new(cx + dy * r, cy - dx * r),
            Point::new(cx - dy * r, cy + dx * r)
        )
    })
}

/// An open arrowhead with its tip at (`x`, `y`), pointing along (`dx`,
/// `dy`).
fn arrow_marker(x: f64, y: f64, dx: f64, dy: f64) -> impl fmt::Display {
    fragment(move |f| {
        let p = crate::PREFIX;
        let bx = x - dx * ARROW_LENGTH;
        let by = y - dy * ARROW_LENGTH;
        write!(
            f,
            "<path class=\"{p}-marker\" d=\"M{}L{}L{}\" stroke-linejoin=\"round\"/>",
            Point::new(bx - dy * ARROW_HALF, by + dx * ARROW_HALF),
            Point::new(x, y),
            Point::new(bx + dy * ARROW_HALF, by - dx * ARROW_HALF)
        )
    })
}

struct End {
    edge: usize,
    at: usize,
    toward: f64,
}

/// The layout attaches every edge to the middle of a node's side. Ends that
/// share a side are spread along it, in the order of where they head, so
/// their markers do not sit on each other.
fn spread_ends(cnodes: &[CNode], cedges: &mut [CEdge], views: &[NodeView]) {
    let mut sides: IndexMap<(usize, Side), Vec<End>> = IndexMap::new();
    for (e, edge) in cedges.iter().enumerate() {
        let route = &edge.route;
        if edge.src == edge.dst || route.len() < 2 {
            continue;
        }
        for at in [0, route.len() - 1] {
            let node = if at == 0 { edge.src } else { edge.dst };
            let next = route[if at == 0 { 1 } else { at - 1 }].at;
            let Some(side) = cnodes[node].side_of(route[at].at) else {
                continue;
            };
            let end = End {
                edge: e,
                at,
                toward: if side.is_horizontal() { next.x } else { next.y },
            };
            sides.entry((node, side)).or_default().push(end);
        }
    }
    for (&(node, side), list) in sides.iter_mut() {
        if list.len() < 2 {
            continue;
        }
        let vertical = !side.is_horizontal();
        let span = (if vertical {
            views[node].h
        } else {
            views[node].w
        }) - 2.0 * END_MARGIN;
        let step = END_STEP.min(span / (list.len() - 1) as f64).max(0.0);
        list.sort_by(|a, b| js::compare(a.toward - b.toward));
        let count = list.len() as f64;
        for (j, end) in list.iter().enumerate() {
            let offset = (j as f64 - (count - 1.0) / 2.0) * step;
            let point = &mut cedges[end.edge].route[end.at].at;
            if vertical {
                point.y = cnodes[node].center.y + offset;
            } else {
                point.x = cnodes[node].center.x + offset;
            }
        }
    }
}

pub fn render_requirement(
    model: &RequirementModel,
    config: &Config,
    ctx: &Ctx<'_, impl Host>,
) -> Rendered {
    if !runs_across(Some(&model.direction)) {
        return draw(model, config, ctx, false);
    }
    turn_to_fit(ctx, |down| draw(model, config, ctx, down))
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(
    model: &RequirementModel,
    _config: &Config,
    ctx: &Ctx<'_, impl Host>,
    turned: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let graph = build_requirement_graph(model);
    let size = ctx.options.font_size;
    let small = js::round(size * 0.875);
    let measure = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<NodeView> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();
    for node in &graph.nodes {
        let style = resolve_style(node.css_styles);
        let base = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let name_size = style.font_size.unwrap_or(size);
        let body_size = style.font_size.unwrap_or(small);
        let kind_text = format!(
            "«{}»",
            node.requirement
                .map_or("Element", |requirement| requirement.kind.name())
        );
        let kind = restyle(
            Label::layout(Some(&kind_text), false, measure, body_size, 4000.0, 0),
            base,
            measure,
        );
        let name = restyle(
            Label::layout(Some(node.id), true, measure, name_size, NAME_WRAP, 0),
            base | Style::BOLD,
            measure,
        );
        let fields: Vec<(&str, &str)> = match (node.requirement, node.element) {
            (Some(requirement), _) => vec![
                ("ID", &requirement.requirement_id),
                ("Text", &requirement.text),
                ("Risk", requirement.risk.map_or("", |risk| risk.name())),
                (
                    "Verification",
                    requirement.verify_method.map_or("", |method| method.name()),
                ),
            ],
            (None, Some(element)) => vec![("Type", &element.kind), ("Doc Ref", &element.doc_ref)],
            (None, None) => Vec::new(),
        };
        let mut rows = Vec::new();
        let mut key_width: f64 = 0.0;
        let mut value_width: f64 = 0.0;
        let mut body_height = 0.0;
        for (label, text) in fields {
            if text.is_empty() {
                continue;
            }
            let key = restyle(
                Label::layout(Some(label), false, measure, body_size, 4000.0, 0),
                base,
                measure,
            );
            let value = restyle(
                Label::layout(Some(text), true, measure, body_size, VALUE_WRAP, 0),
                base,
                measure,
            );
            let height = key.height.max(value.height);
            key_width = key_width.max(key.width);
            value_width = value_width.max(value.width);
            body_height += height + ROW_GAP;
            rows.push(Row { key, value, height });
        }
        if !rows.is_empty() {
            body_height += 2.0 * PAD_Y - ROW_GAP;
        }
        let head_height = 2.0 * PAD_Y + kind.height + name.height;
        let w = MIN_WIDTH
            .max(kind.width.max(name.width) + 2.0 * PAD_X)
            .max(if rows.is_empty() {
                0.0
            } else {
                key_width + COLUMN_GAP + value_width + 2.0 * PAD_X
            });
        // A requirement and an element may share a name; relationships then
        // go to the requirement.
        index.entry(node.id).or_insert(views.len());
        let mut c = CNode::new(Size::new(w, head_height + body_height), None);
        c.seq = views.len() as f64;
        views.push(NodeView {
            node,
            style,
            kind,
            name,
            rows,
            key_width,
            head_height,
            w,
            h: head_height + body_height,
        });
        cnodes.push(c);
    }

    let root_dir = if turned {
        Direction::TopToBottom
    } else {
        Direction::from_name(graph.direction)
    };
    let sideways = !root_dir.is_vertical();
    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&GraphEdge> = Vec::new();
    for edge in &graph.edges {
        let src = edge.start.and_then(|start| index.get(start).copied());
        let dst = edge.end.and_then(|end| index.get(end).copied());
        let (Some(src), Some(dst)) = (src, dst) else {
            continue;
        };
        let label = Label::layout(
            Some(&format!("«{}»", edge.kind)),
            false,
            measure,
            small,
            4000.0,
            0,
        );
        if src == dst {
            // A self-loop sits beside the node, across the flow, and the
            // node's box grows to hold it.
            if sideways {
                cnodes[src].size.h += 2.0 * (LOOP + label.height + 8.0);
            } else {
                cnodes[src].size.w += 2.0 * (LOOP + label.width + 12.0);
            }
        }
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1,
            label: Size::new(label.width + 12.0, label.height + 4.0),
            ..CEdge::default()
        });
        edge_labels.push(label);
    }

    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: NODE_SEP,
            edge_sep: EDGE_SEP,
            rank_sep: RANK_SEP,
            port_sep: 20.0,
            ..LayeredOptions::default()
        },
    );

    spread_ends(&cnodes, &mut cedges, &views);

    let title = Label::layout(
        model.titles.title.as_deref(),
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
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.h + title_height + 2.0 * pad).ceil();

    let mut nodes_out = String::new();
    let mut kind_attrs_buf = String::new();
    let mut name_attrs_buf = String::new();
    for (i, view) in views.iter().enumerate() {
        let (w, h, style) = (view.w, view.h, &view.style);
        let left = -w / 2.0;
        let top = -h / 2.0;
        let _ = write!(
            nodes_out,
            "<g class=\"{p}-node {p}-{}{}\" data-id=\"{}\" transform=\"translate({},{})\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
            if view.node.requirement.is_some() {
                "requirement"
            } else {
                "element"
            },
            class_names(&RE_DEFAULT.replace(&view.node.css_classes, |_| String::new())),
            EscText(view.node.id),
            num(cnodes[i].center.x + ox),
            num(cnodes[i].center.y + oy),
            num(left),
            num(top),
            num(w),
            num(h),
            style.shape
        );
        let kind_attrs = rewrite(
            &mut kind_attrs_buf,
            format_args!(
                " class=\"{p}-stereotype\" font-size=\"{}\" fill=\"var(--_m)\"{}",
                js::Number(view.kind.size),
                style.text
            ),
        );
        let name_attrs = rewrite(
            &mut name_attrs_buf,
            format_args!(
                " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"{}",
                js::Number(view.name.size),
                style.text
            ),
        );
        let _ = write!(
            nodes_out,
            "{}{}",
            view.kind.svg(
                Point::new(0.0, top + PAD_Y + view.kind.height / 2.0),
                kind_attrs
            ),
            view.name.svg(
                Point::new(0.0, top + PAD_Y + view.kind.height + view.name.height / 2.0),
                name_attrs
            )
        );
        if let Some(first) = view.rows.first() {
            let divider = top + view.head_height;
            let _ = write!(
                nodes_out,
                "<path class=\"{p}-divider\" d=\"M{}H{}\" stroke=\"var(--_b)\"{}/><g class=\"{p}-fields\" font-size=\"{}\">",
                Point::new(left, divider),
                num(-left),
                style.line,
                js::Number(first.key.size)
            );
            let mut y = divider + PAD_Y;
            for row in &view.rows {
                let _ = write!(
                    nodes_out,
                    "{}{}",
                    StartText(
                        &row.key,
                        left + PAD_X,
                        y,
                        format_args!(" class=\"{p}-field\" fill=\"var(--_m)\"{}", style.text),
                    ),
                    StartText(
                        &row.value,
                        left + PAD_X + view.key_width + COLUMN_GAP,
                        y,
                        format_args!(" class=\"{p}-value\"{}", style.text),
                    )
                );
                y += row.height + ROW_GAP;
            }
            nodes_out.push_str("</g>");
        }
        nodes_out.push_str("</g>");
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen: HashMap<usize, f64> = HashMap::new();
    for (i, ce) in cedges.iter().enumerate() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let start_trim = if edge.contains {
            2.0 * CONTAINS_RADIUS
        } else {
            0.0
        };
        let path: EdgePath;
        let mut label_x = ce.label_at.x + ox;
        let mut label_y = ce.label_at.y + oy;
        if ce.src == ce.dst {
            let view = &views[ce.src];
            let cx = cnodes[ce.src].center.x + ox;
            let cy = cnodes[ce.src].center.y + oy;
            let k = loop_seen.get(&ce.src).copied().unwrap_or(0.0);
            loop_seen.insert(ce.src, k + 1.0);
            let half = (if sideways { view.w } else { view.h }) / 2.0;
            let spread = (half - 4.0).min(12.0 + k * 6.0);
            let reach = LOOP + k * 8.0;
            // Local frame: `out` points away from the node, `along` runs
            // along its side.
            let at = |out: f64, along: f64| {
                if sideways {
                    Point::new(cx + along, cy + view.h / 2.0 + out)
                } else {
                    Point::new(cx + view.w / 2.0 + out, cy + along)
                }
            };
            // A cubic whose control points are this far out peaks at
            // `reach`.
            let pull = reach / 0.75;
            path = EdgePath {
                d: format!(
                    "M{}C{} {} {}",
                    at(start_trim, -spread),
                    at(pull, -spread),
                    at(pull, spread),
                    at(0.0, spread)
                ),
                start: if sideways {
                    Point::new(cx - spread, cy + view.h / 2.0)
                } else {
                    Point::new(cx + view.w / 2.0, cy - spread)
                },
                start_dir: if sideways {
                    Point::new(0.0, -1.0)
                } else {
                    Point::new(-1.0, 0.0)
                },
                end: if sideways {
                    Point::new(cx + spread, cy + view.h / 2.0)
                } else {
                    Point::new(cx + view.w / 2.0, cy + spread)
                },
                end_dir: if sideways {
                    Point::new(0.0, -1.0)
                } else {
                    Point::new(-1.0, 0.0)
                },
            };
            if sideways {
                label_x = cx;
                label_y = cy + view.h / 2.0 + reach + 4.0 + ce.label.h / 2.0;
            } else {
                label_x = cx + view.w / 2.0 + reach + 4.0 + ce.label.w / 2.0;
                label_y = cy;
            }
        } else {
            let mut route = ce.route.clone();
            for point in &mut route {
                point.at += Point::new(ox, oy);
            }
            path = route_path(&route, Curve::Smooth, start_trim, 0.0, None, None);
        }
        let _ = write!(
            edges_out,
            "<g class=\"{p}-edge {p}-relationship-{}\" data-id=\"{}\"><path d=\"{}\"{}/>{}</g>",
            if edge.contains { "contains" } else { "arrow" },
            EscText(&edge.id),
            path.d,
            if edge.contains {
                ""
            } else {
                " stroke-dasharray=\"5 4\""
            },
            fragment(|f| if edge.contains {
                write!(
                    f,
                    "{}",
                    contains_marker(
                        path.start.x,
                        path.start.y,
                        path.start_dir.x,
                        path.start_dir.y
                    )
                )
            } else {
                write!(
                    f,
                    "{}",
                    arrow_marker(path.end.x, path.end.y, path.end_dir.x, path.end_dir.y)
                )
            })
        );
        let _ = write!(
            labels_out,
            "{}",
            edge_label_svg(&edge.id, label, Point::new(label_x, label_y), "")
        );
    }

    let title_attrs = crate::svg::builder::TITLE;
    let mut content =
        String::with_capacity(edges_out.len() + labels_out.len() + nodes_out.len() + 512);
    let _ = write!(
        content,
        "{}",
        title.svg(
            Point::new(width / 2.0, pad + title.height / 2.0),
            title_attrs
        )
    );
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edge-labels\" font-size=\"{}\" fill=\"var(--_m)\">{labels_out}</g>",
            js::Number(small)
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");

    let svg = svg_document(
        "requirement",
        width,
        height,
        size,
        ctx,
        Accessible {
            acc_title: model.titles.acc_title.as_deref(),
            acc_descr: model.titles.acc_descr.as_deref(),
        },
        &content,
    );

    Rendered {
        svg,
        width,
        height,
        links: Vec::new(),
    }
}
