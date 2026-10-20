//! Pele's `diagrams/requirement/render.ts`.

use crate::geom::{Direction, Point, Size};
use crate::svg::builder::rewrite;
use crate::svg::edges::Curve;
use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::{
    db::RequirementModel,
    graph::{GraphEdge, GraphNode, NodeItem},
    types::Relationship,
};
use crate::Ctx;
use crate::{
    Config, Error, Rendered,
    diagrams::Render,
    js,
    layout::{
        compound::{CEdge, CNode, compound_layout, spread_ends},
        layered::LayeredOptions,
    },
    svg::{
        builder::{Esc, EscText, fragment, num, span_style, text_style},
        edges::{EdgePath, edge_label_svg, route_path},
        root::svg_document,
        theme::{RADIUS, ResolvedStyle, node_class_names, resolve_style},
    },
    text::{Host, Label, Style},
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
    size: Size,
}

/// A measured label with its lines starting at the point's `x`, the first
/// line's box starting at its `y`.
struct StartText<'a, A>(&'a Label, Point, A);

impl<A: fmt::Display> fmt::Display for StartText<'_, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let StartText(label, Point { x, y: top }, ref attrs) = *self;
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

/// The containment mark: a circled plus whose rim touches `tip`, where `dir`
/// points at the node.
fn contains_marker(tip: Point, dir: Point) -> impl fmt::Display {
    fragment(move |f| {
        let p = crate::PREFIX;
        let r = CONTAINS_RADIUS;
        let center = tip - dir * r;
        write!(
            f,
            "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\"/><path class=\"{p}-marker\" d=\"M{}L{}M{}L{}\"/>",
            num(center.x),
            num(center.y),
            js::Number(r),
            center - dir * r,
            center + dir * r,
            Point::new(center.x + dir.y * r, center.y - dir.x * r),
            Point::new(center.x - dir.y * r, center.y + dir.x * r)
        )
    })
}

/// An open arrowhead with its tip at `tip`, pointing along `dir`.
fn arrow_marker(tip: Point, dir: Point) -> impl fmt::Display {
    fragment(move |f| {
        let p = crate::PREFIX;
        let base = tip - dir * ARROW_LENGTH;
        write!(
            f,
            "<path class=\"{p}-marker\" d=\"M{}L{}L{}\" stroke-linejoin=\"round\"/>",
            Point::new(base.x - dir.y * ARROW_HALF, base.y + dir.x * ARROW_HALF),
            tip,
            Point::new(base.x + dir.y * ARROW_HALF, base.y - dir.x * ARROW_HALF)
        )
    })
}

