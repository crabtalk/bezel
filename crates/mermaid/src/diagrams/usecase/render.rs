//! Pele's `diagrams/usecase/render.ts`.

use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use crate::config::positive;
use crate::svg::builder::rewrite;
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Write as _},
};

use indexmap::IndexMap;

use super::{
    graph::{UsecaseGraphEdge, UsecaseGraphNode},
    types::{ActorType, BoundaryType, LabelType, UsecaseModel},
};
use crate::Ctx;
use crate::diagrams::common::icon::icon_svg;
use crate::{
    Rendered,
    diagrams::common::fit_width::{runs_across, turn_to_fit},
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, Dir, cnode, compound_layout, direction, shift_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{EscText, LabelSvg, LeftLabel, Point, fragment, num},
        cluster::{Crossings, mark_crossings},
        edges::{EdgePath, edge_label_svg, marker, marker_trim, route_path},
        root::{Accessible, svg_document},
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
    },
    text::{Host, Label, Style, layout_label},
};

const NODE_SEP: f64 = 32.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 72.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const FIGURE_W: f64 = 40.0;
const FIGURE_H: f64 = 60.0;
const FIGURE_GAP: f64 = 6.0;
const TAB_PAD: f64 = 6.0;
const CELL_PAD: f64 = 10.0;
const ROW_PAD: f64 = 8.0;
const FOLD: f64 = 9.0;
const EXTENSION: f64 = 12.0;
const PORT_GAP: f64 = 14.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";

static RE_MARKUP: Lazy = Lazy::new(r"[&<>\\]", "g");
static RE_DEFAULT: Lazy = Lazy::new(r"^default\s?", "");
static RE_EDGE_CLASSES: Lazy = Lazy::new(r"^default relationship relationship-\S+\s?", "");

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Actor,
    Ellipse,
    Box,
    Json,
    Boundary,
}

struct View<'g> {
    node: &'g UsecaseGraphNode,
    kind: ViewKind,
    style: ResolvedStyle,
    label: Label,
    stereotype: Label,
    /// The size of what is drawn; the layout box is larger when the node
    /// carries a self-relationship.
    w: f64,
    h: f64,
    rows: Vec<(Label, Label)>,
    key_w: f64,
    head_h: f64,
    loops: usize,
}

/// Mermaid's `usecase` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UsecaseConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub node_spacing: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub rank_spacing: Option<f64>,
}

/// A plain label is shown as written: no tags, no markdown, no `\n` breaks.
/// Its markup characters are handed to the label parser as character
/// references, which it turns back into characters.
fn literal(text: &str) -> String {
    RE_MARKUP.replace(text, |caps| {
        format!("&#{};", caps.str(0).chars().next().map_or(0, u32::from))
    })
}

fn text(
    raw: Option<&str>,
    markdown: bool,
    measure: &impl Host,
    size: f64,
    wrap: f64,
    base: u8,
) -> Label {
    match raw {
        Some(raw) if !raw.is_empty() && !markdown => {
            layout_label(Some(&literal(raw)), markdown, measure, size, wrap, base)
        }
        _ => layout_label(raw, markdown, measure, size, wrap, base),
    }
}

/// The point of a node's outline on one of its four sides (0 top, 1 right,
/// 2 bottom, 3 left), `along` away from the middle of that side. Lines meet
/// an actor at its figure, not at its label.
fn anchor(view: &View, c: &CNode, side: usize, along: f64) -> (f64, f64) {
    let sign = if side == 1 || side == 2 { 1.0 } else { -1.0 };
    let upright = side == 1 || side == 3;
    let mut reach = (if upright { view.w } else { view.h }) / 2.0;
    let mut middle = 0.0;
    if view.kind == ViewKind::Actor {
        if upright {
            reach = FIGURE_W / 2.0 + 3.0;
            middle = (FIGURE_H - view.h) / 2.0;
        } else {
            reach += 2.0;
        }
    } else if view.kind == ViewKind::Ellipse {
        let span = (if upright { view.h } else { view.w }) / 2.0;
        reach *= (1.0 - (along / span).powi(2)).max(0.0).sqrt();
    }
    if upright {
        (c.x + sign * reach, c.y + middle + along)
    } else {
        (c.x + along, c.y + sign * reach)
    }
}

