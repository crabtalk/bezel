//! Pele's `diagrams/requirement/render.ts`.

use std::{collections::HashMap, fmt::Write as _};

use indexmap::IndexMap;

use super::{
    db::RequirementModel,
    graph::{GraphEdge, GraphNode, build_requirement_graph},
};
use crate::mermaid::{
    Config, Options, Rendered,
    diagrams::common::fit_width::{runs_across, turn_to_fit},
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, Dir, cnode, compound_layout, direction},
        layered::LayeredOptions,
    },
    svg::{
        builder::{esc, esc_text, label_svg, num, shared_style, span_style, text_style},
        edges::{EdgePath, edge_label_svg, route_path},
        root::{Accessible, svg_document},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
    },
    text::{Label, Measure, Span, Style, layout_label},
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
fn restyle(label: Label, bits: u8, measure: &dyn Measure) -> Label {
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
fn start_text(label: &Label, x: f64, top: f64, attrs: &str) -> String {
    let mut y = top + label.line_height / 2.0 + label.size * 0.35;
    let shared = shared_style(label);
    let mut body = String::new();
    for line in &label.lines {
        let mut first = true;
        for span in line {
            if span.icon.is_some() || span.text.is_empty() {
                continue;
            }
            let pos = if first {
                format!(" x=\"{}\" y=\"{}\"", num(x), num(y))
            } else {
                String::new()
            };
            let _ = write!(
                body,
                "<tspan{pos}{}>{}</tspan>",
                span_style(span.style & !shared),
                esc(&span.text)
            );
            first = false;
        }
        y += label.line_height;
    }
    if body.is_empty() {
        String::new()
    } else {
        format!(
            "<text{} text-anchor=\"start\" xml:space=\"preserve\">{body}</text>",
            text_style(attrs, shared)
        )
    }
}

/// The containment mark: a circled plus whose rim touches (`x`, `y`), where
/// (`dx`, `dy`) points at the node.
fn contains_marker(x: f64, y: f64, dx: f64, dy: f64) -> String {
    let p = crate::mermaid::PREFIX;
    let r = CONTAINS_RADIUS;
    let cx = x - dx * r;
    let cy = y - dy * r;
    format!(
        "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\"/><path class=\"{p}-marker\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
        num(cx),
        num(cy),
        js::num_str(r),
        num(cx - dx * r),
        num(cy - dy * r),
        num(cx + dx * r),
        num(cy + dy * r),
        num(cx + dy * r),
        num(cy - dx * r),
        num(cx - dy * r),
        num(cy + dx * r)
    )
}

/// An open arrowhead with its tip at (`x`, `y`), pointing along (`dx`,
/// `dy`).
fn arrow_marker(x: f64, y: f64, dx: f64, dy: f64) -> String {
    let p = crate::mermaid::PREFIX;
    let bx = x - dx * ARROW_LENGTH;
    let by = y - dy * ARROW_LENGTH;
    format!(
        "<path class=\"{p}-marker\" d=\"M{},{}L{},{}L{},{}\" stroke-linejoin=\"round\"/>",
        num(bx - dy * ARROW_HALF),
        num(by + dx * ARROW_HALF),
        num(x),
        num(y),
        num(bx + dy * ARROW_HALF),
        num(by - dx * ARROW_HALF)
    )
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
    let mut sides: IndexMap<usize, Vec<End>> = IndexMap::new();
    for (e, edge) in cedges.iter().enumerate() {
        let route = &edge.route;
        if edge.src == edge.dst || route.len() < 6 {
            continue;
        }
        for at in [0, route.len() - 3] {
            let node = if at == 0 { edge.src } else { edge.dst };
            let c = &cnodes[node];
            let dx = route[at] - c.x;
            let dy = route[at + 1] - c.y;
            let next = if at == 0 { 3 } else { at - 3 };
            let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
                if dy < 0.0 { 0 } else { 2 }
            } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
                if dx < 0.0 { 3 } else { 1 }
            } else {
                continue;
            };
            let end = End {
                edge: e,
                at,
                toward: route[if side % 2 == 0 { next } else { next + 1 }],
            };
            sides.entry(node * 4 + side).or_default().push(end);
        }
    }
    for (&key, list) in sides.iter_mut() {
        if list.len() < 2 {
            continue;
        }
        let node = key >> 2;
        let vertical = key % 2 == 1;
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
            if vertical {
                cedges[end.edge].route[end.at + 1] = cnodes[node].y + offset;
            } else {
                cedges[end.edge].route[end.at] = cnodes[node].x + offset;
            }
        }
    }
}

