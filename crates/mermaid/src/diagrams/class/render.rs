//! Pele's `diagrams/class/render.ts`.

use std::{collections::HashMap, fmt::Write as _};

use indexmap::IndexMap;

use super::{
    db::ClassModel,
    graph::{ClassGraph, GraphEdge, NodeShape, Pattern, build_class_graph},
    markers::{class_marker, class_marker_trim},
    members::ClassMember,
    types::{ClassNode, StyleClass},
};
use crate::Ctx;
use crate::{
    Config, Link, Rendered,
    config::{Map, Value},
    diagrams::common::{
        fit_width::{runs_across, tighten, turn_to_fit},
        generics::parse_generic_types,
    },
    js::{self, Lazy},
    layout::{
        compound::{CEdge, CNode, Dir, cnode, compound_layout, direction},
        layered::LayeredOptions,
    },
    svg::{
        builder::{esc, esc_text, label_svg, num},
        cluster::{Crossings, cluster_title_x, mark_crossings, struck_title, title_crossed},
        edges::{EdgePath, edge_label_svg, route_path},
        root::{Accessible, svg_document},
        shapes::end_side,
        theme::{RADIUS, ResolvedStyle, class_names, resolve_style},
        title::with_title,
    },
    text::{Host, Label, Style, decode_entities, layout_label},
    util::url::linked,
};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 56.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const PAD_X: f64 = 12.0;
const TITLE_PAD: f64 = 8.0;
const SECTION_PAD: f64 = 6.0;
const EMPTY_SECTION: f64 = 10.0;
const MIN_WIDTH: f64 = 64.0;
const FOLD: f64 = 8.0;
const LINE: &str = "var(--_l)";
const TARGETS: [&str; 4] = ["_self", "_blank", "_parent", "_top"];

static RE_DEFAULT: Lazy = Lazy::new(r"^default\s?", "");

struct Line {
    text: String,
    width: f64,
    classifier: String,
}

struct View {
    style: ResolvedStyle,
    label: Label,
    w: f64,
    h: f64,
    annotations: Vec<String>,
    attributes: Vec<Line>,
    methods: Vec<Line>,
    title_h: f64,
    attributes_h: f64,
    font_size: f64,
}

#[derive(Clone, Copy)]
struct End {
    /// The edge whose route this end is on.
    edge: usize,
    at: usize,
    toward: f64,
    order: usize,
    room: f64,
    side: i32,
    /// Whether the text of this end goes on the right of, or below, its line.
    after: bool,
}

struct Card {
    text: String,
    x: f64,
    y: f64,
}

/// What every part of one drawing is measured with.
struct Sizes<'a, H> {
    measure: &'a H,
    size: f64,
    tiny: f64,
    tiny_height: f64,
    hide_empty: bool,
}

/// The styles of a class from its classDefs and its own. A class may carry
/// the declarations of many classDefs many times over. Each distinct
/// declaration is checked once, and only the last value of each property that
/// has an effect is resolved.
struct Styler<'m> {
    style_classes: &'m IndexMap<String, StyleClass>,
    effect: HashMap<String, String>,
    from_defs: HashMap<String, Vec<(String, String)>>,
    kept: IndexMap<String, String>,
}

impl Styler<'_> {
    fn keep(&mut self, decl: &str) {
        let prop = self
            .effect
            .entry(decl.to_string())
            .or_insert_with(|| {
                let one = resolve_style(&[decl]);
                if one.shape.is_empty() && one.text.is_empty() {
                    String::new()
                } else {
                    let name = match decl.find(':') {
                        Some(at) => &decl[..at],
                        None => &decl[..decl.char_indices().last().map_or(0, |(at, _)| at)],
                    };
                    js::trim(name).to_lowercase()
                }
            })
            .clone();
        if !prop.is_empty() {
            self.kept.insert(prop, decl.to_string());
        }
    }

    fn class_style(&mut self, cls: &ClassNode) -> ResolvedStyle {
        self.kept.clear();
        match self.from_defs.get(&cls.css_classes) {
            None => {
                for name in cls.css_classes.split(' ') {
                    let Some(def) = self.style_classes.get(name) else {
                        continue;
                    };
                    for s in &def.styles {
                        for part in s.split(',') {
                            self.keep(part);
                        }
                    }
                }
                let base = self
                    .kept
                    .iter()
                    .map(|(prop, decl)| (prop.clone(), decl.clone()))
                    .collect();
                self.from_defs.insert(cls.css_classes.clone(), base);
            }
            Some(base) => {
                for (prop, decl) in base {
                    self.kept.insert(prop.clone(), decl.clone());
                }
            }
        }
        for s in &cls.styles {
            self.keep(s);
        }
        let kept: Vec<&str> = self.kept.values().map(String::as_str).collect();
        resolve_style(&kept)
    }
}

