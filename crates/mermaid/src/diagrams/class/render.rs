//! Pele's `diagrams/class/render.ts`.

use crate::geom::{Axis, Direction, Point, RoutePoint, Side, Size};
use crate::svg::edges::Curve;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};

use crate::config::positive;
use crate::svg::builder::rewrite;
use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use indexmap::IndexMap;

use super::{
    db::ClassModel,
    graph::{ClassGraph, GraphEdge, NodeShape, Pattern},
    members::{ClassMember, Classifier},
    types::{ClassNode, RelationType, StyleClass},
};
use crate::{Config, Ctx};
use crate::{
    Error, Link, Rendered,
    diagrams::{Render, common::generics::parse_generic_types},
    js,
    layout::{
        compound::{CEdge, CNode, ancestors, compound_layout},
        layered::LayeredOptions,
    },
    svg::{
        builder::{Esc, EscText, fragment, num},
        cluster::{Crossings, struck_title},
        edges::{EdgePath, edge_label_svg, route_path},
        root::svg_document,
        theme::{RADIUS, ResolvedStyle, node_class_names, resolve_style},
        title::with_title,
    },
    text::{Host, Label, Style, decode_entities},
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

struct Line {
    text: String,
    width: f64,
    classifier: String,
}

struct View {
    style: ResolvedStyle,
    label: Label,
    size: Size,
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
    side: Side,
    /// Whether the text of this end goes on the right of, or below, its line.
    after: bool,
}

struct Card {
    text: String,
    at: Point,
}

/// How many edges leave or reach one side of a class, and the widest text at
/// their ends there.
#[derive(Clone, Copy, Default)]
struct Crowd {
    count: f64,
    room: f64,
}

impl Crowd {
    fn meet(&mut self, room: f64) {
        self.count += 1.0;
        if room > self.room {
            self.room = room;
        }
    }
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

impl Line {
    fn of(member: &ClassMember, measure: &impl Host, size: f64) -> Line {
        let text = js::trim(&decode_entities(&member.display_details().display_text)).to_string();
        let width = measure.width(
            &text,
            size,
            if member.classifier == Some(Classifier::Abstract) {
                Style::ITALIC
            } else {
                0
            },
        );
        Line {
            text,
            width,
            classifier: member.classifier.map_or("", Classifier::marker).to_string(),
        }
    }
}

/// Mermaid's `class` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClassConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub hierarchical_namespaces: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub hide_empty_members_box: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub node_spacing: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub rank_spacing: Option<f64>,
}

impl Render for ClassModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let config = &config.class;
        let fitted = |down: bool| ctx.tighten(|tight| self.draw(config, ctx, down, tight));
        Ok(if self.direction.is_vertical() {
            fitted(false)
        } else {
            ctx.turn_to_fit(fitted)
        })
    }
}

impl View {
    /// A view of `label` alone, with no class styles.
    fn plain(label: Label, size: Size, font_size: f64) -> View {
        View {
            style: resolve_style::<&str>(&[]),
            label,
            size,
            annotations: Vec::new(),
            attributes: Vec::new(),
            methods: Vec::new(),
            title_h: 0.0,
            attributes_h: 0.0,
            font_size,
        }
    }