impl<'g, 'a> NodeView<'g, 'a> {
    /// `node` measured: `size` is the diagram's font size, `small` the one
    /// its fields and edge labels use.
    fn measure(node: &'g GraphNode<'a>, size: f64, small: f64, measure: &impl Host) -> Self {
        let style = resolve_style(node.css_styles);
        let base = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let name_size = style.font_size.unwrap_or(size);
        let body_size = style.font_size.unwrap_or(small);
        let kind_text = format!(
            "«{}»",
            match node.item {
                NodeItem::Requirement(requirement) => requirement.kind.name(),
                NodeItem::Element(_) => "Element",
            }
        );
        let kind = Label::layout(Some(&kind_text), false, measure, body_size, 4000.0, 0)
            .restyled(base, measure);
        let name = Label::layout(Some(node.id), true, measure, name_size, NAME_WRAP, 0)
            .restyled(base | Style::BOLD, measure);
        let fields: Vec<(&str, &str)> = match node.item {
            NodeItem::Requirement(requirement) => vec![
                ("ID", &requirement.requirement_id),
                ("Text", &requirement.text),
                ("Risk", requirement.risk.map_or("", |risk| risk.name())),
                (
                    "Verification",
                    requirement.verify_method.map_or("", |method| method.name()),
                ),
            ],
            NodeItem::Element(element) => {
                vec![("Type", &element.kind), ("Doc Ref", &element.doc_ref)]
            }
        };
        let mut rows = Vec::new();
        let mut key_width: f64 = 0.0;
        let mut value_width: f64 = 0.0;
        let mut body_height = 0.0;
        for (label, text) in fields {
            if text.is_empty() {
                continue;
            }
            let key = Label::layout(Some(label), false, measure, body_size, 4000.0, 0)
                .restyled(base, measure);
            let value = Label::layout(Some(text), true, measure, body_size, VALUE_WRAP, 0)
                .restyled(base, measure);
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
        NodeView {
            node,
            style,
            kind,
            name,
            rows,
            key_width,
            head_height,
            size: Size::new(w, head_height + body_height),
        }
    }

    /// The node drawn with its center at `center`.
    fn svg(&self, out: &mut String, center: Point, attrs_buf: &mut [String; 2]) {
        let p = crate::PREFIX;
        let (Size { w, h }, style) = (self.size, &self.style);
        let left = -w / 2.0;
        let top = -h / 2.0;
        let _ = write!(
            out,
            "<g class=\"{p}-node {p}-{}{}\" data-id=\"{}\" transform=\"translate({},{})\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
            match self.node.item {
                NodeItem::Requirement(_) => "requirement",
                NodeItem::Element(_) => "element",
            },
            node_class_names(&self.node.css_classes),
            EscText(self.node.id),
            num(center.x),
            num(center.y),
            num(left),
            num(top),
            num(w),
            num(h),
            style.shape
        );
        let [kind_attrs_buf, name_attrs_buf] = attrs_buf;
        let kind_attrs = rewrite(
            kind_attrs_buf,
            format_args!(
                " class=\"{p}-stereotype\" font-size=\"{}\" fill=\"var(--_m)\"{}",
                js::Number(self.kind.size),
                style.text
            ),
        );
        let name_attrs = rewrite(
            name_attrs_buf,
            format_args!(
                " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"{}",
                js::Number(self.name.size),
                style.text
            ),
        );
        let _ = write!(
            out,
            "{}{}",
            self.kind.svg(
                Point::new(0.0, top + PAD_Y + self.kind.height / 2.0),
                kind_attrs
            ),
            self.name.svg(
                Point::new(0.0, top + PAD_Y + self.kind.height + self.name.height / 2.0),
                name_attrs
            )
        );
        if let Some(first) = self.rows.first() {
            let divider = top + self.head_height;
            let _ = write!(
                out,
                "<path class=\"{p}-divider\" d=\"M{}H{}\" stroke=\"var(--_b)\"{}/><g class=\"{p}-fields\" font-size=\"{}\">",
                Point::new(left, divider),
                num(-left),
                style.line,
                js::Number(first.key.size)
            );
            let mut y = divider + PAD_Y;
            for row in &self.rows {
                let _ = write!(
                    out,
                    "{}{}",
                    StartText(
                        &row.key,
                        Point::new(left + PAD_X, y),
                        format_args!(" class=\"{p}-field\" fill=\"var(--_m)\"{}", style.text),
                    ),
                    StartText(
                        &row.value,
                        Point::new(left + PAD_X + self.key_width + COLUMN_GAP, y),
                        format_args!(" class=\"{p}-value\"{}", style.text),
                    )
                );
                y += row.height + ROW_GAP;
            }
            out.push_str("</g>");
        }
        out.push_str("</g>");
    }
}

impl Render for RequirementModel {
    fn render(&self, _config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(if self.direction.is_vertical() {
            self.draw(ctx, false)
        } else {
            ctx.turn_to_fit(|down| self.draw(ctx, down))
        })
    }
}

impl RequirementModel {
    /// `turned` draws a diagram that runs across as one that runs down.
    fn draw(&self, ctx: &Ctx<'_, impl Host>, turned: bool) -> Rendered {
        let p = crate::PREFIX;
        let graph = self.graph();
        let size = ctx.options.font_size;
        let small = (size * 0.875).round();
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);

        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut views: Vec<NodeView> = Vec::new();
        let mut cnodes: Vec<CNode> = Vec::new();
        for node in &graph.nodes {
            let view = NodeView::measure(node, size, small, measure);
            // A requirement and an element may share a name; relationships
            // then go to the requirement.
            index.entry(node.id).or_insert(views.len());
            let mut c = CNode::new(view.size, None);
            c.seq = views.len() as f64;
            views.push(view);
            cnodes.push(c);
        }