fn member_line(member: &ClassMember, measure: &impl Host, size: f64) -> Line {
    let text = js::trim(&decode_entities(&member.display_details().display_text)).to_string();
    let width = measure.width(
        &text,
        size,
        if member.classifier == "*" {
            Style::ITALIC
        } else {
            0
        },
    );
    Line {
        text,
        width,
        classifier: member.classifier.clone(),
    }
}

pub fn render_class(model: &ClassModel, config: &Config, ctx: &Ctx<'_, impl Host>) -> Rendered {
    let fitted = |down: bool| tighten(ctx, |tight| draw(model, config, ctx, down, tight));
    if runs_across(Some(&model.direction)) {
        turn_to_fit(ctx, fitted)
    } else {
        fitted(false)
    }
}

fn base(label: Label, w: f64, h: f64, font_size: f64) -> View {
    View {
        style: resolve_style::<&str>(&[]),
        label,
        w,
        h,
        annotations: Vec::new(),
        attributes: Vec::new(),
        methods: Vec::new(),
        title_h: 0.0,
        attributes_h: 0.0,
        font_size,
    }
}

fn class_view(sizes: &Sizes<'_, impl Host>, styler: &mut Styler, cls: &ClassNode) -> View {
    let measure = sizes.measure;
    let style = styler.class_style(cls);
    let font_size = style.font_size.unwrap_or(sizes.size);
    let member_size = js::round(font_size * 0.875);
    let line_height = js::round(member_size * 1.5);
    // The generic type is escaped so that `<T>` is not read as a tag.
    let generic = if cls.kind.is_empty() {
        String::new()
    } else {
        format!(
            "&lt;{}&gt;",
            parse_generic_types(&cls.kind)
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        )
    };
    let label = layout_label(
        Some(&(cls.label.clone() + &generic)),
        false,
        measure,
        font_size,
        4000.0,
        Style::BOLD | if style.italic { Style::ITALIC } else { 0 },
    );
    let annotations: Vec<String> = cls
        .annotations
        .iter()
        .map(|a| format!("«{}»", js::trim(&decode_entities(a))))
        .collect();
    let attributes: Vec<Line> = cls
        .members
        .iter()
        .map(|m| member_line(m, measure, member_size))
        .collect();
    let methods: Vec<Line> = cls
        .methods
        .iter()
        .map(|m| member_line(m, measure, member_size))
        .collect();
    let mut inner = label.width;
    for a in &annotations {
        inner = inner.max(measure.width(a, sizes.tiny, 0));
    }
    for line in attributes.iter().chain(&methods) {
        inner = inner.max(line.width);
    }
    let sections = !(sizes.hide_empty && attributes.is_empty() && methods.is_empty());
    let section = |count: usize| -> f64 {
        if !sections {
            0.0
        } else if count > 0 {
            2.0 * SECTION_PAD + count as f64 * line_height
        } else {
            EMPTY_SECTION
        }
    };
    let title_h = 2.0 * TITLE_PAD
        + annotations.len() as f64 * sizes.tiny_height
        + label.height.max(js::round(font_size * 1.5));
    let attributes_h = section(attributes.len());
    let h = title_h + attributes_h + section(methods.len());
    View {
        style,
        label,
        w: (inner + 2.0 * PAD_X).max(MIN_WIDTH),
        h,
        annotations,
        attributes,
        methods,
        title_h,
        attributes_h,
        font_size,
    }
}

/// Where an end of an edge meets the side of its node, recorded on that side.
fn attach(
    route: &mut [f64],
    at: usize,
    c: &CNode,
    view_w: f64,
    key_item: usize,
    end: (usize, usize, f64),
    sides: &mut IndexMap<usize, Vec<End>>,
) -> Option<End> {
    let (edge, order, room) = end;
    let side = end_side(route, at, c);
    if side < 0 {
        return None;
    }
    let slack = (c.w - view_w) / 2.0;
    if side == 1 {
        route[at] -= slack;
    } else if side == 3 {
        route[at] += slack;
    }
    let across = (side & 1) as usize;
    let toward = route[if at == 0 { 3 } else { at - 3 } + across];
    let bend = toward - route[at + across];
    let end = End {
        edge,
        at,
        toward,
        order,
        room,
        side,
        after: if across == 1 { bend > 0.0 } else { bend >= 0.0 },
    };
    sides
        .entry(key_item * 4 + side as usize)
        .or_default()
        .push(end);
    Some(end)
}