    fn class(sizes: &Sizes<'_, impl Host>, styler: &mut Styler, cls: &ClassNode) -> View {
        let measure = sizes.measure;
        let style = styler.class_style(cls);
        let font_size = style.font_size.unwrap_or(sizes.size);
        let member_size = (font_size * 0.875).round();
        let line_height = (member_size * 1.5).round();
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
        let label = Label::layout(
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
            .map(|m| Line::of(m, measure, member_size))
            .collect();
        let methods: Vec<Line> = cls
            .methods
            .iter()
            .map(|m| Line::of(m, measure, member_size))
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
            + label.height.max((font_size * 1.5).round());
        let attributes_h = section(attributes.len());
        let h = title_h + attributes_h + section(methods.len());
        View {
            style,
            label,
            size: Size::new((inner + 2.0 * PAD_X).max(MIN_WIDTH), h),
            annotations,
            attributes,
            methods,
            title_h,
            attributes_h,
            font_size,
        }
    }
}

/// Where an end of an edge meets the side of its node, recorded on that side.
fn attach(
    route: &mut [RoutePoint],
    at: usize,
    c: &CNode,
    view_w: f64,
    key_item: usize,
    end: (usize, usize, f64),
    sides: &mut IndexMap<(usize, Side), Vec<End>>,
) -> Option<End> {
    let (edge, order, room) = end;
    let side = c.side_of(route[at].at)?;
    let slack = (c.size.w - view_w) / 2.0;
    if side == Side::Right {
        route[at].at.x -= slack;
    } else if side == Side::Left {
        route[at].at.x += slack;
    }
    let across = if side.is_horizontal() {
        Axis::X
    } else {
        Axis::Y
    };
    let toward = route[if at == 0 { 1 } else { at - 1 }].at.along(across);
    let bend = toward - route[at].at.along(across);
    let end = End {
        edge,
        at,
        toward,
        order,
        room,
        side,
        after: if across == Axis::Y {
            bend > 0.0
        } else {
            bend >= 0.0
        },
    };
    sides.entry((key_item, side)).or_default().push(end);
    Some(end)
}

/// An end with text runs straight out of its node for the length of the
/// text.
fn straighten(
    route: &mut Vec<RoutePoint>,
    end: Option<End>,
    room: f64,
    first: bool,
    tiny_height: f64,
) {
    let Some(end) = end else {
        return;
    };
    if room <= 0.0 {
        return;
    }
    let across = if end.side.is_horizontal() {
        Axis::X
    } else {
        Axis::Y
    };
    let along = across.other();
    let at = if first { 0 } else { route.len() - 1 };
    let next = if first { 1 } else { at - 1 };
    let reach = (route[next].at.along(along) - route[at].at.along(along)).abs();
    let length = js::min(
        if across == Axis::Y {
            room + 10.0
        } else {
            tiny_height + 6.0
        },
        reach * 0.35,
    );
    if length < 4.0 {
        return;
    }
    let mut point = route[at];
    *point.at.along_mut(along) += if end.side == Side::Top || end.side == Side::Left {
        -1.0
    } else {
        1.0
    } * length;
    let from = if first { 1 } else { at };
    route.insert(from, point);
}

/// The text at the ends of edges, and the extent of the drawing grown to
/// hold it.
struct Cards {
    list: Vec<Card>,
    min: Point,
    max: Point,
    tiny_height: f64,
}

impl Cards {
    /// Adds the text `raw`, `width` wide, beside the end of an edge at `end` on
    /// `side` of its node; `after` puts it on the right of, or below, the line.
    fn add(&mut self, raw: &str, width: f64, end: Point, side: Side, after: bool) {
        if raw.is_empty() {
            return;
        }
        let tiny_height = self.tiny_height;
        let mut cx = end.x;
        let mut cy = end.y;
        if !side.is_horizontal() {
            cx += if side == Side::Left { -1.0 } else { 1.0 } * (5.0 + width / 2.0);
            cy += if after { 1.0 } else { -1.0 } * (7.0 + tiny_height / 2.0);
        } else {
            cx += if after { 1.0 } else { -1.0 } * (9.0 + width / 2.0);
            cy += if side == Side::Top { -1.0 } else { 1.0 } * (3.0 + tiny_height / 2.0);
        }
        self.list.push(Card {
            text: decode_entities(raw),
            at: Point::new(cx, cy),
        });
        self.min.x = js::min(self.min.x, cx - width / 2.0);
        self.max.x = js::max(self.max.x, cx + width / 2.0);
        self.min.y = js::min(self.min.y, cy - tiny_height / 2.0);
        self.max.y = js::max(self.max.y, cy + tiny_height / 2.0);
    }
}

/// `turned` draws a diagram that runs across as one that runs down. `tight`
/// gives up the room that keeps a namespace's name clear of an edge coming in
/// beside it.
impl ClassModel {
    fn draw(
        &self,
        config: &ClassConfig,
        ctx: &Ctx<'_, impl Host>,
        turned: bool,
        tight: bool,
    ) -> Rendered {
        let p = crate::PREFIX;
        let model = self;
        let conf = config;
        let graph: ClassGraph = model.graph(conf.hierarchical_namespaces != Some(false));
        let hide_empty = conf.hide_empty_members_box == Some(true);
        let size = ctx.options.font_size;
        let small = (size * 0.875).round();
        let tiny = (size * 0.8).round();
        let tiny_height = (tiny * 1.4).round();
        let measurer = ctx.host;
        let measure = measurer;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let root_dir = if turned {
            Direction::TopToBottom
        } else {
            model.direction
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
        let mut children = vec![0usize; count];
        for node in &graph.nodes {
            if let Some(parent) = node.parent {
                children[parent] += 1;
            }
        }

        // Namespaces that an edge may cross into are made wide enough for the
        // title to sit beside it.
        let mut crossed = vec![false; count];
        for edge in &graph.edges {
            let (Some(source), Some(target)) = (edge.source, edge.target) else {
                continue;
            };
            let a = graph.nodes[source].parent;
            let b = graph.nodes[target].parent;
            if a == b {
                continue;
            }
            for start in [a, b] {
                let mut g = start;
                while let Some(at) = g
                    && !crossed[at]
                {
                    crossed[at] = true;
                    g = graph.nodes[at].parent;
                }
            }
        }

        let mut views: Vec<View> = Vec::with_capacity(count);
        let mut cnodes: Vec<CNode> = Vec::with_capacity(count);
        for (i, node) in graph.nodes.iter().enumerate() {
            let (view, mut c) = if node.is_group {
                let label = Label::layout(Some(node.label), false, measure, size, 4000.0, 0);
                let mut view = View::plain(label, Size::default(), size);
                if children[i] > 0 {
                    let mut c = CNode::new(Size::default(), None);
                    c.is_group = true;
                    c.pad_x = GROUP_PAD;
                    c.pad_top = if view.label.height > 0.0 {
                        view.label.height + 16.0
                    } else {
                        GROUP_PAD
                    };
                    c.pad_bottom = GROUP_PAD;
                    c.min_w = view.label.width + 2.0 * GROUP_PAD - 12.0
                        + if crossed[i] && !tight {
                            (view.label.width + 24.0).min(240.0)
                        } else {
                            0.0
                        };
                    (view, c)
                } else {
                    view.size = Size::new(
                        (view.label.width + 2.0 * GROUP_PAD).max(80.0),
                        view.label.height + 2.0 * GROUP_PAD,
                    );
                    let c = CNode::new(view.size, None);
                    (view, c)
                }
            } else if let Some(cls) = node.class_node {
                let view = View::class(&sizes, &mut styler, cls);
                let c = CNode::new(view.size, None);
                (view, c)
            } else if node.shape == NodeShape::Note {
                let label = Label::layout(Some(node.label), true, measure, small, 260.0, 0);
                let w = (label.width + 2.0 * PAD_X).max(40.0);
                let h = label.height + 2.0 * TITLE_PAD;
                let view = View::plain(label, Size::new(w, h), small);
                let c = CNode::new(view.size, None);
                (view, c)
            } else {
                let label = Label::layout(Some(node.label), false, measure, small, 200.0, 0);
                let w = label.width.max(12.0) + 4.0;
                let h = label.height.max(12.0);
                let view = View::plain(label, Size::new(w, h), small);
                let c = CNode::new(view.size, None);
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
        // Per class: where edges leave, then where they arrive.
        let mut crowd = vec![[Crowd::default(); 2]; views.len()];
        for edge in &graph.edges {
            let (Some(source), Some(target)) = (edge.source, edge.target) else {
                continue;
            };
            let label = Label::layout(edge.label, true, measure, small, 200.0, 0);
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
                cnodes[source].size.w += 2.0
                    * (LOOP
                        + if label.width > 0.0 {
                            label.width + 12.0
                        } else {
                            0.0
                        });
            } else {
                crowd[source][0].meet(start_w);
                crowd[target][1].meet(end_w);
            }
            start_room.push(start_w);
            end_room.push(end_w);
            drawn.push(edge);
            cedges.push(CEdge {
                src: source,
                dst: target,
                minlen: 1,
                label: CEdge::label_room(&label),
                ..CEdge::default()
            });
            edge_labels.push(label);
        }

        if root_dir.is_vertical() {
            for i in 0..views.len() {
                if graph.nodes[i].class_node.is_none() {
                    continue;
                }
                let mut need: f64 = 0.0;
                for Crowd { count, room } in crowd[i] {
                    if count > 1.0 {
                        need = need.max(
                            (count - 1.0) * if room > 0.0 { room + 24.0 } else { 14.0 } + 24.0,
                        );
                    }
                }
                need = need.min(480.0);
                if need > views[i].size.w {
                    cnodes[i].size.w += need - views[i].size.w;
                    views[i].size.w = need;
                }
            }
        }

        let lettered = (0..drawn.len()).any(|i| start_room[i] > 0.0 || end_room[i] > 0.0);
        let layout = compound_layout(
            &mut cnodes,
            &mut cedges,
            root_dir,
            &LayeredOptions {
                node_sep: positive(conf.node_spacing, NODE_SEP),
                edge_sep: EDGE_SEP,
                rank_sep: positive(
                    conf.rank_spacing,
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
        let mut sides: IndexMap<(usize, Side), Vec<End>> = IndexMap::new();
        let mut starts: Vec<Option<End>> = Vec::with_capacity(cedges.len());
        let mut ends: Vec<Option<End>> = Vec::with_capacity(cedges.len());
        for i in 0..cedges.len() {
            let (src, dst) = (cedges[i].src, cedges[i].dst);
            let routed = src != dst && cedges[i].route.len() >= 2;
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
                views[src].size.w,
                src,
                (i, i, start_room[i]),
                &mut sides,
            ));
            let last = route.len() - 1;
            ends.push(attach(
                route,
                last,
                &cnodes[dst],
                views[dst].size.w,
                dst,
                (i, i, end_room[i]),
                &mut sides,
            ));
        }
        let mut gaps: Vec<f64> = Vec::new();
        for (&(item, side), list) in sides.iter_mut() {
            if list.len() < 2 {
                continue;
            }
            let across = if side.is_horizontal() {
                Axis::X
            } else {
                Axis::Y
            };
            let length = views[item].size.along(across);
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
                let mut gap = if across == Axis::Y { 20.0 } else { 24.0 };
                if a.room > 0.0 && a.after {
                    gap += if across == Axis::Y {
                        tiny_height + 6.0
                    } else {
                        a.room
                    };
                }
                if b.room > 0.0 && !b.after {
                    gap += if across == Axis::Y {
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
            let mut at = cnodes[item].center.along(across) - (total * scale) / 2.0;
            for (j, end) in list.iter().enumerate() {
                let route = &mut cedges[end.edge].route;
                let end_at = end.at as isize;
                // An edge that crosses the border of the class's namespace close
                // by runs straight to it: the point where it crosses moves into
                // line with its end.
                let step: isize = if end_at == 0 { 1 } else { -1 };
                let first = end_at + step;
                let first_value = route[first as usize].at.along(across);
                let mut border = None;
                let mut k = first;
                while k >= 0
                    && (k as usize) < route.len()
                    && (route[k as usize].at.along(across) - first_value).abs() < 0.5
                {
                    let p = route[k as usize].at;
                    if ancestors(&cnodes, item).any(|g| cnodes[g].on_border(p)) {
                        border = Some(k);
                        break;
                    }
                    k += step;
                }
                if let Some(border) = border
                    && (first_value - at).abs() <= 24.0
                {
                    let mut k = first;
                    while k != border + step {
                        *route[k as usize].at.along_mut(across) = at;
                        k += step;
                    }
                }
                *route[end.at].at.along_mut(across) = at;
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

        let mut cards = Cards {
            list: Vec::new(),
            min: Point::default(),
            max: Point::new(layout.w, layout.h),
            tiny_height,
        };
        let mut loop_seen: HashMap<usize, usize> = HashMap::new();
        let mut loop_at = vec![0usize; cedges.len()];
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
                let x = c.center.x + view.size.w / 2.0;
                let spread = (view.size.h / 2.0 - 4.0).min(8.0 + k as f64 * 6.0);
                cards.add(
                    edge.start_label_right,
                    start_room[i],
                    Point::new(x, c.center.y - spread),
                    Side::Right,
                    false,
                );
                cards.add(
                    edge.end_label_left,
                    end_room[i],
                    Point::new(x, c.center.y + spread),
                    Side::Right,
                    true,
                );
            } else if route.len() >= 2 {
                let last = route.len() - 1;
                let from = starts[i];
                let to = ends[i];
                cards.add(
                    edge.start_label_right,
                    start_room[i],
                    route[0].at,
                    from.map_or(Side::Bottom, |e| e.side),
                    from.is_none_or(|e| e.after),
                );
                cards.add(
                    edge.end_label_left,
                    end_room[i],
                    route[last].at,
                    to.map_or(Side::Top, |e| e.side),
                    to.is_none_or(|e| e.after),
                );
            }
        }

        let Cards { min, max, .. } = cards;
        let offset = Point::new(pad - min.x, pad - min.y);
        let width = (max.x - min.x + 2.0 * pad).ceil();
        let height = (max.y - min.y + 2.0 * pad).ceil();
        let mut links: Vec<Link> = Vec::new();

        let mut crossings: Crossings = Crossings::new();
        if graph.nodes.first().is_some_and(|node| node.is_group) {
            for e in &cedges {
                crossings.mark(&e.route);
            }
        }

        let mut clusters = String::new();
        let mut struck_titles = String::new();
        let mut nodes_out = String::new();
        let mut member_attrs = String::new();
        let mut method_attrs = String::new();
        for i in 0..views.len() {
            let view = &views[i];
            let c = &cnodes[i];
            let node = &graph.nodes[i];
            let Point { x, y } = c.center + offset;
            let id = EscText(node.id);
            let Size { w, h } = if c.is_group { c.size } else { view.size };
            if node.is_group {
                let title_x = if c.is_group {
                    crossings.title_x(c, view.label.width, GROUP_PAD)
                } else {
                    c.center.x
                };
                let title_y = y - h / 2.0 + 8.0 + view.label.height / 2.0;
                let title_attrs = format!(" class=\"{p}-cluster-label\" fill=\"var(--_m)\"");
                let struck = c.is_group && crossings.crosses_title(c, view.label.width, title_x);
                if struck {
                    let _ = write!(
                        struck_titles,
                        "{}",
                        struck_title(
                            &view.label,
                            Point::new(title_x + offset.x, title_y),
                            &title_attrs,
                            &id,
                            &cnodes,
                            i
                        )
                    );
                }
                let _ = write!(
                    clusters,
                    "<g class=\"{p}-cluster {p}-namespace\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"/>{}</g>",
                    num(x - w / 2.0),
                    num(y - h / 2.0),
                    num(w),
                    num(h),
                    fragment(|f| if struck {
                        Ok(())
                    } else {
                        write!(
                            f,
                            "{}",
                            view.label
                                .svg(Point::new(title_x + offset.x, title_y), &title_attrs)
                        )
                    })
                );
                continue;
            }
            let at = fragment(|f| write!(f, " transform=\"translate({},{})\"", num(x), num(y)));
            if let Some(cls) = node.class_node {
                let _ = write!(
                    nodes_out,
                    "{}",
                    view.class_svg(cls, id, at, &sizes, &mut links, ctx)
                );
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
                    view.label.svg(
                        Point::new(0.0, 0.0),
                        rewrite(
                            &mut member_attrs,
                            format_args!(
                                " class=\"{p}-label\" font-size=\"{}\"",
                                js::Number(small)
                            )
                        )
                    )
                );
            } else {
                let _ = write!(
                    nodes_out,
                    "<g class=\"{p}-node {p}-interface\" data-id=\"{id}\"{at}>{}</g>",
                    view.label.svg(
                        Point::new(0.0, 0.0),
                        rewrite(
                            &mut method_attrs,
                            format_args!(
                                " class=\"{p}-label\" font-size=\"{}\"",
                                js::Number(small)
                            )
                        )
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
            let start_trim = start_type.map_or(0.0, RelationType::trim);
            let end_trim = end_type.map_or(0.0, RelationType::trim);
            let ce = &mut cedges[i];
            let path: EdgePath = if ce.src == ce.dst {
                let c = &cnodes[ce.src];
                let view = &views[ce.src];
                let k = loop_at[i] as f64;
                let x = c.center.x + offset.x + view.size.w / 2.0;
                let y = c.center.y + offset.y;
                let spread = (view.size.h / 2.0 - 4.0).min(8.0 + k * 6.0);
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
                    start: Point::new(x, y - spread),
                    start_dir: Point::new(-1.0, 0.0),
                    end: Point::new(x, y + spread),
                    end_dir: Point::new(-1.0, 0.0),
                };
                ce.label_at = Point::new(
                    c.center.x + view.size.w / 2.0 + reach + 4.0 + ce.label.w / 2.0,
                    c.center.y,
                );
                path
            } else {
                for point in &mut ce.route {
                    point.at += offset;
                }
                route_path(&ce.route, Curve::Smooth, start_trim, end_trim, None, None)
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
                EscText(&edge.id),
                path.d,
                fragment(|f| match start_type {
                    Some(kind) => write!(f, "{}", kind.draw(path.start, path.start_dir, LINE)),
                    None => Ok(()),
                }),
                fragment(|f| match end_type {
                    Some(kind) => write!(f, "{}", kind.draw(path.end, path.end_dir, LINE)),
                    None => Ok(()),
                })
            );
            if label.width > 0.0 {
                let _ = write!(
                    labels_out,
                    "{}",
                    edge_label_svg(&edge.id, label, ce.label_at + offset, "")
                );
            }
        }

        let mut cards_out = String::new();
        for item in &cards.list {
            let _ = write!(
                cards_out,
                "<text class=\"{p}-cardinality\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                num(item.at.x + offset.x),
                num(item.at.y + offset.y + tiny * 0.35),
                Esc(&item.text)
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
                js::Number(small)
            );
        }
        if !cards_out.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-cardinalities\" font-size=\"{}\" fill=\"var(--_m)\">{cards_out}</g>",
                js::Number(tiny)
            );
        }
        let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");
        let titled = with_title(
            model.titles.title.as_deref(),
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
            model.titles.accessible(),
            &titled.content,
        );

        Rendered {
            svg,
            width: titled.width,
            height: titled.height,
            links,
        }
    }
}

impl View {
    fn class_svg<'a, H: Host>(
        &'a self,
        cls: &'a ClassNode,
        id: impl fmt::Display + 'a,
        at: impl fmt::Display + 'a,
        sizes: &'a Sizes<'_, H>,
        links: &mut Vec<Link>,
        ctx: &'a Ctx<'_, impl Host>,
    ) -> impl fmt::Display + 'a {
        let view = self;
        let p = crate::PREFIX;
        let (Size { w, h }, style) = (view.size, &view.style);
        let tiny = sizes.tiny;
        let tiny_height = sizes.tiny_height;
        let left = -w / 2.0;
        let top = -h / 2.0;
        let member_size = (view.font_size * 0.875).round();
        let line_height = (member_size * 1.5).round();
        let first = top + view.title_h;
        let second = first + view.attributes_h;
        let title_attrs = format!(
            " class=\"{p}-label\"{} font-weight=\"var(--_hw)\"{}",
            fragment(|f| if view.font_size == sizes.size {
                Ok(())
            } else {
                write!(f, " font-size=\"{}\"", num(view.font_size))
            }),
            style.text
        );
        let lines = move |f: &mut fmt::Formatter<'_>, list: &[Line], kind: &str, from: f64| {
            let mut baseline = from + SECTION_PAD + line_height / 2.0 + member_size * 0.35;
            for line in list {
                write!(f, "<text class=\"{p}-member {p}-{kind}")?;
                match line.classifier.as_str() {
                    "$" if style.text.is_empty() => {
                        write!(f, " {p}-static\" style=\"text-decoration:underline\"")?
                    }
                    "$" => write!(
                        f,
                        " {p}-static\"{}text-decoration:underline\"",
                        &style.text[..style.text.len() - 1]
                    )?,
                    "*" => write!(f, " {p}-abstract\" font-style=\"italic\"{}", style.text)?,
                    _ => write!(f, "\"{}", style.text)?,
                }
                write!(
                    f,
                    " x=\"{}\" y=\"{}\">{}</text>",
                    num(left + PAD_X),
                    num(baseline),
                    Esc(&line.text)
                )?;
                baseline += line_height;
            }
            Ok(())
        };
        let body = fragment(move |f| {
            if let Some(tooltip) = cls.tooltip.as_deref().filter(|tip| !tip.is_empty()) {
                write!(f, "<title>{}</title>", EscText(tooltip))?;
            }
            write!(
                f,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
                num(left),
                num(top),
                num(w),
                num(h),
                style.shape
            )?;
            if h > view.title_h {
                write!(
                    f,
                    "<path class=\"{p}-divider\" d=\"M{},{}H{}M{},{}H{}\" fill=\"none\" stroke=\"var(--_b)\"",
                    num(left),
                    num(first),
                    num(-left),
                    num(left),
                    num(second),
                    num(-left)
                )?;
                if let Some(stroke) = style.stroke.as_deref().filter(|stroke| !stroke.is_empty()) {
                    write!(f, " style=\"stroke:{}\"", Esc(stroke))?;
                }
                f.write_str("/>")?;
            }
            let mut y = top + TITLE_PAD;
            for annotation in &view.annotations {
                write!(
                    f,
                    "<text class=\"{p}-annotation\" x=\"0\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
                    num(y + tiny_height / 2.0 + tiny * 0.35),
                    js::Number(tiny),
                    Esc(annotation)
                )?;
                y += tiny_height;
            }
            write!(
                f,
                "{}",
                view.label
                    .svg(Point::new(0.0, (y + first - TITLE_PAD) / 2.0), &title_attrs)
            )?;
            if !view.attributes.is_empty() || !view.methods.is_empty() {
                write!(
                    f,
                    "<g class=\"{p}-members\" font-size=\"{}\">",
                    js::Number(member_size)
                )?;
                lines(f, &view.attributes, "attribute", first)?;
                lines(f, &view.methods, "method", second)?;
                f.write_str("</g>")?;
            }
            Ok(())
        });
        let target = cls
            .link_target
            .as_deref()
            .filter(|target| TARGETS.contains(target));
        let target = fragment(move |f| match target {
            Some(target) => write!(f, " target=\"{target}\""),
            None => Ok(()),
        });
        let url = cls.link.as_deref().filter(|link| !link.is_empty());
        let body = linked(body, url, &cls.id, links, ctx, target);
        let classes = node_class_names(&cls.css_classes);
        fragment(move |f| {
            write!(
                f,
                "<g class=\"{p}-node {p}-class{classes}\" data-id=\"{id}\"{at}>"
            )?;
            write!(f, "{body}</g>")
        })
    }
}