/// How much of a side edges may spread over.
fn side_length(view: &View, side: usize) -> f64 {
    let upright = side == 1 || side == 3;
    if view.kind == ViewKind::Actor {
        return if upright {
            FIGURE_H * 0.5
        } else {
            FIGURE_W * 0.6
        };
    }
    let length = if upright { view.h } else { view.w };
    if view.kind == ViewKind::Ellipse {
        length * 0.6
    } else {
        length - 16.0
    }
}

struct End {
    edge: usize,
    at: usize,
    toward: f64,
}

/// The layout brings every edge that ends on one side of a node to the
/// middle of that side. This spaces them along the side in the order they
/// fan out, and puts each on the node's outline.
fn attach_ends(cedges: &mut [CEdge], cnodes: &[CNode], views: &[View]) {
    let mut sides: IndexMap<usize, Vec<End>> = IndexMap::new();
    let mut add = |index: usize, edge: usize, route: &[f64], at: usize, next: usize| {
        let c = &cnodes[index];
        if c.is_group {
            return;
        }
        let dx = route[at] - c.x;
        let dy = route[at + 1] - c.y;
        let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
            if dy < 0.0 { 0 } else { 2 }
        } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
            if dx < 0.0 { 3 } else { 1 }
        } else {
            return;
        };
        sides.entry(index * 4 + side).or_default().push(End {
            edge,
            at,
            toward: route[next + (side & 1)],
        });
    };
    for (i, e) in cedges.iter().enumerate() {
        if e.src == e.dst || e.route.len() < 6 {
            continue;
        }
        let last = e.route.len() - 3;
        add(e.src, i, &e.route, 0, 3);
        add(e.dst, i, &e.route, last, last - 3);
    }
    for (key, mut ends) in sides {
        let index = key >> 2;
        let side = key & 3;
        let view = &views[index];
        let step = if ends.len() > 1 {
            PORT_GAP.min(side_length(view, side) / (ends.len() - 1) as f64)
        } else {
            0.0
        };
        ends.sort_by(|a, b| js::compare(a.toward - b.toward));
        let count = ends.len() as f64;
        for (i, end) in ends.iter().enumerate() {
            let (x, y) = anchor(
                view,
                &cnodes[index],
                side,
                (i as f64 - (count - 1.0) / 2.0) * step,
            );
            let route = &mut cedges[end.edge].route;
            route[end.at] = x;
            route[end.at + 1] = y;
        }
    }
}

/// The hollow triangle of a generalization, with its tip at (x, y).
fn extension(
    f: &mut fmt::Formatter<'_>,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    color: &str,
) -> fmt::Result {
    let p = crate::PREFIX;
    let bx = x - dx * EXTENSION;
    let by = y - dy * EXTENSION;
    let half = EXTENSION * 0.5;
    write!(
        f,
        "<path class=\"{p}-marker\" d=\"M{}L{}L{}Z\" fill=\"var(--_bg)\" stroke=\"{color}\" stroke-linejoin=\"round\"/>",
        Point(x, y),
        Point(bx - dy * half, by + dx * half),
        Point(bx + dy * half, by - dx * half)
    )
}

fn trim(kind: &str) -> f64 {
    if kind == "extension" {
        EXTENSION
    } else {
        marker_trim(kind)
    }
}

fn mark<'a>(
    kind: &'a str,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    color: &'a str,
) -> impl fmt::Display + 'a {
    fragment(move |f| {
        if kind == "extension" {
            extension(f, x, y, dx, dy, color)
        } else {
            write!(f, "{}", marker(kind, x, y, dx, dy, color))
        }
    })
}