/// Whether a point lies on the border of one of the namespaces around a
/// class.
fn on_border(cnodes: &[CNode], item: usize, x: f64, y: f64) -> bool {
    let mut g = cnodes[item].parent;
    let mut hops = 0;
    while g >= 0 && hops < 10000 {
        let c = &cnodes[g as usize];
        let dx = (x - c.x).abs();
        let dy = (y - c.y).abs();
        if ((dy - c.h / 2.0).abs() < 0.5 && dx <= c.w / 2.0 + 0.5)
            || ((dx - c.w / 2.0).abs() < 0.5 && dy <= c.h / 2.0 + 0.5)
        {
            return true;
        }
        g = c.parent;
        hops += 1;
    }
    false
}

/// An end with text runs straight out of its node for the length of the
/// text.
fn straighten(route: &mut Vec<f64>, end: Option<End>, room: f64, first: bool, tiny_height: f64) {
    let Some(end) = end else {
        return;
    };
    if room <= 0.0 {
        return;
    }
    let across = (end.side & 1) as usize;
    let at = if first { 0 } else { route.len() - 3 };
    let along = at + 1 - across;
    let reach = (route[if first { 3 } else { at - 3 } + 1 - across] - route[along]).abs();
    let length = js::min(
        if across == 1 {
            room + 10.0
        } else {
            tiny_height + 6.0
        },
        reach * 0.35,
    );
    if length < 4.0 {
        return;
    }
    let mut point = [route[at], route[at + 1], route[at + 2]];
    point[1 - across] += if end.side == 0 || end.side == 3 {
        -1.0
    } else {
        1.0
    } * length;
    let from = if first { 3 } else { at };
    route.splice(from..from, point);
}

struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

#[allow(clippy::too_many_arguments)]
fn card(
    cards: &mut Vec<Card>,
    bounds: &mut Bounds,
    tiny_height: f64,
    raw: &str,
    width: f64,
    x: f64,
    y: f64,
    side: i32,
    after: bool,
) {
    if raw.is_empty() {
        return;
    }
    let mut cx = x;
    let mut cy = y;
    if side & 1 == 1 {
        cx += if side == 3 { -1.0 } else { 1.0 } * (5.0 + width / 2.0);
        cy += if after { 1.0 } else { -1.0 } * (7.0 + tiny_height / 2.0);
    } else {
        cx += if after { 1.0 } else { -1.0 } * (9.0 + width / 2.0);
        cy += if side == 0 { -1.0 } else { 1.0 } * (3.0 + tiny_height / 2.0);
    }
    cards.push(Card {
        text: decode_entities(raw),
        x: cx,
        y: cy,
    });
    bounds.min_x = js::min(bounds.min_x, cx - width / 2.0);
    bounds.max_x = js::max(bounds.max_x, cx + width / 2.0);
    bounds.min_y = js::min(bounds.min_y, cy - tiny_height / 2.0);
    bounds.max_y = js::max(bounds.max_y, cy + tiny_height / 2.0);
}

