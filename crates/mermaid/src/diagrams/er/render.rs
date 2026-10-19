//! Pele's `diagrams/er/render.ts`.

use crate::svg::builder::rewrite;
use crate::svg::builder::{Esc, EscText, LabelSvg, Point, fragment, icon_element};
use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use indexmap::IndexMap;

use super::{
    db::ErModel,
    graph::{ErGraphEdge, ErGraphNode, build_er_graph},
    markers::{MARKER_LENGTH, er_marker},
};
use crate::{
    Config, Link, Rendered,
    diagrams::common::{
        fit_width::{runs_across, turn_to_fit},
        generics::parse_generic_types,
    },
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, compound_layout, direction, shift_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{num, shared_style, span_style, text_style},
        cluster::{Crossings, cluster_title_x, mark_crossings},
        edges::{edge_label_svg, route_path},
        root::{Accessible, svg_document},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
    },
    text::{Label, Style, decode_entities, layout_label},
    util::url::NoteLink,
};
use crate::{Ctx, Host};

const NODE_SEP: f64 = 48.0;
const EDGE_SEP: f64 = 20.0;
const RANK_SEP: f64 = 64.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 28.0;
const LOOP_BAND: f64 = 38.0;
const MIN_WIDTH: f64 = 96.0;
const BOX_PAD_X: f64 = 18.0;
const BOX_PAD_Y: f64 = 12.0;
const HEAD_PAD_Y: f64 = 9.0;
const CELL_PAD_X: f64 = 12.0;
const CELL_GAP: f64 = 16.0;
const ROW_PAD: f64 = 9.0;
const COMMENT_WRAP: f64 = 260.0;
const NO_WRAP: f64 = 1e6;
const PORT_GAP: f64 = 18.0;
const PORT_MARGIN: f64 = 14.0;
const BOX_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";

static RE_DEFAULT: Lazy = Lazy::new(r"^default\s?", "");

/// Type, name, keys, comment.
type Cells = [Label; 4];

struct Row {
    cells: Cells,
    h: f64,
}

struct NodeView<'g, 'a> {
    node: &'g ErGraphNode<'a>,
    style: ResolvedStyle,
    label: Label,
    w: f64,
    h: f64,
    head_h: f64,
    rows: Vec<Row>,
    /// Left edge of each column, measured from the left edge of the box.
    cols: Vec<f64>,
    row_size: f64,
    key_size: f64,
    /// Whether self-relationships loop out of the bottom side rather than
    /// the right.
    below: bool,
}

/// A relationship from an entity to itself, with the stretch of the side it
/// loops out of.
struct Loop {
    label: Label,
    band: f64,
    at: f64,
}

/// A measured label starting at `x`, with the baseline of its first line at
/// `y`.
struct TextLeft<'a, A>(&'a Label, f64, f64, A);

impl<A: fmt::Display> fmt::Display for TextLeft<'_, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let TextLeft(label, x, mut y, ref attrs) = *self;
        let _p = crate::PREFIX;
        let lines = &label.lines;
        if lines.is_empty() {
            return Ok(());
        }
        if lines.len() == 1
            && lines[0].len() == 1
            && lines[0][0].icon.is_none()
            && lines[0][0].style == 0
        {
            return write!(
                f,
                "<text{attrs} x=\"{}\" y=\"{}\">{}</text>",
                num(x),
                num(y),
                Esc(&lines[0][0].text)
            );
        }
        let shared = shared_style(label);
        let attrs = attrs.to_string();
        write!(
            f,
            "<text{} xml:space=\"preserve\">",
            text_style(&attrs, shared)
        )?;
        let top = y;
        for line in lines {
            let has_icon = line.iter().any(|span| span.icon.is_some());
            let mut sx = x;
            let mut placed = false;
            for span in line {
                if span.icon.is_none() && !span.text.is_empty() {
                    f.write_str("<tspan")?;
                    if has_icon || !placed {
                        write!(f, " x=\"{}\" y=\"{}\"", num(sx), num(y))?;
                    }
                    write!(
                        f,
                        "{}>{}</tspan>",
                        span_style(span.style & !shared),
                        Esc(&span.text)
                    )?;
                    placed = true;
                }
                sx += span.width;
            }
            y += label.line_height;
        }
        f.write_str("</text>")?;
        let mut y = top;
        for line in lines {
            let mut sx = x;
            for span in line {
                if let Some(icon) = &span.icon {
                    let side = label.size * 1.1;
                    write!(
                        f,
                        "{}",
                        icon_element(
                            &icon.name,
                            sx,
                            y - label.size * 0.35 - side / 2.0,
                            side,
                            &icon.markup
                        )
                    )?;
                }
                sx += span.width;
            }
            y += label.line_height;
        }
        Ok(())
    }
}