/// Appends an actor's figure to `out`.
fn figure(out: &mut String, view: &View, host: &impl Host) {
    let p = crate::PREFIX;
    let node = view.node;
    let shape = &view.style.shape;
    let line = fragment(|f| write!(f, " fill=\"none\" stroke=\"var(--_b)\"{}", view.style.line));
    let mut head = -21.0;
    let mut radius = 9.0;
    let _ = match node.actor_type {
        Some(ActorType::Hollow) => {
            radius = 8.0;
            write!(
                out,
                "<circle cy=\"-21\" r=\"8\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{shape}/><path d=\"M-20,-9H20V0H5.5L20,15.5L12,25.5L0,12L-12,25.5L-20,15.5L-5.5,0H-20Z\" fill=\"var(--_bg)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{shape}/>"
            )
        }
        Some(ActorType::Awesome) => write!(
            out,
            "<path d=\"M0,-30A11,11 0 1 1 0,-8A11,11 0 1 1 0,-30ZM-20,25C-20,9 -12,-3 0,-3C12,-3 20,9 20,25C20,28 18,30 15,30H-15C-18,30 -20,28 -20,25Z\" fill=\"var(--_b)\"{shape}/>"
        ),
        Some(ActorType::Icon) => {
            head = -14.0;
            radius = 12.0;
            let icon = node.icon.as_deref().unwrap_or("");
            write!(
                out,
                "<rect x=\"-22\" y=\"-26\" width=\"44\" height=\"44\" rx=\"{RADIUS}\"{SHAPE_ATTRS}{shape}/>{}",
                icon_svg(host, icon, -14.0, -18.0, 28.0)
            )
        }
        _ => write!(
            out,
            "<circle cy=\"-21\" r=\"9\"{SHAPE_ATTRS}{shape}/><path d=\"M0,-12V10M-14,-3H14M0,10L-12,30M0,10L12,30\"{line}/>"
        ),
    };
    if node.business == Some(true) {
        // The slash of a business actor: a chord across the lower right of
        // the head.
        let _ = write!(
            out,
            "<path class=\"{p}-business\" d=\"M{}L{}\"{line}/>",
            Point(0.12 * radius, head + 0.99 * radius),
            Point(0.92 * radius, head - 0.39 * radius)
        );
    }
}

pub fn render_usecase(
    model: &UsecaseModel,
    config: &UsecaseConfig,
    ctx: &Ctx<'_, impl Host>,
) -> Rendered {
    if !runs_across(Some(model.direction.name())) {
        return draw(model, config, ctx, false);
    }
    turn_to_fit(ctx, |down| draw(model, config, ctx, down))
}

/// A boundary's title sits in the middle of its top edge. Where edges enter
/// through that edge it moves into the stretch between them that is nearest
/// the middle, or the widest if none has room.
fn title_x(c: &CNode, width: f64, crossings: &Crossings) -> f64 {
    let xs = crossings.get(&(js::round(c.y - c.h / 2.0) as i64));
    let Some(xs) = xs.filter(|_| width != 0.0) else {
        return c.x;
    };
    let left = c.x - c.w / 2.0 + 6.0;
    let right = c.x + c.w / 2.0 - 6.0;
    let mut cuts: Vec<f64> = xs
        .iter()
        .copied()
        .filter(|&x| x > left && x < right)
        .collect();
    cuts.sort_by(|a, b| js::compare(a - b));
    cuts.push(right);
    let need = width / 2.0 + 6.0;
    let mut best = c.x;
    let mut best_off = f64::INFINITY;
    let mut widest = 0.0;
    let mut from = left;
    for cut in cuts {
        if cut - from >= 2.0 * need {
            let x = c.x.max(from + need).min(cut - need);
            if (x - c.x).abs() < best_off {
                best_off = (x - c.x).abs();
                best = x;
            }
        } else if best_off == f64::INFINITY && cut - from > widest {
            widest = cut - from;
            best = (from + cut) / 2.0;
        }
        from = cut;
    }
    best
}