/// `turned` draws a diagram that runs across as one that runs down. `tight`
/// gives up the room that keeps a namespace's name clear of an edge coming in
/// beside it.
fn draw(
    model: &ClassModel,
    config: &Config,
    ctx: &Ctx<'_, impl Host>,
    turned: bool,
    tight: bool,
) -> Rendered {
    let p = crate::PREFIX;
    let empty = Map::new();
    let conf = config.section("class").unwrap_or(&empty);
    let graph: ClassGraph = build_class_graph(
        model,
        conf.get("hierarchicalNamespaces") != Some(&Value::Bool(false)),
    );
    let hide_empty = conf.get("hideEmptyMembersBox") == Some(&Value::Bool(true));
    let size = ctx.options.font_size;
    let small = js::round(size * 0.875);
    let tiny = js::round(size * 0.8);
    let tiny_height = js::round(tiny * 1.4);
    let measurer = ctx.host;
    let measure = measurer;
    let pad = ctx.options.padding.unwrap_or(8.0);
    let root_dir = if turned {
        Dir::TB
    } else {
        direction(Some(&model.direction))
    };
    let sizes = Sizes {
        measure,
        size,
        tiny,
        tiny_height,
        hide_empty,
    };
    let mut styler = Styler {
        style_classes: &model.style_classes,
        effect: HashMap::new(),
        from_defs: HashMap::new(),
        kept: IndexMap::new(),
    };

    let count = graph.nodes.len();
    let mut children = vec![0i32; count];
    for node in &graph.nodes {
        if node.parent >= 0 {
            children[node.parent as usize] += 1;
        }
    }

    // Namespaces that an edge may cross into are made wide enough for the
    // title to sit beside it.
    let mut crossed = vec![0u8; count];
    for edge in &graph.edges {
        if edge.source < 0 || edge.target < 0 {
            continue;
        }
        let a = graph.nodes[edge.source as usize].parent;
        let b = graph.nodes[edge.target as usize].parent;
        if a == b {
            continue;
        }
        for start in [a, b] {
            let mut g = start;
            while g >= 0 && crossed[g as usize] == 0 {
                crossed[g as usize] = 1;
                g = graph.nodes[g as usize].parent;
            }
        }
    }

    let mut views: Vec<View> = Vec::with_capacity(count);
    let mut cnodes: Vec<CNode> = Vec::with_capacity(count);
    for (i, node) in graph.nodes.iter().enumerate() {
        let (view, mut c) = if node.is_group {
            let label = layout_label(Some(node.label), false, measure, size, 4000.0, 0);
            let mut view = base(label, 0.0, 0.0, size);
            if children[i] > 0 {
                let mut c = cnode(0.0, 0.0, -1);
                c.is_group = true;
                c.pad_x = GROUP_PAD;
                c.pad_top = if view.label.height > 0.0 {
                    view.label.height + 16.0
                } else {
                    GROUP_PAD
                };
                c.pad_bottom = GROUP_PAD;
                c.min_w = view.label.width + 2.0 * GROUP_PAD - 12.0
                    + if crossed[i] == 1 && !tight {
                        (view.label.width + 24.0).min(240.0)
                    } else {
                        0.0
                    };
                (view, c)
            } else {
                view.w = (view.label.width + 2.0 * GROUP_PAD).max(80.0);
                view.h = view.label.height + 2.0 * GROUP_PAD;
                let c = cnode(view.w, view.h, -1);
                (view, c)
            }
        } else if let Some(cls) = node.class_node {
            let view = class_view(&sizes, &mut styler, cls);
            let c = cnode(view.w, view.h, -1);
            (view, c)
        } else if node.shape == NodeShape::Note {
            let label = layout_label(Some(node.label), true, measure, small, 260.0, 0);
            let w = (label.width + 2.0 * PAD_X).max(40.0);
            let h = label.height + 2.0 * TITLE_PAD;
            let view = base(label, w, h, small);
            let c = cnode(view.w, view.h, -1);
            (view, c)
        } else {
            let label = layout_label(Some(node.label), false, measure, small, 200.0, 0);
            let w = label.width.max(12.0) + 4.0;
            let h = label.height.max(12.0);
            let view = base(label, w, h, small);
            let c = cnode(view.w, view.h, -1);
            (view, c)
        };
        c.parent = node.parent;
        c.seq = i as f64;
        views.push(view);
        cnodes.push(c);
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut drawn: Vec<&GraphEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut start_room: Vec<f64> = Vec::new();
    let mut end_room: Vec<f64> = Vec::new();
    // Edges leave one side of a class and arrive at the other, so a class with
    // many of them is widened until they, and the text at their ends, fit
    // side by side.
    let mut crowd = vec![0f64; views.len() * 4];
    let meet = |crowd: &mut [f64], item: usize, slot: usize, room: f64| {
        crowd[item * 4 + slot] += 1.0;
        if room > crowd[item * 4 + slot + 1] {
            crowd[item * 4 + slot + 1] = room;
        }
    };
    for edge in &graph.edges {
        if edge.source < 0 || edge.target < 0 {
            continue;
        }
        let (source, target) = (edge.source as usize, edge.target as usize);
        let label = layout_label(edge.label, true, measure, small, 200.0, 0);
        let start_w = if edge.start_label_right.is_empty() {
            0.0
        } else {
            measure.width(&decode_entities(edge.start_label_right), tiny, 0)
        };
        let end_w = if edge.end_label_left.is_empty() {
            0.0
        } else {
            measure.width(&decode_entities(edge.end_label_left), tiny, 0)
        };
        if source == target {
            cnodes[source].w += 2.0
                * (LOOP
                    + if label.width > 0.0 {
                        label.width + 12.0
                    } else {
                        0.0
                    });
        } else {
            meet(&mut crowd, source, 0, start_w);
            meet(&mut crowd, target, 2, end_w);
        }
        start_room.push(start_w);
        end_room.push(end_w);
        drawn.push(edge);
        cedges.push(CEdge {
            src: source,
            dst: target,
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

    if root_dir == Dir::TB || root_dir == Dir::BT {
        for i in 0..views.len() {
            if graph.nodes[i].class_node.is_none() {
                continue;
            }
            let mut need: f64 = 0.0;
            for slot in [0, 2] {
                let count = crowd[i * 4 + slot];
                let room = crowd[i * 4 + slot + 1];
                if count > 1.0 {
                    need = need
                        .max((count - 1.0) * if room > 0.0 { room + 24.0 } else { 14.0 } + 24.0);
                }
            }
            need = need.min(480.0);
            if need > views[i].w {
                cnodes[i].w += need - views[i].w;
                views[i].w = need;
            }
        }
    }

    let lettered = (0..drawn.len()).any(|i| start_room[i] > 0.0 || end_room[i] > 0.0);
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: conf.number_option("nodeSpacing", NODE_SEP),
            edge_sep: EDGE_SEP,
            rank_sep: conf.number_option(
                "rankSpacing",
                if lettered { RANK_SEP + 16.0 } else { RANK_SEP },
            ),
            port_sep: 20.0,
            apart: true,
            ..LayeredOptions::default()
        },
    );

    // Every edge reaches the middle of a side. Where several share a side they
    // are spread along it, in the order of where they come from. An end with
    // text runs straight for the length of the text, which sits beside it on
    // the side the line then bends toward, away from the neighbors that bend
    // the other way. Neighbors are set apart far enough for the text between
    // them.
    let mut sides: IndexMap<usize, Vec<End>> = IndexMap::new();
    let mut starts: Vec<Option<End>> = Vec::with_capacity(cedges.len());
    let mut ends: Vec<Option<End>> = Vec::with_capacity(cedges.len());
    for i in 0..cedges.len() {
        let (src, dst) = (cedges[i].src, cedges[i].dst);
        let routed = src != dst && cedges[i].route.len() >= 6;
        if !routed {
            starts.push(None);
            ends.push(None);
            continue;
        }
        let route = &mut cedges[i].route;
        starts.push(attach(
            route,
            0,
            &cnodes[src],
            views[src].w,
            src,
            (i, i, start_room[i]),
            &mut sides,
        ));
        let last = route.len() - 3;
        ends.push(attach(
            route,
            last,
            &cnodes[dst],
            views[dst].w,
            dst,
            (i, i, end_room[i]),
            &mut sides,
        ));
    }
    let mut gaps: Vec<f64> = Vec::new();
    for (&key, list) in sides.iter_mut() {
        if list.len() < 2 {
            continue;
        }
        let item = key >> 2;
        let across = key & 1;
        let length = if across == 1 {
            views[item].h
        } else {
            views[item].w
        };
        list.sort_by(|a, b| {
            let d = a.toward - b.toward;
            if d != 0.0 && !d.is_nan() {
                js::compare(d)
            } else {
                a.order.cmp(&b.order)
            }
        });
        let mut total = 0.0;
        for j in 0..list.len() - 1 {
            let a = &list[j];
            let b = &list[j + 1];
            let mut gap = if across == 1 { 20.0 } else { 24.0 };
            if a.room > 0.0 && a.after {
                gap += if across == 1 {
                    tiny_height + 6.0
                } else {
                    a.room
                };
            }
            if b.room > 0.0 && !b.after {
                gap += if across == 1 {
                    tiny_height + 6.0
                } else {
                    b.room
                };
            }
            if gaps.len() <= j {
                gaps.resize(j + 1, 0.0);
            }
            gaps[j] = gap;
            total += gap;
        }
        let scale = js::min(1.0, (length - 24.0) / total);
        if scale <= 0.0 {
            continue;
        }
        let mut at = if across == 1 {
            cnodes[item].y
        } else {
            cnodes[item].x
        } - (total * scale) / 2.0;
        for (j, end) in list.iter().enumerate() {
            let route = &mut cedges[end.edge].route;
            let end_at = end.at as isize;
            // An edge that crosses the border of the class's namespace close
            // by runs straight to it: the point where it crosses moves into
            // line with its end.
            let step: isize = if end_at == 0 { 3 } else { -3 };
            let first = end_at + step;
            let first_value = route[first as usize + across];
            let mut border: isize = -1;
            let mut k = first;
            while k >= 0
                && (k as usize) < route.len()
                && (route[k as usize + across] - first_value).abs() < 0.5
            {
                if on_border(&cnodes, item, route[k as usize], route[k as usize + 1]) {
                    border = k;
                    break;
                }
                k += step;
            }
            if border >= 0 && (first_value - at).abs() <= 24.0 {
                let mut k = first;
                while k != border + step {
                    route[k as usize + across] = at;
                    k += step;
                }
            }
            route[end.at + across] = at;
            // Past the gaps this side set, `gaps[j]` is what another side
            // left, or undefined; either way `at` is not read again.
            at += gaps.get(j).copied().unwrap_or(f64::NAN) * scale;
        }
    }

    for i in 0..cedges.len() {
        straighten(
            &mut cedges[i].route,
            starts[i],
            start_room[i],
            true,
            tiny_height,
        );
        straighten(
            &mut cedges[i].route,
            ends[i],
            end_room[i],
            false,
            tiny_height,
        );
    }

    let mut bounds = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: layout.width,
        max_y: layout.height,
    };
    let mut cards: Vec<Card> = Vec::new();
    let mut loop_seen: HashMap<usize, i32> = HashMap::new();
    let mut loop_at = vec![0i32; cedges.len()];
    for i in 0..cedges.len() {
        let ce = &cedges[i];
        let edge = drawn[i];
        let route = &ce.route;
        if ce.src == ce.dst {
            let k = *loop_seen.get(&ce.src).unwrap_or(&0);
            loop_seen.insert(ce.src, k + 1);
            loop_at[i] = k;
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let x = c.x + view.w / 2.0;
            let spread = (view.h / 2.0 - 4.0).min(8.0 + k as f64 * 6.0);
            card(
                &mut cards,
                &mut bounds,
                tiny_height,
                edge.start_label_right,
                start_room[i],
                x,
                c.y - spread,
                1,
                false,
            );
            card(
                &mut cards,
                &mut bounds,
                tiny_height,
                edge.end_label_left,
                end_room[i],
                x,
                c.y + spread,
                1,
                true,
            );
        } else if route.len() >= 6 {
            let last = route.len() - 3;
            let from = starts[i];
            let to = ends[i];
            card(
                &mut cards,
                &mut bounds,
                tiny_height,
                edge.start_label_right,
                start_room[i],
                route[0],
                route[1],
                from.map_or(2, |e| e.side),
                from.is_none_or(|e| e.after),
            );
            card(
                &mut cards,
                &mut bounds,
                tiny_height,
                edge.end_label_left,
                end_room[i],
                route[last],
                route[last + 1],
                to.map_or(0, |e| e.side),
                to.is_none_or(|e| e.after),
            );
        }
    }

    let Bounds {
        min_x,
        min_y,
        max_x,
        max_y,
    } = bounds;
    let ox = pad - min_x;
    let oy = pad - min_y;
    let width = (max_x - min_x + 2.0 * pad).ceil();
    let height = (max_y - min_y + 2.0 * pad).ceil();
    let mut links: Vec<Link> = Vec::new();

    let mut crossings: Crossings = Crossings::new();
    if graph.nodes.first().is_some_and(|node| node.is_group) {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }

    let mut clusters = String::new();
    let mut struck_titles = String::new();
    let mut nodes_out = String::new();
    for i in 0..views.len() {
        let view = &views[i];
        let c = &cnodes[i];
        let node = &graph.nodes[i];
        let x = c.x + ox;
        let y = c.y + oy;
        let id = esc_text(node.id);
        let w = if c.is_group { c.w } else { view.w };
        let h = if c.is_group { c.h } else { view.h };
        if node.is_group {
            let title_x = if c.is_group {
                cluster_title_x(c, view.label.width, &crossings, GROUP_PAD)
            } else {
                c.x
            };
            let title_y = y - h / 2.0 + 8.0 + view.label.height / 2.0;
            let title_attrs = format!(" class=\"{p}-cluster-label\" fill=\"var(--_m)\"");
            let struck = c.is_group && title_crossed(c, view.label.width, title_x, &crossings);
            if struck {
                struck_titles += &struck_title(
                    &view.label,
                    title_x + ox,
                    title_y,
                    &title_attrs,
                    &id,
                    &cnodes,
                    i,
                );
            }
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster {p}-namespace\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"/>{}</g>",
                num(x - w / 2.0),
                num(y - h / 2.0),
                num(w),
                num(h),
                if struck {
                    String::new()
                } else {
                    label_svg(&view.label, title_x + ox, title_y, &title_attrs)
                }
            );
            continue;
        }
        let at = format!(" transform=\"translate({},{})\"", num(x), num(y));
        if let Some(cls) = node.class_node {
            nodes_out += &class_svg(view, cls, &id, &at, &sizes, &mut links, ctx);
        } else if node.shape == NodeShape::Note {
            let r = w / 2.0;
            let t = -h / 2.0;
            let _ = write!(
                nodes_out,
                "<g class=\"{p}-node {p}-note\" data-id=\"{id}\"{at}><path d=\"M{},{}H{}L{},{}V{}H{}Z\" fill=\"var(--_a)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/><path d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/>{}</g>",
                num(-r),
                num(t),
                num(r - FOLD),
                num(r),
                num(t + FOLD),
                num(-t),
                num(-r),
                num(r - FOLD),
                num(t),
                num(t + FOLD),
                num(r),
                label_svg(
                    &view.label,
                    0.0,
                    0.0,
                    &format!(" class=\"{p}-label\" font-size=\"{}\"", js::num_str(small))
                )
            );
        } else {
            let _ = write!(
                nodes_out,
                "<g class=\"{p}-node {p}-interface\" data-id=\"{id}\"{at}>{}</g>",
                label_svg(
                    &view.label,
                    0.0,
                    0.0,
                    &format!(" class=\"{p}-label\" font-size=\"{}\"", js::num_str(small))
                )
            );
        }
    }

    let mut edges_out = String::new();
    let mut labels_out = String::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let start_type = edge.arrow_type_start;
        let end_type = edge.arrow_type_end;
        let start_trim = class_marker_trim(start_type);
        let end_trim = class_marker_trim(end_type);
        let ce = &mut cedges[i];
        let path: EdgePath = if ce.src == ce.dst {
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let k = loop_at[i] as f64;
            let x = c.x + ox + view.w / 2.0;
            let y = c.y + oy;
            let spread = (view.h / 2.0 - 4.0).min(8.0 + k * 6.0);
            let reach = LOOP + k * 8.0;
            let path = EdgePath {
                d: format!(
                    "M{},{}C{},{} {},{} {},{}",
                    num(x + start_trim),
                    num(y - spread),
                    num(x + reach + start_trim),
                    num(y - spread),
                    num(x + reach + end_trim),
                    num(y + spread),
                    num(x + end_trim),
                    num(y + spread)
                ),
                sx: x,
                sy: y - spread,
                sdx: -1.0,
                sdy: 0.0,
                ex: x,
                ey: y + spread,
                edx: -1.0,
                edy: 0.0,
            };
            ce.label_x = c.x + view.w / 2.0 + reach + 4.0 + ce.label_w / 2.0;
            ce.label_y = c.y;
            path
        } else {
            for k in (0..ce.route.len()).step_by(3) {
                ce.route[k] += ox;
                ce.route[k + 1] += oy;
            }
            route_path(&ce.route, None, start_trim, end_trim, None, None)
        };
        let attrs = match edge.pattern {
            Pattern::Dashed => " stroke-dasharray=\"6 4\"",
            Pattern::Dotted => " stroke-dasharray=\"2 4\"",
            Pattern::Solid => "",
        };
        let _ = write!(
            edges_out,
            "<g class=\"{p}-edge{}\" data-id=\"{}\"><path d=\"{}\"{attrs}/>{}{}</g>",
            if edge.pattern == Pattern::Dotted {
                format!(" {p}-note-edge")
            } else {
                format!(" {p}-relation")
            },
            esc_text(&edge.id),
            path.d,
            class_marker(start_type, path.sx, path.sy, path.sdx, path.sdy, LINE),
            class_marker(end_type, path.ex, path.ey, path.edx, path.edy, LINE)
        );
        if label.width > 0.0 {
            labels_out += &edge_label_svg(&edge.id, label, ce.label_x + ox, ce.label_y + oy, "");
        }
    }

    let mut cards_out = String::new();
    for item in &cards {
        let _ = write!(
            cards_out,
            "<text class=\"{p}-cardinality\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            num(item.x + ox),
            num(item.y + oy + tiny * 0.35),
            esc(&item.text)
        );
    }

    let mut content = String::new();
    if !clusters.is_empty() {
        let _ = write!(content, "<g class=\"{p}-clusters\">{clusters}</g>");
    }
    if !edges_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edges\" fill=\"none\" stroke=\"{LINE}\" stroke-linecap=\"round\">{edges_out}</g>"
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
            js::num_str(small)
        );
    }
    if !cards_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-cardinalities\" font-size=\"{}\" fill=\"var(--_m)\">{cards_out}</g>",
            js::num_str(tiny)
        );
    }
    let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");
    let titled = with_title(
        model.title.as_deref(),
        measure,
        size,
        pad,
        width,
        height,
        content,
    );
    let svg = svg_document(
        "class",
        titled.width,
        titled.height,
        size,
        ctx,
        Accessible {
            acc_title: model.acc_title.as_deref(),
            acc_descr: model.acc_descr.as_deref(),
        },
        &titled.content,
    );

    Rendered {
        svg,
        width: titled.width,
        height: titled.height,
        links,
    }
}