pub fn render_requirement(
    model: &RequirementModel,
    config: &Config,
    options: &Options,
) -> Rendered {
    if !runs_across(Some(&model.direction)) {
        return draw(model, config, options, false);
    }
    turn_to_fit(options, |down| draw(model, config, options, down))
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(model: &RequirementModel, _config: &Config, options: &Options, turned: bool) -> Rendered {
    let p = crate::mermaid::PREFIX;
    let graph = build_requirement_graph(model);
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let measure = options.measurer();
    let measure = &*measure;
    let pad = options.padding.unwrap_or(8.0);

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
                .map_or("Element", |requirement| &requirement.kind)
        );
        let kind = restyle(
            layout_label(Some(&kind_text), false, measure, body_size, 4000.0, 0),
            base,
            measure,
        );
        let name = restyle(
            layout_label(Some(node.id), true, measure, name_size, NAME_WRAP, 0),
            base | Style::BOLD,
            measure,
        );
        let fields: Vec<(&str, &str)> = match (node.requirement, node.element) {
            (Some(requirement), _) => vec![
                ("ID", &requirement.requirement_id),
                ("Text", &requirement.text),
                ("Risk", &requirement.risk),
                ("Verification", &requirement.verify_method),
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
                layout_label(Some(label), false, measure, body_size, 4000.0, 0),
                base,
                measure,
            );
            let value = restyle(
                layout_label(Some(text), true, measure, body_size, VALUE_WRAP, 0),
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
        let mut c = cnode(w, head_height + body_height, -1);
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
        Dir::TB
    } else {
        direction(Some(graph.direction))
    };
    let sideways = root_dir == Dir::LR || root_dir == Dir::RL;
    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&GraphEdge> = Vec::new();
    for edge in &graph.edges {
        let src = edge.start.and_then(|start| index.get(start).copied());
        let dst = edge.end.and_then(|end| index.get(end).copied());
        let (Some(src), Some(dst)) = (src, dst) else {
            continue;
        };
        let label = layout_label(
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
                cnodes[src].h += 2.0 * (LOOP + label.height + 8.0);
            } else {
                cnodes[src].w += 2.0 * (LOOP + label.width + 12.0);
            }
        }
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1,
            label_w: label.width + 12.0,
            label_h: label.height + 4.0,
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
            node_sep: NODE_SEP,
            edge_sep: EDGE_SEP,
            rank_sep: RANK_SEP,
            port_sep: 20.0,
            ..LayeredOptions::default()
        },
    );

    spread_ends(&cnodes, &mut cedges, &views);

    let title = layout_label(
        model.title.as_deref(),
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
    let inner = layout.width.max(title.width);
    let ox = pad + (inner - layout.width) / 2.0;
    let oy = pad + title_height;
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();

    let mut nodes_out = String::new();
    for (i, view) in views.iter().enumerate() {
        let (w, h, style) = (view.w, view.h, &view.style);
        let left = -w / 2.0;
        let top = -h / 2.0;
        let mut body = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
            num(left),
            num(top),
            num(w),
            num(h),
            style.shape
        );
        body.push_str(&label_svg(
            &view.kind,
            0.0,
            top + PAD_Y + view.kind.height / 2.0,
            &format!(
                " class=\"{p}-stereotype\" font-size=\"{}\" fill=\"var(--_m)\"{}",
                js::num_str(view.kind.size),
                style.text
            ),
        ));
        body.push_str(&label_svg(
            &view.name,
            0.0,
            top + PAD_Y + view.kind.height + view.name.height / 2.0,
            &format!(
                " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"{}",
                js::num_str(view.name.size),
                style.text
            ),
        ));
        if let Some(first) = view.rows.first() {
            let divider = top + view.head_height;
            let _ = write!(
                body,
                "<path class=\"{p}-divider\" d=\"M{},{}H{}\" stroke=\"var(--_b)\"{}/>",
                num(left),
                num(divider),
                num(-left),
                style.line
            );
            let mut y = divider + PAD_Y;
            let mut rows = String::new();
            for row in &view.rows {
                rows.push_str(&start_text(
                    &row.key,
                    left + PAD_X,
                    y,
                    &format!(" class=\"{p}-field\" fill=\"var(--_m)\"{}", style.text),
                ));
                rows.push_str(&start_text(
                    &row.value,
                    left + PAD_X + view.key_width + COLUMN_GAP,
                    y,
                    &format!(" class=\"{p}-value\"{}", style.text),
                ));
                y += row.height + ROW_GAP;
            }
            let _ = write!(
                body,
                "<g class=\"{p}-fields\" font-size=\"{}\">{rows}</g>",
                js::num_str(first.key.size)
            );
        }
        let _ = write!(
            nodes_out,
            "<g class=\"{p}-node {p}-{}{}\" data-id=\"{}\" transform=\"translate({},{})\">{body}</g>",
            if view.node.requirement.is_some() {
                "requirement"
            } else {
                "element"
            },
            class_names(&RE_DEFAULT.replace(&view.node.css_classes, |_| String::new())),
            esc_text(view.node.id),
            num(cnodes[i].x + ox),
            num(cnodes[i].y + oy)
        );
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
        let mut label_x = ce.label_x + ox;
        let mut label_y = ce.label_y + oy;
        if ce.src == ce.dst {
            let view = &views[ce.src];
            let cx = cnodes[ce.src].x + ox;
            let cy = cnodes[ce.src].y + oy;
            let k = loop_seen.get(&ce.src).copied().unwrap_or(0.0);
            loop_seen.insert(ce.src, k + 1.0);
            let half = (if sideways { view.w } else { view.h }) / 2.0;
            let spread = (half - 4.0).min(12.0 + k * 6.0);
            let reach = LOOP + k * 8.0;
            // Local frame: `out` points away from the node, `along` runs
            // along its side.
            let at = |out: f64, along: f64| -> String {
                if sideways {
                    format!("{},{}", num(cx + along), num(cy + view.h / 2.0 + out))
                } else {
                    format!("{},{}", num(cx + view.w / 2.0 + out), num(cy + along))
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
                sx: if sideways {
                    cx - spread
                } else {
                    cx + view.w / 2.0
                },
                sy: if sideways {
                    cy + view.h / 2.0
                } else {
                    cy - spread
                },
                sdx: if sideways { 0.0 } else { -1.0 },
                sdy: if sideways { -1.0 } else { 0.0 },
                ex: if sideways {
                    cx + spread
                } else {
                    cx + view.w / 2.0
                },
                ey: if sideways {
                    cy + view.h / 2.0
                } else {
                    cy + spread
                },
                edx: if sideways { 0.0 } else { -1.0 },
                edy: if sideways { -1.0 } else { 0.0 },
            };
            if sideways {
                label_x = cx;
                label_y = cy + view.h / 2.0 + reach + 4.0 + ce.label_h / 2.0;
            } else {
                label_x = cx + view.w / 2.0 + reach + 4.0 + ce.label_w / 2.0;
                label_y = cy;
            }
        } else {
            let mut route = ce.route.clone();
            for k in (0..route.len()).step_by(3) {
                route[k] += ox;
                route[k + 1] += oy;
            }
            path = route_path(&route, None, start_trim, 0.0, None, None);
        }
        let _ = write!(
            edges_out,
            "<g class=\"{p}-edge {p}-relationship-{}\" data-id=\"{}\"><path d=\"{}\"{}/>{}</g>",
            if edge.contains { "contains" } else { "arrow" },
            esc_text(&edge.id),
            path.d,
            if edge.contains {
                ""
            } else {
                " stroke-dasharray=\"5 4\""
            },
            if edge.contains {
                contains_marker(path.sx, path.sy, path.sdx, path.sdy)
            } else {
                arrow_marker(path.ex, path.ey, path.edx, path.edy)
            }
        );
        labels_out.push_str(&edge_label_svg(&edge.id, label, label_x, label_y, ""));
    }

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
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
            js::num_str(small)
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");

    let svg = svg_document(
        "requirement",
        width,
        height,
        size,
        options,
        Accessible {
            acc_title: model.acc_title.as_deref(),
            acc_descr: model.acc_descr.as_deref(),
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