        let root_dir = if turned {
            Direction::TopToBottom
        } else {
            self.direction
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
                Some(&format!("«{}»", edge.kind.name())),
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

        spread_ends(
            &cnodes,
            &mut cedges,
            |i| views[i].size,
            END_STEP,
            END_MARGIN,
        );

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);
        let inner = layout.w.max(title.width);
        let offset = Point::new(pad + (inner - layout.w) / 2.0, pad + title_height);
        let width = (inner + 2.0 * pad).ceil();
        let height = (layout.h + title_height + 2.0 * pad).ceil();

        let mut nodes_out = String::new();
        let mut attrs_buf = [String::new(), String::new()];
        for (view, c) in views.iter().zip(&cnodes) {
            view.svg(&mut nodes_out, c.center + offset, &mut attrs_buf);
        }

        let mut edges_out = String::new();
        let mut labels_out = String::new();
        let mut loop_seen: HashMap<usize, f64> = HashMap::new();
        for (i, ce) in cedges.iter().enumerate() {
            let edge = drawn[i];
            let label = &edge_labels[i];
            let contains = edge.kind == Relationship::Contains;
            let start_trim = if contains { 2.0 * CONTAINS_RADIUS } else { 0.0 };
            let path: EdgePath;
            let mut label_at = ce.label_at + offset;
            if ce.src == ce.dst {
                let view = &views[ce.src];
                let Size { w, h } = view.size;
                let center = cnodes[ce.src].center + offset;
                let k = loop_seen.get(&ce.src).copied().unwrap_or(0.0);
                loop_seen.insert(ce.src, k + 1.0);
                let half = (if sideways { h } else { w }) / 2.0;
                let spread = (half - 4.0).min(12.0 + k * 6.0);
                let reach = LOOP + k * 8.0;
                // Local frame: `out` points away from the node, `along` runs
                // along its side.
                let at = |out: f64, along: f64| {
                    if sideways {
                        Point::new(center.x + along, center.y + h / 2.0 + out)
                    } else {
                        Point::new(center.x + w / 2.0 + out, center.y + along)
                    }
                };
                // A cubic whose control points are this far out peaks at
                // `reach`.
                let pull = reach / 0.75;
                let inward = if sideways {
                    Point::new(0.0, -1.0)
                } else {
                    Point::new(-1.0, 0.0)
                };
                path = EdgePath {
                    d: format!(
                        "M{}C{} {} {}",
                        at(start_trim, -spread),
                        at(pull, -spread),
                        at(pull, spread),
                        at(0.0, spread)
                    ),
                    start: if sideways {
                        Point::new(center.x - spread, center.y + h / 2.0)
                    } else {
                        Point::new(center.x + w / 2.0, center.y - spread)
                    },
                    start_dir: inward,
                    end: if sideways {
                        Point::new(center.x + spread, center.y + h / 2.0)
                    } else {
                        Point::new(center.x + w / 2.0, center.y + spread)
                    },
                    end_dir: inward,
                };
                label_at = if sideways {
                    Point::new(
                        center.x,
                        center.y + h / 2.0 + reach + 4.0 + ce.label.h / 2.0,
                    )
                } else {
                    Point::new(
                        center.x + w / 2.0 + reach + 4.0 + ce.label.w / 2.0,
                        center.y,
                    )
                };
            } else {
                let mut route = ce.route.clone();
                for point in &mut route {
                    point.at += offset;
                }
                path = route_path(&route, Curve::Smooth, start_trim, 0.0, None, None);
            }
            let _ = write!(
                edges_out,
                "<g class=\"{p}-edge {p}-relationship-{}\" data-id=\"{}\"><path d=\"{}\"{}/>{}</g>",
                if contains { "contains" } else { "arrow" },
                EscText(&edge.id),
                path.d,
                if contains {
                    ""
                } else {
                    " stroke-dasharray=\"5 4\""
                },
                fragment(|f| if contains {
                    write!(f, "{}", contains_marker(path.start, path.start_dir))
                } else {
                    write!(f, "{}", arrow_marker(path.end, path.end_dir))
                })
            );
            let _ = write!(
                labels_out,
                "{}",
                edge_label_svg(&edge.id, label, label_at, "")
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
            self.titles.accessible(),
            &content,
        );

        Rendered {
            svg,
            width,
            height,
            links: Vec::new(),
        }
    }
}