/// Mermaid reads `~` pairs as angle brackets. They are written as entities
/// so the label parser does not take `<int>` for a tag.
fn cell_text(raw: &str) -> String {
    let converted = parse_generic_types(raw);
    if converted == raw {
        raw.to_string()
    } else {
        converted.replace('<', "&lt;").replace('>', "&gt;")
    }
}

pub fn render_er(db: &ErModel, config: &Config, ctx: &Ctx<'_, impl Host>) -> Rendered {
    if !runs_across(Some(&db.direction)) {
        return draw(db, config, ctx, false);
    }
    turn_to_fit(ctx, |down| draw(db, config, ctx, down))
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(db: &ErModel, config: &Config, ctx: &Ctx<'_, impl Host>, turned: bool) -> Rendered {
    let p = crate::PREFIX;
    let graph = build_er_graph(db);
    let size = ctx.options.font_size;
    let small_size = js::round(size * 0.875);
    let measure = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<NodeView> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        index.insert(node.id, i);
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&ErGraphEdge> = Vec::new();
    let mut loop_list: Vec<Loop> = Vec::new();
    let mut loops: IndexMap<usize, Vec<usize>> = IndexMap::new();
    let mut loop_of: Vec<Option<usize>> = Vec::new();
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) = (index.get(edge.start), index.get(edge.end)) else {
            continue;
        };
        let label = layout_label(Some(edge.label), true, measure, small_size, 200.0, 0);
        let mut at = None;
        if src == dst {
            loop_list.push(Loop {
                label: label.clone(),
                band: 0.0,
                at: 0.0,
            });
            loops.entry(src).or_default().push(loop_list.len() - 1);
            at = Some(loop_list.len() - 1);
        }
        loop_of.push(at);
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1,
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

    // The direction of the level a node is laid out in: its nearest subgraph
    // that sets one, or the diagram's.
    let horizontal = |i: usize| -> bool {
        let mut at = i;
        // Subgraphs can name each other as members, so the walk up is
        // bounded.
        for _ in 0..graph.nodes.len() {
            let Some(up) = graph.nodes[at]
                .parent_id
                .and_then(|parent| index.get(parent).copied())
            else {
                break;
            };
            if let Some(dir) = graph.nodes[up].dir.filter(|dir| !dir.is_empty()) {
                return dir == "LR" || dir == "RL";
            }
            at = up;
        }
        db.direction == "LR" || db.direction == "RL"
    };

    for (i, node) in graph.nodes.iter().enumerate() {
        let style = if node.css_styles.is_empty() {
            resolve_style(&node.css_compiled_styles)
        } else {
            let mut all = node.css_compiled_styles.clone();
            all.extend(node.css_styles.iter().cloned());
            resolve_style(&all)
        };
        let base = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let name_size = style.font_size.unwrap_or(size);
        let row_size = style.font_size.unwrap_or(small_size);
        let key_size = style.font_size.unwrap_or(js::round(size * 0.75));
        let label = if node.is_group {
            layout_label(Some(node.label), true, measure, name_size, 4000.0, base)
        } else {
            let text = if node.alias.is_empty() {
                node.label
            } else {
                node.alias
            };
            layout_label(Some(text), true, measure, name_size, 240.0, base)
        };
        let own: &[usize] = loops.get(&i).map_or(&[], Vec::as_slice);
        // Loops leave by a side the layout keeps free of other edges: the
        // right when ranks run down, the bottom when they run across.
        let below = !own.is_empty() && !node.is_group && horizontal(i);
        let mut view = NodeView {
            node,
            style,
            label,
            w: 0.0,
            h: 0.0,
            head_h: 0.0,
            rows: Vec::new(),
            cols: Vec::new(),
            row_size,
            key_size,
            below,
        };
        let mut reach: f64 = 0.0;
        let mut bands = 0.0;
        for &k in own {
            let lp = &mut loop_list[k];
            lp.band = LOOP_BAND.max(if below {
                lp.label.width + 12.0
            } else {
                lp.label.height + 6.0
            });
            bands += lp.band;
            reach = reach.max(if below {
                lp.label.height
            } else {
                lp.label.width
            });
        }
        let min_w = MIN_WIDTH.max(if below { bands + 8.0 } else { 0.0 });
        let mut c;
        if node.is_group {
            c = CNode {
                is_group: true,
                dir: node
                    .dir
                    .filter(|dir| !dir.is_empty())
                    .map(|dir| direction(Some(dir))),
                pad_x: GROUP_PAD,
                pad_top: if view.label.height > 0.0 {
                    view.label.height + 16.0
                } else {
                    GROUP_PAD
                },
                pad_bottom: GROUP_PAD,
                min_w: (view.label.width + 2.0 * GROUP_PAD - 12.0).max(MIN_WIDTH),
                ..crate::layout::compound::cnode(0.0, 0.0, -1)
            };
        } else {
            let label = &view.label;
            let name_h = label.height.max(js::round(name_size * 1.5));
            if node.attributes.is_empty() {
                view.w = (label.width + 2.0 * BOX_PAD_X).max(min_w);
                view.h = (name_h + 2.0 * BOX_PAD_Y).max(if below { 0.0 } else { bands + 8.0 });
            } else {
                let row_line = js::round(row_size * 1.5);
                let mut widths = [0.0f64; 4];
                for attribute in node.attributes {
                    let cells: Cells = [
                        layout_label(
                            Some(&cell_text(&attribute.kind)),
                            true,
                            measure,
                            row_size,
                            NO_WRAP,
                            base,
                        ),
                        layout_label(
                            Some(&cell_text(&attribute.name)),
                            true,
                            measure,
                            row_size,
                            NO_WRAP,
                            base,
                        ),
                        layout_label(
                            Some(&attribute.keys.join(", ")),
                            false,
                            measure,
                            key_size,
                            NO_WRAP,
                            base,
                        ),
                        layout_label(
                            Some(&cell_text(&attribute.comment)),
                            true,
                            measure,
                            row_size,
                            COMMENT_WRAP,
                            base,
                        ),
                    ];
                    let mut h = row_line;
                    for k in 0..4 {
                        if cells[k].width > widths[k] {
                            widths[k] = cells[k].width;
                        }
                        if cells[k].height > h {
                            h = cells[k].height;
                        }
                    }
                    view.rows.push(Row {
                        cells,
                        h: h + ROW_PAD,
                    });
                    view.h += h + ROW_PAD;
                }
                let mut columns = 0.0;
                let mut inner = 0.0;
                for &w in &widths {
                    if w == 0.0 {
                        continue;
                    }
                    inner += w + if columns > 0.0 { CELL_GAP } else { 0.0 };
                    columns += 1.0;
                }
                view.w = (inner + 2.0 * CELL_PAD_X)
                    .max(view.label.width + 2.0 * BOX_PAD_X)
                    .max(min_w);
                // A box wider than its table shares the spare width between
                // the columns.
                let spare = if columns > 0.0 {
                    (view.w - inner - 2.0 * CELL_PAD_X) / columns
                } else {
                    0.0
                };
                let mut x = CELL_PAD_X;
                for &w in &widths {
                    view.cols.push(x);
                    if w > 0.0 {
                        x += w + spare + CELL_GAP;
                    }
                }
                view.head_h = name_h + 2.0 * HEAD_PAD_Y;
                view.h += view.head_h;
            }
            // The layout keeps a node centered in its box, so room for loops
            // is added on both sides.
            let room = if own.is_empty() {
                0.0
            } else {
                2.0 * (MARKER_LENGTH + LOOP * 0.75 + 10.0 + reach)
            };
            c = if below {
                crate::layout::compound::cnode(view.w, view.h + room, -1)
            } else {
                crate::layout::compound::cnode(view.w + room, view.h, -1)
            };
        }
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = node
            .parent_id
            .map_or(-1, |parent| index.get(parent).map_or(-1, |&at| at as i32));
    }

    let root_dir = if turned {
        direction(Some("TB"))
    } else {
        direction(Some(&db.direction))
    };
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: config.number_option("nodeSpacing", NODE_SEP),
            edge_sep: EDGE_SEP,
            rank_sep: config.number_option("rankSpacing", RANK_SEP),
            port_sep: 24.0,
            ..LayeredOptions::default()
        },
    );

    spread_ends(&mut cedges, &cnodes, &views);

    // A subgraph gets its size from the layout, so its loops hang outside it
    // and may widen the diagram.
    let mut layout_width = layout.width;
    for (&i, own) in &loops {
        let c = &cnodes[i];
        if !c.is_group {
            continue;
        }
        let mut reach: f64 = 0.0;
        for &k in own {
            reach = reach.max(loop_list[k].label.width);
        }
        layout_width =
            layout_width.max(c.x + c.w / 2.0 + MARKER_LENGTH + LOOP * 0.75 + 10.0 + reach);
    }

    let title = layout_label(
        db.title.as_deref(),
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
    let inner = layout_width.max(title.width);
    let ox = pad + (inner - layout_width) / 2.0;
    let oy = pad + title_height;
    shift_layout(&mut cnodes, &mut cedges, ox, oy);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();
    let mut links: Vec<Link> = Vec::new();

    let mut crossings: Crossings = Crossings::new();
    let any_group = cnodes.iter().any(|c| c.is_group);
    if any_group {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }

    let mut clusters = String::new();
    let mut nodes_out = String::new();
    let mut name_attrs_buf = String::new();
    let mut attrs_buf = String::new();
    for i in 0..views.len() {
        let c = &cnodes[i];
        let view = &mut views[i];
        let node = view.node;
        let classes = class_names(&RE_DEFAULT.replace(&node.css_classes, |_| String::new()));
        if c.is_group {
            view.w = c.w;
            view.h = c.h;
            let x = c.x - c.w / 2.0;
            let y = c.y - c.h / 2.0;
            let title_x = cluster_title_x(c, view.label.width, &crossings, GROUP_PAD);
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster{classes}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}/>{}</g>",
                EscText(node.id),
                num(x),
                num(y),
                num(c.w),
                num(c.h),
                view.style.shape,
                LabelSvg(
                    &view.label,
                    title_x,
                    y + 8.0 + view.label.height / 2.0,
                    rewrite(
                        &mut attrs_buf,
                        format_args!(
                            " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
                            view.style.text
                        )
                    )
                )
            );
            continue;
        }

        let w = view.w;
        let h = view.h;
        let x0 = -w / 2.0;
        let y0 = -h / 2.0;
        let text = &view.style.text;
        let shape = fragment(|f| {
            write!(
                f,
                "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"",
                num(x0),
                num(y0),
                num(w),
                num(h)
            )
        });
        let name_attrs = rewrite(
            &mut name_attrs_buf,
            format_args!(" class=\"{p}-label\"{text}"),
        );
        let name_y = if view.rows.is_empty() {
            0.0
        } else {
            y0 + view.head_h / 2.0
        };
        let name = LabelSvg(&view.label, 0.0, name_y, name_attrs);
        let note = (!view.label.lines.is_empty()
            && ctx.options.links
            && NoteLink::marked(&classes))
        .then(|| {
            let display = decode_entities(if node.alias.is_empty() {
                node.label
            } else {
                node.alias
            });
            NoteLink::new(display, node.label, &mut links, ctx)
        });
        let name = fragment(|f| match &note {
            Some(note) => write!(f, "{}", note.around(name, ctx)),
            None => write!(f, "{name}"),
        });

        let _ = write!(
            nodes_out,
            "<g class=\"{p}-node {p}-entity{classes}\" data-id=\"{}\" transform=\"translate({},{})\"><rect {shape}{BOX_ATTRS}{}/>",
            EscText(node.label),
            num(c.x),
            num(c.y),
            view.style.shape
        );
        if view.rows.is_empty() {
            let _ = write!(nodes_out, "{name}</g>");
            continue;
        }
        let row_line = js::round(view.row_size * 1.5);
        // Over a fill the diagram chose, the shading is a tint of that fill.
        // The opacity is on the group because the two parts of the last row
        // overlap.
        let opacity = if view.style.shape.contains("fill:") {
            " opacity=\"0.6\""
        } else {
            ""
        };
        let _ = write!(
            nodes_out,
            "<g class=\"{p}-er-stripes\" fill=\"var(--_bg)\"{opacity}>"
        );
        let mut y = y0 + view.head_h;
        for (r, row) in view.rows.iter().enumerate() {
            if r % 2 == 0 {
                let span = fragment(|f| write!(f, "x=\"{}\" width=\"{}\"", num(x0), num(w)));
                let _ = if r == view.rows.len() - 1 {
                    // The last row follows the box's rounded bottom corners:
                    // a rounded rect, squared off on top.
                    write!(
                        nodes_out,
                        "<rect {span} y=\"{}\" height=\"{}\" rx=\"{RADIUS}\"/><rect {span} y=\"{}\" height=\"{}\"/>",
                        num(y),
                        num(row.h),
                        num(y),
                        num(row.h / 2.0)
                    )
                } else {
                    write!(
                        nodes_out,
                        "<rect {span} y=\"{}\" height=\"{}\"/>",
                        num(y),
                        num(row.h)
                    )
                };
            }
            y += row.h;
        }
        let line_color = match &view.style.stroke {
            Some(stroke) if !stroke.is_empty() => Esc(stroke),
            _ => Esc("var(--_b)"),
        };
        let _ = write!(
            nodes_out,
            "</g><path class=\"{p}-er-divider\" d=\"M{},{}H{}\" stroke=\"{line_color}\"/><rect {shape} fill=\"none\" stroke=\"var(--_b)\"{}/>{name}<g class=\"{p}-er-attributes\" font-size=\"{}\">",
            num(x0),
            num(y0 + view.head_h),
            num(x0 + w),
            view.style.line,
            js::Number(view.row_size)
        );
        let mut y = y0 + view.head_h;
        for row in &view.rows {
            let baseline = y + ROW_PAD / 2.0 + row_line / 2.0 + view.row_size * 0.35;
            let [kind, attribute, keys, comment] = &row.cells;
            let _ = write!(
                nodes_out,
                "{}{}{}{}",
                TextLeft(
                    kind,
                    x0 + view.cols[0],
                    baseline,
                    format_args!(" class=\"{p}-er-type\" fill=\"var(--_m)\"{text}"),
                ),
                TextLeft(
                    attribute,
                    x0 + view.cols[1],
                    baseline,
                    format_args!(" class=\"{p}-er-name\"{text}"),
                ),
                TextLeft(
                    keys,
                    x0 + view.cols[2],
                    baseline,
                    format_args!(
                        " class=\"{p}-er-keys\" font-size=\"{}\" fill=\"var(--_m)\"{text}",
                        js::Number(view.key_size)
                    ),
                ),
                TextLeft(
                    comment,
                    x0 + view.cols[3],
                    baseline,
                    format_args!(" class=\"{p}-er-comment\" fill=\"var(--_m)\"{text}"),
                ),
            );
            y += row.h;
        }
        let _ = write!(nodes_out, "</g></g>");
    }

    // Self-relationships share the side they loop out of, squeezed together
    // if it is too short for them.
    for (&i, own) in &loops {
        let view = &views[i];
        let mut total = 0.0;
        for &k in own {
            total += loop_list[k].band;
        }
        let side = if view.below { view.w } else { view.h };
        let scale = ((side - 8.0).max(0.0) / total).min(1.0);
        let mut at = (-total * scale) / 2.0;
        for &k in own {
            let lp = &mut loop_list[k];
            lp.band *= scale;
            lp.at = at + lp.band / 2.0;
            at += lp.band;
        }
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut d = String::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        d.clear();
        let start;
        let end;
        if let Some(k) = loop_of[i] {
            let lp = &loop_list[k];
            let ce = &cedges[i];
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let spread = (lp.band / 2.0 - 6.0).clamp(1.0, 13.0);
            let far = MARKER_LENGTH + LOOP * 0.75 + 6.0;
            let (label_x, label_y);
            if view.below {
                let x = c.x + lp.at;
                let y = c.y + view.h / 2.0;
                let turn = num(y + MARKER_LENGTH);
                let out = num(y + MARKER_LENGTH + LOOP);
                let _ = write!(
                    d,
                    "M{},{}V{turn}C{},{out} {},{out} {},{turn}V{}",
                    num(x - spread),
                    num(y),
                    num(x - spread),
                    num(x + spread),
                    num(x + spread),
                    num(y)
                );
                start = er_marker(edge.arrow_type_start, x - spread, y, 0.0, -1.0);
                end = er_marker(edge.arrow_type_end, x + spread, y, 0.0, -1.0);
                label_x = x;
                label_y = y + far + label.height / 2.0;
            } else {
                let x = c.x + view.w / 2.0;
                let y = c.y + lp.at;
                let turn = num(x + MARKER_LENGTH);
                let out = num(x + MARKER_LENGTH + LOOP);
                let _ = write!(
                    d,
                    "M{},{}H{turn}C{out},{} {out},{} {turn},{}H{}",
                    num(x),
                    num(y - spread),
                    num(y - spread),
                    num(y + spread),
                    num(y + spread),
                    num(x)
                );
                start = er_marker(edge.arrow_type_start, x, y - spread, -1.0, 0.0);
                end = er_marker(edge.arrow_type_end, x, y + spread, -1.0, 0.0);
                label_x = x + far + label.width / 2.0;
                label_y = y;
            }
            cedges[i].label_x = label_x;
            cedges[i].label_y = label_y;
        } else {
            let ce = &cedges[i];
            let mut route = straighten(&ce.route);
            if route.len() < 6 {
                continue;
            }
            inset(&mut route, 0, &views[ce.src], &cnodes[ce.src]);
            let last = route.len() - 3;
            inset(&mut route, last, &views[ce.dst], &cnodes[ce.dst]);
            // A curve stops short of each box so that the marker sits on a
            // straight run of line.
            let dx = (route[route.len() - 3] - route[0]).abs();
            let dy = (route[route.len() - 2] - route[1]).abs();
            let direct = route.len() == 6
                && (dx < 0.01 || dy < 0.01 || js::hypot(dx, dy) <= 3.0 * MARKER_LENGTH);
            let stub = if direct { 0.0 } else { MARKER_LENGTH };
            let path = route_path(&route, None, stub, stub, None, None);
            let _ = if stub > 0.0 {
                write!(
                    d,
                    "M{}L{}L{}",
                    Point(path.sx, path.sy),
                    &path.d[1..],
                    Point(path.ex, path.ey)
                )
            } else {
                write!(d, "{}", path.d)
            };
            start = er_marker(edge.arrow_type_start, path.sx, path.sy, path.sdx, path.sdy);
            end = er_marker(edge.arrow_type_end, path.ex, path.ey, path.edx, path.edy);
        }
        let dashed = edge.dashed;
        let _ = write!(
            edges_out,
            "<g class=\"{p}-edge {p}-er-{}\" data-id=\"{}\"><path d=\"{d}\"{}/>{start}{end}</g>",
            if dashed {
                "non-identifying"
            } else {
                "identifying"
            },
            EscText(&edge.id),
            if dashed {
                " stroke-dasharray=\"4 3\""
            } else {
                ""
            }
        );
        if label.width > 0.0 {
            let _ = write!(
                labels_out,
                "{}",
                edge_label_svg(&edge.id, label, cedges[i].label_x, cedges[i].label_y, "",)
            );
        }
    }

    let title_attrs = crate::svg::builder::TITLE;
    let mut content = String::with_capacity(
        clusters.len() + edges_out.len() + labels_out.len() + nodes_out.len() + 512,
    );
    let _ = write!(
        content,
        "{}",
        LabelSvg(&title, width / 2.0, pad + title.height / 2.0, title_attrs)
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
            js::Number(small_size)
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");

    let svg = svg_document(
        "er",
        width,
        height,
        size,
        ctx,
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

fn straighten(route: &[f64]) -> Vec<f64> {
    let mut out = route[..3.min(route.len())].to_vec();
    let mut k = 3;
    while k < route.len() {
        let prev = out.len() - 3;
        let flat = |axis: usize| {
            (out[prev + axis] - route[k + axis]).abs() < 0.01
                && (route[k + axis] - route[k + 3 + axis]).abs() < 0.01
        };
        if k + 3 < route.len() && (flat(0) || flat(1)) {
            k += 3;
            continue;
        }
        out.extend_from_slice(&route[k..k + 3]);
        k += 3;
    }
    out
}

struct End {
    edge: usize,
    at: usize,
    toward: f64,
}

/// The layout brings every edge that ends on one side of a box to the middle
/// of that side, where their markers would cover each other. This spaces
/// them along the side, in the order they fan out.
fn spread_ends(cedges: &mut [CEdge], cnodes: &[CNode], views: &[NodeView]) {
    let mut sides: IndexMap<usize, Vec<End>> = IndexMap::new();
    let mut add = |node: usize, edge: usize, route: &[f64], at: usize, next: usize| {
        let c = &cnodes[node];
        let dx = route[at] - c.x;
        let dy = route[at + 1] - c.y;
        let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
            if dy < 0.0 { 0 } else { 2 }
        } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
            if dx < 0.0 { 3 } else { 1 }
        } else {
            return;
        };
        let end = End {
            edge,
            at,
            toward: route[next + if side & 1 == 1 { 1 } else { 0 }],
        };
        sides.entry(node * 4 + side).or_default().push(end);
    };
    for (i, e) in cedges.iter().enumerate() {
        if e.route.len() < 3 {
            continue;
        }
        let last = e.route.len() - 3;
        if e.src == e.dst || last < 3 {
            continue;
        }
        add(e.src, i, &e.route, 0, 3);
        add(e.dst, i, &e.route, last, last - 3);
    }
    for (&key, ends) in sides.iter_mut() {
        if ends.len() < 2 {
            continue;
        }
        let node = key >> 2;
        let vertical = key & 1 == 1;
        let (bw, bh) = if cnodes[node].is_group {
            (cnodes[node].w, cnodes[node].h)
        } else {
            (views[node].w, views[node].h)
        };
        let length = if vertical { bh } else { bw };
        let step = PORT_GAP
            .min((length - 2.0 * PORT_MARGIN) / (ends.len() - 1) as f64)
            .max(0.0);
        let center = if vertical {
            cnodes[node].y
        } else {
            cnodes[node].x
        };
        ends.sort_by(|a, b| js::compare(a.toward - b.toward));
        let count = ends.len() as f64;
        for (i, end) in ends.iter().enumerate() {
            cedges[end.edge].route[end.at + if vertical { 1 } else { 0 }] =
                center + (i as f64 - (count - 1.0) / 2.0) * step;
        }
    }
}

/// An entity with a self-relationship is laid out larger than it is drawn;
/// this moves a route end from the side of that larger box onto the
/// entity's own side.
fn inset(route: &mut [f64], at: usize, view: &NodeView, c: &CNode) {
    if c.is_group {
        return;
    }
    let dx = route[at] - c.x;
    let dy = route[at + 1] - c.y;
    if c.w != view.w && (dx.abs() - c.w / 2.0).abs() < 0.5 {
        route[at] -= (js::sign(dx) * (c.w - view.w)) / 2.0;
    }
    if c.h != view.h && (dy.abs() - c.h / 2.0).abs() < 0.5 {
        route[at + 1] -= (js::sign(dy) * (c.h - view.h)) / 2.0;
    }
}