/// `turned` draws a diagram that runs across as one that runs down.
fn draw(
    model: &UsecaseModel,
    config: &UsecaseConfig,
    ctx: &Ctx<'_, impl Host>,
    turned: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let graph = model.graph();
    let size = ctx.options.font_size;
    let small = js::round(size * 0.875);
    let tiny = js::round(size * 0.8125);
    let measure = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);
    let root_dir = if turned {
        Dir::TB
    } else {
        direction(Some(model.direction.name()))
    };
    let sideways = root_dir == Dir::LR || root_dir == Dir::RL;

    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut views: Vec<View> = Vec::new();
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
        let base = (if style.bold { Style::BOLD } else { 0 })
            | (if style.italic { Style::ITALIC } else { 0 });
        let font_size = style.font_size.unwrap_or(size);
        let markdown = node.label_type == LabelType::Markdown;
        let stereotype = text(
            node.stereotype
                .as_deref()
                .map(|s| format!("«{s}»"))
                .as_deref(),
            false,
            measure,
            tiny,
            200.0,
            base,
        );
        let mut view = View {
            node,
            kind: ViewKind::Box,
            style,
            label: stereotype.clone(),
            stereotype,
            w: 0.0,
            h: 0.0,
            rows: Vec::new(),
            key_w: 0.0,
            head_h: 0.0,
            loops: 0,
        };
        let mut c: CNode;
        if node.is_group {
            view.kind = ViewKind::Boundary;
            view.label = text(
                Some(&node.label),
                markdown,
                measure,
                font_size,
                4000.0,
                base,
            );
            let title_h = view.label.height;
            c = cnode(0.0, 0.0, -1);
            c.is_group = true;
            c.pad_x = GROUP_PAD;
            c.pad_bottom = GROUP_PAD;
            c.pad_top = if node.boundary_type == Some(BoundaryType::Package) {
                title_h + 2.0 * TAB_PAD + 16.0
            } else if title_h > 0.0 {
                title_h + 16.0
            } else {
                GROUP_PAD
            };
            c.min_w = view.label.width + 2.0 * GROUP_PAD;
        } else {
            if let Some(rows) = &node.json_rows {
                view.kind = ViewKind::Json;
                view.label = text(
                    Some(&node.label),
                    false,
                    measure,
                    font_size,
                    4000.0,
                    base | Style::BOLD,
                );
                let mut value_w: f64 = 0.0;
                for row in rows {
                    let key = text(Some(&row.key), false, measure, small, 4000.0, base);
                    let value = text(Some(&row.value), false, measure, small, 260.0, base);
                    view.key_w = view.key_w.max(key.width);
                    value_w = value_w.max(value.width);
                    view.h += value.height.max(js::round(small * 1.5)) + ROW_PAD;
                    view.rows.push((key, value));
                }
                // The root of an empty object has a value and no key, and
                // then there is no key column.
                if view.key_w > 0.0 {
                    view.key_w += 2.0 * CELL_PAD;
                }
                view.head_h = view.label.height.max(js::round(font_size * 1.5)) + ROW_PAD;
                view.w = (view.label.width + 2.0 * CELL_PAD)
                    .max(if view.rows.is_empty() {
                        0.0
                    } else {
                        view.key_w + value_w + 2.0 * CELL_PAD
                    })
                    .max(72.0);
                view.h += view.head_h;
            } else if node.actor_type.is_some() {
                view.kind = ViewKind::Actor;
                view.label = text(Some(&node.label), markdown, measure, font_size, 140.0, base);
                view.w = FIGURE_W.max(view.label.width).max(view.stereotype.width);
                view.h = FIGURE_H + FIGURE_GAP + view.stereotype.height + view.label.height;
            } else {
                view.label = text(
                    Some(&node.label),
                    markdown,
                    measure,
                    if node.shape == "note" {
                        small
                    } else {
                        font_size
                    },
                    180.0,
                    base,
                );
                let tw = view.label.width.max(view.stereotype.width);
                let th = view.label.height + view.stereotype.height;
                let business = node.business == Some(true);
                if node.shape == "usecaseEllipse" || node.shape == "usecaseBusiness" {
                    view.kind = ViewKind::Ellipse;
                    // An oval needs more room around its text than a box, and
                    // a business oval room for its slash.
                    view.w =
                        (tw + 2.0 * node.padding + if business { 16.0 } else { 4.0 }).max(88.0);
                    view.h = (th + 28.0).max(48.0);
                } else if node.shape == "note" {
                    view.w = tw + 2.0 * node.padding + 6.0;
                    view.h = th + 18.0;
                } else {
                    view.w = (tw + 2.0 * node.padding + 12.0).max(80.0);
                    view.h = (th + 24.0).max(44.0);
                }
            }
            c = cnode(view.w, view.h, -1);
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

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn: Vec<&UsecaseGraphEdge> = Vec::new();
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) =
            (index.get(edge.start.as_str()), index.get(edge.end.as_str()))
        else {
            continue;
        };
        let semantic = matches!(edge.relationship_type, "include" | "extend");
        let raw = if semantic {
            Some(format!(
                "«{}»",
                edge.label.as_deref().unwrap_or("undefined")
            ))
        } else {
            edge.label.clone()
        };
        let label = text(
            raw.as_deref(),
            edge.label_type == Some(LabelType::Markdown),
            measure,
            small,
            200.0,
            0,
        );
        if src == dst {
            // A self-relationship loops out across the flow, and the node's
            // box grows to hold it.
            views[src].loops += 1;
            if sideways {
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
        edge_labels.push(label);
        let label = &edge_labels[edge_labels.len() - 1];
        cedges.push(CEdge {
            src,
            dst,
            minlen: edge.minlen as i32,
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
    shift_layout(&mut cnodes, &mut cedges, ox, oy);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();

    // Where edges cross each boundary's top border, so that a title can keep
    // out of their way.
    let mut crossings: Crossings = HashMap::new();
    for e in &cedges {
        mark_crossings(&mut crossings, &e.route);
    }
    attach_ends(&mut cedges, &cnodes, &views);

    let mut clusters = String::new();
    let mut nodes_out = String::new();
    let mut stereotype_attrs_buf = String::new();
    let mut label_attrs_buf = String::new();
    let mut key_attrs_buf = String::new();
    let mut value_attrs_buf = String::new();
    let mut title_attrs_buf = String::new();
    for i in 0..views.len() {
        let view = &views[i];
        let c = &cnodes[i];
        let node = view.node;
        let classes = class_names(&RE_DEFAULT.replace(&node.css_classes, |_| String::new()));
        let id = EscText(&node.id);
        let text_attrs = &view.style.text;
        let shape = &view.style.shape;
        let w = view.w;
        let h = view.h;

        if view.kind == ViewKind::Boundary {
            let x = c.x - c.w / 2.0;
            let y = c.y - c.h / 2.0;
            let boxed = fragment(|f| {
                write!(
                    f,
                    " fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{shape}"
                )
            });
            let title_attrs = rewrite(
                &mut title_attrs_buf,
                format_args!(" class=\"{p}-cluster-label\" fill=\"var(--_m)\"{text_attrs}"),
            );
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster {p}-boundary{classes}\" data-id=\"{id}\">"
            );
            let _ = if node.boundary_type == Some(BoundaryType::Package) {
                let tab_w = (view.label.width + 2.0 * TAB_PAD + 8.0).min(c.w);
                let tab_h = view.label.height + 2.0 * TAB_PAD;
                write!(
                    clusters,
                    "<rect class=\"{p}-boundary-tab\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{boxed}/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{boxed}/>{}</g>",
                    num(x),
                    num(y),
                    num(tab_w),
                    num(tab_h),
                    num(x),
                    num(y + tab_h),
                    num(c.w),
                    num(c.h - tab_h),
                    LabelSvg(&view.label, x + tab_w / 2.0, y + tab_h / 2.0, title_attrs)
                )
            } else {
                write!(
                    clusters,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{boxed}/>{}</g>",
                    num(x),
                    num(y),
                    num(c.w),
                    num(c.h),
                    LabelSvg(
                        &view.label,
                        title_x(c, view.label.width, &crossings),
                        y + 8.0 + view.label.height / 2.0,
                        title_attrs
                    )
                )
            };
            continue;
        }

        let kind = match view.kind {
            ViewKind::Actor => "actor",
            ViewKind::Json => "json",
            _ if view.kind != ViewKind::Ellipse && node.shape == "note" => "note",
            _ => "usecase",
        };
        let _ = write!(
            nodes_out,
            "<g class=\"{p}-node {p}-{kind}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">",
            num(c.x),
            num(c.y)
        );
        if view.kind == ViewKind::Actor {
            let top = -h / 2.0;
            let stereotype_y = top + FIGURE_H + FIGURE_GAP + view.stereotype.height / 2.0;
            let _ = write!(
                nodes_out,
                "<g class=\"{p}-actor-figure\" transform=\"translate(0,{})\">",
                num(top + FIGURE_H / 2.0)
            );
            figure(&mut nodes_out, view, ctx.host);
            let stereotype_attrs = rewrite(
                &mut stereotype_attrs_buf,
                format_args!(
                    " class=\"{p}-stereotype\" font-size=\"{}\" fill=\"var(--_m)\"",
                    js::Number(tiny)
                ),
            );
            let label_attrs = rewrite(
                &mut label_attrs_buf,
                format_args!(" class=\"{p}-label\"{text_attrs}"),
            );
            let _ = write!(
                nodes_out,
                "</g>{}{}",
                LabelSvg(&view.stereotype, 0.0, stereotype_y, stereotype_attrs),
                LabelSvg(
                    &view.label,
                    0.0,
                    h / 2.0 - view.label.height / 2.0,
                    label_attrs
                )
            );
        } else if view.kind == ViewKind::Json {
            let x0 = -w / 2.0;
            let head_y = -h / 2.0 + view.head_h;
            let _ = write!(
                nodes_out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE_ATTRS}{shape}/><path class=\"{p}-json-lines\" d=\"M{}H{}",
                num(x0),
                num(-h / 2.0),
                num(w),
                num(h),
                Point(x0, head_y),
                num(x0 + w)
            );
            if view.key_w > 0.0 {
                let _ = write!(
                    nodes_out,
                    "M{}V{}",
                    Point(x0 + view.key_w, head_y),
                    num(h / 2.0)
                );
            }
            let title_attrs = rewrite(
                &mut title_attrs_buf,
                format_args!(" class=\"{p}-label\" font-weight=\"var(--_hw)\"{text_attrs}"),
            );
            let _ = write!(
                nodes_out,
                "\" fill=\"none\" stroke=\"var(--_b)\"{}/>{}<g font-size=\"{}\">",
                view.style.line,
                LabelSvg(&view.label, 0.0, -h / 2.0 + view.head_h / 2.0, title_attrs),
                js::Number(small)
            );
            let key_attrs = rewrite(
                &mut key_attrs_buf,
                format_args!(" class=\"{p}-json-key\" fill=\"var(--_m)\""),
            );
            let value_attrs = rewrite(
                &mut value_attrs_buf,
                format_args!(" class=\"{p}-json-value\"{text_attrs}"),
            );
            let mut y = head_y;
            for (key, value) in &view.rows {
                let row_h = value.height.max(js::round(small * 1.5)) + ROW_PAD;
                let _ = write!(
                    nodes_out,
                    "{}{}",
                    LabelSvg(
                        key,
                        x0 + CELL_PAD + key.width / 2.0,
                        y + row_h / 2.0,
                        key_attrs
                    ),
                    LeftLabel(
                        value,
                        x0 + view.key_w + CELL_PAD,
                        y + row_h / 2.0,
                        value_attrs
                    )
                );
                y += row_h;
            }
            nodes_out.push_str("</g>");
        } else {
            let label_y = view.stereotype.height / 2.0;
            if view.kind == ViewKind::Ellipse {
                let _ = write!(
                    nodes_out,
                    "<ellipse rx=\"{}\" ry=\"{}\"{SHAPE_ATTRS}{shape}/>",
                    num(w / 2.0),
                    num(h / 2.0)
                );
                if node.business == Some(true) {
                    // The slash of a business use case, cutting off the right
                    // end of the oval beside the text.
                    let from = w / 2.0 - 15.0;
                    let to = w / 2.0 - 5.0;
                    let arc = |at: f64| (h / 2.0) * (1.0 - (at / (w / 2.0)).powi(2)).sqrt();
                    let _ = write!(
                        nodes_out,
                        "<path class=\"{p}-business\" d=\"M{}L{}\" fill=\"none\" stroke=\"var(--_b)\"{}/>",
                        Point(from, arc(from)),
                        Point(to, -arc(to)),
                        view.style.line
                    );
                }
            } else if node.shape == "note" {
                let r = w / 2.0;
                let b = h / 2.0;
                let _ = write!(
                    nodes_out,
                    "<path d=\"M{}H{}L{}V{}H{}Z\" fill=\"var(--_a)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{shape}/><path d=\"M{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{}/>",
                    Point(-r, -b),
                    num(r - FOLD),
                    Point(r, FOLD - b),
                    num(b),
                    num(-r),
                    Point(r - FOLD, -b),
                    num(FOLD - b),
                    num(r),
                    view.style.line
                );
            } else {
                let _ = write!(
                    nodes_out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE_ATTRS}{shape}/>",
                    num(-w / 2.0),
                    num(-h / 2.0),
                    num(w),
                    num(h)
                );
            }
            let stereotype_attrs = rewrite(
                &mut stereotype_attrs_buf,
                format_args!(
                    " class=\"{p}-stereotype\" font-size=\"{}\" fill=\"var(--_m)\"",
                    js::Number(tiny)
                ),
            );
            let label_attrs = rewrite(
                &mut label_attrs_buf,
                format_args!(
                    " class=\"{p}-label\"{}{text_attrs}",
                    fragment(|f| if node.shape == "note" {
                        write!(f, " font-size=\"{}\"", js::Number(small))
                    } else {
                        Ok(())
                    })
                ),
            );
            let _ = write!(
                nodes_out,
                "{}{}",
                LabelSvg(
                    &view.stereotype,
                    0.0,
                    -view.label.height / 2.0,
                    stereotype_attrs
                ),
                LabelSvg(&view.label, 0.0, label_y, label_attrs)
            );
        }
        nodes_out.push_str("</g>");
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let style = if !edge.style.is_empty() || !edge.css_compiled_styles.is_empty() {
            let mut all = edge.css_compiled_styles.clone();
            all.extend(edge.style.iter().cloned());
            Some(resolve_style(&all))
        } else {
            None
        };
        let start_type = edge.arrow_type_start;
        let end_type = edge.arrow_type_end;
        let path: EdgePath;
        if cedges[i].src == cedges[i].dst {
            let src = cedges[i].src;
            let c = &cnodes[src];
            let view = &views[src];
            let k = loop_seen.get(&src).copied().unwrap_or(0);
            loop_seen.insert(src, k + 1);
            let k = k as f64;
            let side = if sideways { 2 } else { 1 };
            let spread = (side_length(view, side) / 2.0).min(8.0 + k * 6.0);
            let reach = LOOP + k * 8.0;
            let (ax, ay) = anchor(view, c, side, -spread);
            let (bx, by) = anchor(view, c, side, spread);
            let len = match js::hypot(reach, spread) {
                len if len == 0.0 || len.is_nan() => 1.0,
                len => len,
            };
            let ux = if sideways { spread / len } else { reach / len };
            let uy = if sideways { reach / len } else { spread / len };
            let st = trim(start_type);
            let et = trim(end_type);
            // Both ends leave the node leaning away from each other, and the
            // curve joins them outside it.
            let out = |x: f64, y: f64, sign: f64, far: f64| {
                let lean = if far != 0.0 { 1.0 } else { 0.0 };
                if sideways {
                    Point(x + sign * spread * lean, y + far)
                } else {
                    Point(x + far, y + sign * spread * lean)
                }
            };
            let sx = if sideways { ax - ux * st } else { ax + ux * st };
            let sy = if sideways { ay + uy * st } else { ay - uy * st };
            let ex = bx + ux * et;
            let ey = by + uy * et;
            path = EdgePath {
                d: format!(
                    "M{}C{} {} {}",
                    Point(sx, sy),
                    out(ax, ay, -1.0, reach),
                    out(bx, by, 1.0, reach),
                    Point(ex, ey)
                ),
                sx: ax,
                sy: ay,
                sdx: if sideways { ux } else { -ux },
                sdy: if sideways { -uy } else { uy },
                ex: bx,
                ey: by,
                edx: -ux,
                edy: -uy,
            };
            if sideways {
                cedges[i].label_x = c.x;
                cedges[i].label_y = ay.max(by) + reach * 0.75 + 6.0 + label.height / 2.0;
            } else {
                cedges[i].label_x = ax.max(bx) + reach * 0.75 + 8.0 + label.width / 2.0;
                cedges[i].label_y = c.y;
            }
        } else {
            if cedges[i].route.len() < 6 {
                continue;
            }
            path = route_path(
                &cedges[i].route,
                None,
                trim(start_type),
                trim(end_type),
                None,
                None,
            );
        }
        let color = style
            .as_ref()
            .map_or(Cow::Borrowed("var(--_l)"), ResolvedStyle::line_color);
        // Mermaid animates these dashes. A still picture keeps the dashes.
        let dashes = if edge.internal {
            "2 3"
        } else if edge.pattern == "dotted" {
            "5 4"
        } else if edge.animate {
            "9 5"
        } else {
            ""
        };
        let _ = write!(
            edges_out,
            "<g class=\"{p}-edge {p}-{}{}\" data-id=\"{}\"{}><path d=\"{}\"{}/>{}{}</g>",
            edge.relationship_type,
            class_names(&RE_EDGE_CLASSES.replace(&edge.classes, |_| String::new())),
            EscText(&edge.id),
            style.as_ref().map_or("", |s| s.line.as_str()),
            path.d,
            fragment(|f| if dashes.is_empty() {
                Ok(())
            } else {
                write!(f, " stroke-dasharray=\"{dashes}\"")
            }),
            mark(start_type, path.sx, path.sy, path.sdx, path.sdy, &color),
            mark(end_type, path.ex, path.ey, path.edx, path.edy, &color),
        );
        if label.width > 0.0 {
            let semantic = matches!(edge.relationship_type, "include" | "extend");
            let label_attrs = format!(
                "{}{}",
                if semantic { " fill=\"var(--_m)\"" } else { "" },
                style.as_ref().map_or("", |s| s.text.as_str())
            );
            let _ = write!(
                labels_out,
                "{}",
                edge_label_svg(
                    &edge.id,
                    label,
                    cedges[i].label_x,
                    cedges[i].label_y,
                    &label_attrs,
                )
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
            js::Number(small)
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");

    let svg = svg_document(
        "usecase",
        width,
        height,
        size,
        ctx,
        Accessible {
            acc_title: Some(&model.acc_title),
            acc_descr: Some(&model.acc_descr),
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