#[allow(clippy::too_many_arguments)]
fn class_svg(
    view: &View,
    cls: &ClassNode,
    id: &str,
    at: &str,
    sizes: &Sizes<'_, impl Host>,
    links: &mut Vec<Link>,
    ctx: &Ctx<'_, impl Host>,
) -> String {
    let p = crate::PREFIX;
    let (w, h, style) = (view.w, view.h, &view.style);
    let tiny = sizes.tiny;
    let tiny_height = sizes.tiny_height;
    let left = -w / 2.0;
    let top = -h / 2.0;
    let member_size = js::round(view.font_size * 0.875);
    let line_height = js::round(member_size * 1.5);
    let mut body = format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
        num(left),
        num(top),
        num(w),
        num(h),
        style.shape
    );
    let first = top + view.title_h;
    let second = first + view.attributes_h;
    if h > view.title_h {
        let stroke = match style.stroke.as_deref() {
            Some(stroke) if !stroke.is_empty() => format!(" style=\"stroke:{}\"", esc(stroke)),
            _ => String::new(),
        };
        let _ = write!(
            body,
            "<path class=\"{p}-divider\" d=\"M{},{}H{}M{},{}H{}\" fill=\"none\" stroke=\"var(--_b)\"{stroke}/>",
            num(left),
            num(first),
            num(-left),
            num(left),
            num(second),
            num(-left)
        );
    }
    let mut y = top + TITLE_PAD;
    for annotation in &view.annotations {
        let _ = write!(
            body,
            "<text class=\"{p}-annotation\" x=\"0\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
            num(y + tiny_height / 2.0 + tiny * 0.35),
            js::num_str(tiny),
            esc(annotation)
        );
        y += tiny_height;
    }
    let title_size = if view.font_size == sizes.size {
        String::new()
    } else {
        format!(" font-size=\"{}\"", num(view.font_size))
    };
    body += &label_svg(
        &view.label,
        0.0,
        (y + first - TITLE_PAD) / 2.0,
        &format!(
            " class=\"{p}-label\"{title_size} font-weight=\"var(--_hw)\"{}",
            style.text
        ),
    );
    let lines = |list: &[Line], kind: &str, from: f64| -> String {
        let mut out = String::new();
        let mut baseline = from + SECTION_PAD + line_height / 2.0 + member_size * 0.35;
        for line in list {
            let mark = match line.classifier.as_str() {
                "$" => format!(
                    " {p}-static\"{}",
                    if style.text.is_empty() {
                        " style=\"text-decoration:underline\"".to_string()
                    } else {
                        format!(
                            "{}text-decoration:underline\"",
                            &style.text[..style.text.len() - 1]
                        )
                    }
                ),
                "*" => format!(" {p}-abstract\" font-style=\"italic\"{}", style.text),
                _ => format!("\"{}", style.text),
            };
            let _ = write!(
                out,
                "<text class=\"{p}-member {p}-{kind}{mark} x=\"{}\" y=\"{}\">{}</text>",
                num(left + PAD_X),
                num(baseline),
                esc(&line.text)
            );
            baseline += line_height;
        }
        out
    };
    if !view.attributes.is_empty() || !view.methods.is_empty() {
        let _ = write!(
            body,
            "<g class=\"{p}-members\" font-size=\"{}\">{}{}</g>",
            js::num_str(member_size),
            lines(&view.attributes, "attribute", first),
            lines(&view.methods, "method", second)
        );
    }
    if let Some(tooltip) = cls.tooltip.as_deref().filter(|tip| !tip.is_empty()) {
        body = format!("<title>{}</title>{body}", esc_text(tooltip));
    }
    if let Some(link) = cls.link.as_deref().filter(|link| !link.is_empty()) {
        let target = match cls.link_target.as_deref() {
            Some(target) if TARGETS.contains(&target) => format!(" target=\"{target}\""),
            _ => String::new(),
        };
        body = linked(&body, link, &cls.id, links, ctx, &target);
    }
    format!(
        "<g class=\"{p}-node {p}-class{}\" data-id=\"{id}\"{at}>{body}</g>",
        class_names(&RE_DEFAULT.replace(&cls.css_classes, |_| String::new()))
    )
}
