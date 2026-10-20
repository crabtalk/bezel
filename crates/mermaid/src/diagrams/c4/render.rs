//! Pele's `diagrams/c4/render.ts`.

use crate::geom::{Point, Size};
use crate::svg::builder::rewrite;
use crate::svg::edges::Marker;
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Write as _},
};

use super::{
    db::C4Model,
    layout::{HEAD_LEFT, HEAD_TOP},
    route::{C4Box, HEAD, LANE, Lanes, crosses, cyl_ry, queue_rx, route},
    types::{C4Ref, C4Type},
};
use crate::{
    Config, Error, Kind, Link, Rendered,
    diagrams::Render,
    js::{self, Lazy, max, min},
    svg::{
        builder::{EscText, fragment, num},
        edges::ARROW,
        root::svg_document,
        shapes::Shape,
        theme::{RADIUS, ResolvedStyle, resolve_style},
    },
    text::{Label, Style},
    util::url::{linked, sanitize_url},
};
use crate::{Ctx, Host};

static RE_HASH: Lazy = Lazy::new(r"\ufb02\u00b0\u00b0?", "g");
static RE_SEMI: Lazy = Lazy::new(r"\u00b6\u00df", "g");

const PAIR_SHIFT: f64 = 7.0;
const NARROW_GAP: f64 = 36.0;
const WIDEN: f64 = 24.0;
const MAX_OFFSET: f64 = 2000.0;
/// A label may cover this much of a shape's edge, where there is no text.
const EDGE: f64 = 8.0;
/// Places along a relation to try for its label, nearest the middle first.
const SLIDE: [f64; 7] = [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8];
/// Above this many box comparisons, labels stay at the middle and lines are
/// not steered around shapes.
const CARE_BUDGET: f64 = 2000000.0;

fn user_style(fill: Option<&str>, stroke: Option<&str>, color: Option<&str>) -> ResolvedStyle {
    let mut declarations: Vec<String> = Vec::new();
    if let Some(fill) = fill.filter(|v| !v.is_empty()) {
        declarations.push(format!("fill:{fill}"));
    }
    if let Some(stroke) = stroke.filter(|v| !v.is_empty()) {
        declarations.push(format!("stroke:{stroke}"));
    }
    if let Some(color) = color.filter(|v| !v.is_empty()) {
        declarations.push(format!("color:{color}"));
    }
    resolve_style(&declarations)
}

/// Writes `body`, inside a link to `url` where there is one.
fn link(
    out: &mut String,
    body: impl fmt::Display,
    url: Option<&str>,
    id: &str,
    links: &mut Vec<Link>,
    ctx: &Ctx<'_, impl Host>,
) {
    let Some(url) = url.filter(|url| !url.is_empty()) else {
        let _ = write!(out, "{body}");
        return;
    };
    // Entity encoding has hidden any `#name;` in the address. Put it back, so
    // that the check sees what a browser would.
    let url = if url.contains("\u{b6}\u{df}") {
        RE_SEMI.replace(&RE_HASH.replace(url, |_| "#".into()), |_| ";".into())
    } else {
        url.to_string()
    };
    let _ = write!(
        out,
        "{}",
        linked(body, Some(&sanitize_url(&url)), id, links, ctx, "")
    );
}

/// A relation's label where it was placed.
struct Placed {
    rel: usize,
    label: Label,
    techn: Label,
    style: ResolvedStyle,
    /// The middle of its top edge.
    at: Point,
    size: Size,
}

impl Render for C4Model {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        ctx.try_tighten(|tight| {
            if !tight {
                return Ok(self.draw(config, ctx, false, NARROW_GAP)?.0);
            }
            let (mut best, mut squeezed) = self.draw(config, ctx, true, NARROW_GAP)?;
            let mut gap = NARROW_GAP + WIDEN;
            while squeezed > 0 && gap <= NARROW_GAP + 3.0 * WIDEN {
                let (wider, left) = self.draw(config, ctx, true, gap)?;
                if left < squeezed {
                    best = wider;
                    squeezed = left;
                }
                gap += WIDEN;
            }
            Ok(best)
        })
    }
}

/// A rectangle, by its top-left and bottom-right corners.
type Area = (Point, Point);

/// The extent of what is drawn.
struct Bounds {
    min: Point,
    max: Point,
}

impl Bounds {
    fn grow(&mut self, (lo, hi): Area) {
        self.min = Point::new(min(self.min.x, lo.x), min(self.min.y, lo.y));
        self.max = Point::new(max(self.max.x, hi.x), max(self.max.y, hi.y));
    }
}

/// How much of `area` the areas in `taken` cover.
fn covered(taken: &[Area], (lo, hi): Area) -> f64 {
    let mut area = 0.0;
    for (t0, t1) in taken {
        let w = min(hi.x, t1.x) - max(lo.x, t0.x);
        if w <= 0.0 {
            continue;
        }
        let h = min(hi.y, t1.y) - max(lo.y, t0.y);
        if h > 0.0 {
            area += w * h;
        }
    }
    area
}

/// The square of side `2 * ARROW` around `p`.
fn around(p: Point) -> Area {
    (
        Point::new(p.x - ARROW, p.y - ARROW),
        Point::new(p.x + ARROW, p.y + ARROW),
    )
}

fn offset(value: Option<f64>) -> f64 {
    match value {
        Some(value) if js::truthy(value) => max(-MAX_OFFSET, min(MAX_OFFSET, value)),
        _ => 0.0,
    }
}

impl C4Model {
    fn draw(
        &self,
        _config: &Config,
        ctx: &Ctx<'_, impl Host>,
        tight: bool,
        gap: f64,
    ) -> Result<(Rendered, usize), Error> {
        let db = self;
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = (size * 0.8125).round();
        let measurer = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let room = tight.then(|| max(80.0, ctx.options.max_width.unwrap_or(320.0) - 2.0 * pad));
        let mut layout = db.layout(measurer, size, small, room, gap);
        let root = layout.order[0];

        let root_size = layout.groups[root].bx.size;
        let mut bounds = Bounds {
            min: Point::default(),
            max: Point::new(root_size.w, root_size.h),
        };

        let mut ends: Vec<C4Box> = Vec::new();
        let mut pairs: HashMap<C4Ref, HashSet<C4Ref>> = HashMap::new();
        for rel in &db.rels {
            let from = if rel.from_attr.is_some() {
                None
            } else {
                db.get_c4_shape(&rel.from)
            };
            let to = if rel.to_attr.is_some() {
                None
            } else {
                db.get_c4_shape(&rel.to)
            };
            let a = from.and_then(|from| layout.boxes.get(&from));
            let b = to.and_then(|to| layout.boxes.get(&to));
            let (Some(&a), Some(&b)) = (a, b) else {
                return Err(Error::semantic(format!(
                    "C4 rel \"{}\" -> \"{}\" references an unknown shape or boundary",
                    rel.from, rel.to
                ))
                .of(Kind::C4));
            };
            ends.push(a);
            ends.push(b);
            pairs.entry(a.id).or_default().insert(b.id);
        }

        // What a relation's label should not cover: shapes, arrowheads, boundary
        // headings, and the labels placed before it.
        let mut nodes: Vec<C4Box> = Vec::new();
        let mut taken: Vec<Area> = Vec::new();
        for &at in &layout.order {
            for node in &layout.groups[at].nodes {
                let b = node.bx;
                nodes.push(b);
                taken.push((
                    Point::new(b.at.x + EDGE, b.at.y + EDGE),
                    Point::new(b.right() - EDGE, b.bottom() - EDGE),
                ));
            }
        }
        let rels = db.rels.len() as f64;
        let careful =
            rels * (nodes.len() as f64 + layout.order.len() as f64 + 3.0 * rels) <= CARE_BUDGET;

        // Every line is routed before any label is placed, so that labels can
        // keep off the arrowheads.
        let trim = Marker::Arrow.trim();
        let mut lanes = Lanes::default();
        for &at in &layout.order {
            if at != root {
                lanes.frame(&layout.groups[at].bx);
            }
        }
        let mut routes = Vec::with_capacity(db.rels.len());
        let mut line_styles = Vec::with_capacity(db.rels.len());
        for (index, rel) in db.rels.iter().enumerate() {
            let a = ends[index * 2];
            let b = ends[index * 2 + 1];
            let start_arrow = rel.ty.arrow_at_start();
            let end_arrow = rel.ty.arrow_at_end();
            let reverse = a.id != b.id && pairs.get(&b.id).is_some_and(|set| set.contains(&a.id));
            let r = route(
                &a,
                &b,
                if reverse { PAIR_SHIFT } else { 0.0 },
                careful.then_some(nodes.as_slice()),
                &mut lanes,
                if start_arrow { trim } else { 0.0 },
                if end_arrow { trim } else { 0.0 },
            );
            let (a0, a1) = r.along;
            bounds.grow((
                Point::new(min(a0.x, a1.x), min(a0.y, a1.y)),
                Point::new(max(a0.x, a1.x), max(a0.y, a1.y)),
            ));
            if start_arrow && careful {
                taken.push(around(r.path.start));
            }
            if end_arrow && careful {
                taken.push(around(r.path.end));
            }
            routes.push(r);
            line_styles.push(user_style(None, rel.line_color.as_deref(), None));
        }

        // A boundary's heading goes at the left, the middle or the right of its
        // top edge: the first of them no line crosses.
        for &at in &layout.order {
            let group = &mut layout.groups[at];
            if !careful || group.head_w == 0.0 {
                continue;
            }
            let top = group.bx.at.y + HEAD_TOP;
            let bottom = group.bx.at.y + group.head - LANE;
            let mut fewest = f64::INFINITY;
            for x in [
                HEAD_LEFT,
                (group.bx.size.w - group.head_w) / 2.0,
                group.bx.size.w - HEAD_LEFT - group.head_w,
            ] {
                let mut count = 0.0;
                for r in &routes {
                    if crosses(
                        &r.points,
                        Point::new(group.bx.at.x + x - 4.0, top),
                        Point::new(group.bx.at.x + x + group.head_w + 4.0, bottom),
                    ) {
                        count += 1.0;
                    }
                }
                if count < fewest {
                    fewest = count;
                    group.head_x = x;
                }
                if count == 0.0 {
                    break;
                }
            }
            taken.push((
                Point::new(group.bx.at.x + group.head_x - 4.0, top),
                Point::new(group.bx.at.x + group.head_x + group.head_w + 4.0, bottom),
            ));
        }

        let mut placed: Vec<Placed> = Vec::new();
        for (index, rel) in db.rels.iter().enumerate() {
            let r = &routes[index];
            let style = user_style(None, None, rel.text_color.as_deref());
            let numbered;
            let text = if db.c4_type == Some(C4Type::Dynamic) {
                numbered = format!("{}: {}", index + 1, rel.label);
                &numbered
            } else {
                &rel.label
            };
            let label = Label::layout(Some(text), false, measurer, small, r.room, 0);
            let techn_text = rel
                .techn
                .as_deref()
                .filter(|techn| !techn.is_empty())
                .map(|techn| format!("[{techn}]"));
            let techn = Label::layout(
                techn_text.as_deref(),
                false,
                measurer,
                small,
                r.room,
                Style::ITALIC,
            );
            let lw = max(label.width, techn.width) + 8.0;
            let lh = label.height + techn.height;
            if lh == 0.0 {
                continue;
            }

            // The label sits on the line, or just beside it when another relation
            // runs the opposite way.
            let (n, (a0, a1)) = (r.normal, r.along);
            let beside = n.x.abs() * (lw / 2.0) + n.y.abs() * (lh / 2.0) + 2.0;
            let aside = if js::truthy(r.lean) {
                max(0.0, min(r.lean, beside - 14.0))
            } else {
                beside
            };
            // Manual offsets are tuned to the original spacing. Reposition compact
            // labels beside their routed arrows so an offset cannot leave them
            // floating outside a boundary.
            let mut dx = n.x * aside + if tight { 0.0 } else { offset(rel.offset_x) };
            let mut dy = n.y * aside + if tight { 0.0 } else { offset(rel.offset_y) };
            let mut at = 0.5;
            if careful {
                // Slide along the line to the spot that covers least. Offsets from
                // the diagram are kept unless a spot on the line itself covers
                // less than any they lead to: they are often tuned for Mermaid's
                // spacing.
                let mut least = f64::INFINITY;
                let moved = dx != n.x * aside || dy != n.y * aside;
                let passes = if moved { 2 } else { 1 };
                let mut pass = 0;
                while pass < passes && least > 0.0 {
                    let px = if pass > 0 { n.x * aside } else { dx };
                    let py = if pass > 0 { n.y * aside } else { dy };
                    for t in SLIDE {
                        let cx = a0.x + (a1.x - a0.x) * t + px;
                        let cy = a0.y + (a1.y - a0.y) * t + py;
                        let area = covered(
                            &taken,
                            (
                                Point::new(cx - lw / 2.0, cy - lh / 2.0),
                                Point::new(cx + lw / 2.0, cy + lh / 2.0),
                            ),
                        );
                        if area < least {
                            least = area;
                            at = t;
                            dx = px;
                            dy = py;
                        }
                        if area == 0.0 {
                            break;
                        }
                    }
                    pass += 1;
                }
            }
            let cx = a0.x + (a1.x - a0.x) * at + dx;
            let top = a0.y + (a1.y - a0.y) * at + dy - lh / 2.0;
            let area = (
                Point::new(cx - lw / 2.0, top),
                Point::new(cx + lw / 2.0, top + lh),
            );
            if careful {
                taken.push(area);
            }
            bounds.grow(area);
            placed.push(Placed {
                rel: index,
                label,
                techn,
                style,
                at: Point::new(cx, top),
                size: Size::new(lw, lh),
            });
        }

        let title = Label::layout(
            db.titles.title.as_deref(),
            false,
            measurer,
            size,
            if tight { ctx.title_room() } else { 4000.0 },
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);
        let Bounds {
            min: Point { x: min_x, y: min_y },
            max: Point { x: max_x, y: max_y },
        } = bounds;
        let inner = max(max_x - min_x, title.width);
        let width = (inner + 2.0 * pad).ceil();
        let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
        let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
        let oy = pad + title_height - min_y;

        let mut content = String::new();
        let _ = write!(
            content,
            "{}<g transform=\"translate({},{})\">",
            title.svg(
                Point::new(width / 2.0, pad + title.height / 2.0),
                crate::svg::builder::TITLE
            ),
            num(ox),
            num(oy)
        );

        let small_str = js::Number(small);
        let mut links: Vec<Link> = Vec::new();
        let mut nodes_out = String::new();
        let mut shape_svg = String::new();
        let mut clusters = false;
        let mut label_attrs_buf = String::new();
        let mut type_attrs_buf = String::new();
        let mut descr_attrs_buf = String::new();
        for &at in &layout.order {
            let group = &layout.groups[at];
            if at != root {
                if !clusters {
                    let _ = write!(content, "<g class=\"{p}-clusters\">");
                    clusters = true;
                }
                let boundary = &db.boundaries[group.boundary];
                let style = user_style(
                    boundary.bg_color.as_deref(),
                    boundary.border_color.as_deref(),
                    boundary.font_color.as_deref(),
                );
                let x = group.bx.at.x + group.head_x;
                let y = group.bx.at.y + HEAD_TOP;
                let node = boundary.node_type.is_some();
                let _ = write!(
                    content,
                    "<g class=\"{p}-cluster {p}-c4-{}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"none\" stroke=\"var(--_b)\"{}{}/>",
                    if node { "node" } else { "boundary" },
                    EscText(&boundary.alias),
                    num(group.bx.at.x),
                    num(group.bx.at.y),
                    num(group.bx.size.w),
                    num(group.bx.size.h),
                    if node {
                        ""
                    } else {
                        " stroke-dasharray=\"6 4\""
                    },
                    style.shape
                );
                let label_attrs = rewrite(
                    &mut label_attrs_buf,
                    format_args!(
                        " class=\"{p}-cluster-label\" font-weight=\"var(--_hw)\"{}",
                        style.text
                    ),
                );
                let type_attrs = rewrite(
                    &mut type_attrs_buf,
                    format_args!(
                        " class=\"{p}-c4-type\" font-size=\"{small_str}\" fill=\"var(--_m)\"{}",
                        style.text
                    ),
                );
                let descr_attrs = rewrite(
                    &mut descr_attrs_buf,
                    format_args!(
                        " class=\"{p}-c4-descr\" font-size=\"{small_str}\" fill=\"var(--_m)\"{}",
                        style.text
                    ),
                );
                let (name, kind, descr) = (&group.labels[0], &group.labels[1], &group.labels[2]);
                let text = fragment(|f| {
                    write!(
                        f,
                        "{}{}{}",
                        name.start_lines(Point::new(x, y), label_attrs),
                        kind.start_lines(Point::new(x, y + name.height), type_attrs),
                        descr
                            .start_lines(Point::new(x, y + name.height + kind.height), descr_attrs)
                    )
                });
                link(
                    &mut content,
                    text,
                    boundary.link.as_deref(),
                    &boundary.alias,
                    &mut links,
                    ctx,
                );
                content.push_str("</g>");
            }
            let mut muted_buf = String::new();
            let mut line_attrs = String::new();
            let mut shape_attrs = String::new();
            for node in &group.nodes {
                let shape = &db.shapes[node.shape];
                let kind = shape.kind.map_or(String::new(), |kind| kind.to_string());
                let external = shape.kind.is_some_and(|kind| kind.external);
                let style = user_style(
                    shape.bg_color.as_deref(),
                    shape.border_color.as_deref(),
                    shape.font_color.as_deref(),
                );
                let stroke = if external {
                    " stroke=\"var(--_b)\" stroke-dasharray=\"4 3\""
                } else {
                    " stroke=\"var(--_b)\""
                };
                let C4Box {
                    size: Size { w, h },
                    form,
                    ..
                } = node.bx;
                let labels = &node.labels;
                let dx = if form == Shape::HorizontalCylinder {
                    -queue_rx(h) / 2.0
                } else {
                    0.0
                };
                let dy = match form {
                    Shape::Person => HEAD / 2.0,
                    Shape::Cylinder => cyl_ry(w) / 2.0,
                    _ => 0.0,
                };
                let _ = write!(
                    nodes_out,
                    "<g class=\"{p}-node {p}-shape-{} {p}-c4-{kind}{}\" data-id=\"{}\" transform=\"translate({},{})\">",
                    form.name(),
                    fragment(|f| if external {
                        write!(f, " {p}-c4-external")
                    } else {
                        Ok(())
                    }),
                    EscText(&shape.alias),
                    num(node.bx.at.x + w / 2.0),
                    num(node.bx.at.y + h / 2.0)
                );
                let muted = rewrite(
                    &mut muted_buf,
                    format_args!(" fill=\"var(--_m)\"{}", style.text),
                );
                let attrs = [
                    format!(
                        " class=\"{p}-c4-type\" font-size=\"{}\"{muted}",
                        js::Number(small - 1.0)
                    ),
                    format!(
                        " class=\"{p}-label\" font-weight=\"var(--_hw)\"{}",
                        style.text
                    ),
                    format!(" class=\"{p}-c4-techn\" font-size=\"{small_str}\"{muted}"),
                    format!(
                        " class=\"{p}-c4-descr\" font-size=\"{small_str}\"{}",
                        style.text
                    ),
                ];
                shape_svg.clear();
                {
                    form.draw(
                        &mut shape_svg,
                        Size::new(w, h),
                        rewrite(
                            &mut shape_attrs,
                            format_args!(
                                " fill=\"var({})\"{stroke}{}",
                                if external { "--_bg" } else { "--_s" },
                                style.shape
                            ),
                        ),
                        rewrite(&mut line_attrs, format_args!("{stroke}{}", style.line)),
                    );
                }
                let body = fragment(|f| {
                    f.write_str(&shape_svg)?;
                    let mut y = dy - node.text_h / 2.0;
                    for (i, (label, attrs)) in labels.iter().zip(&attrs).enumerate() {
                        write!(
                            f,
                            "{}",
                            label.svg(Point::new(dx, y + label.height / 2.0), attrs)
                        )?;
                        y += label.height + if i == 2 { 4.0 } else { 0.0 };
                    }
                    Ok(())
                });
                link(
                    &mut nodes_out,
                    body,
                    shape.link.as_deref(),
                    &shape.alias,
                    &mut links,
                    ctx,
                );
                nodes_out.push_str("</g>");
            }
        }
        if clusters {
            content.push_str("</g>");
        }

        if !db.rels.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">"
            );
            for ((rel, r), style) in db.rels.iter().zip(&routes).zip(&line_styles) {
                let color = style.line_color();
                let _ = write!(
                    content,
                    "<g class=\"{p}-edge\" data-id=\"{}-{}\"{}><path d=\"{}\"/>",
                    EscText(&rel.from),
                    EscText(&rel.to),
                    style.line,
                    r.path.d
                );
                if rel.ty.arrow_at_start() {
                    let _ = write!(
                        content,
                        "{}",
                        Marker::Arrow.draw(r.path.start, r.path.start_dir, &color)
                    );
                }
                if rel.ty.arrow_at_end() {
                    let _ = write!(
                        content,
                        "{}",
                        Marker::Arrow.draw(r.path.end, r.path.end_dir, &color)
                    );
                }
                content.push_str("</g>");
            }
            content.push_str("</g>");
        }
        if !nodes_out.is_empty() {
            let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");
        }
        // Links are listed with the labels' first.
        let mut label_links: Vec<Link> = Vec::new();
        if !placed.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-edge-labels\" font-size=\"{small_str}\">"
            );
            let mut id_buf = String::new();
            let mut techn_attrs_buf = String::new();
            for Placed {
                rel,
                label,
                techn,
                style,
                at,
                size: Size { w: lw, h: lh },
            } in &placed
            {
                let rel = &db.rels[*rel];
                let id = rewrite(&mut id_buf, format_args!("{}-{}", rel.from, rel.to));
                let Point { x: cx, y: top } = *at;
                let _ = write!(
                    content,
                    "<g class=\"{p}-edge-label\" data-id=\"{}\">",
                    EscText(id)
                );
                if let Some(descr) = rel.descr.as_deref().filter(|descr| !descr.is_empty()) {
                    let _ = write!(content, "<title>{}</title>", EscText(descr));
                }
                let _ = write!(
                    content,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>",
                    num(cx - lw / 2.0),
                    num(top),
                    num(*lw),
                    num(*lh)
                );
                let techn_attrs = rewrite(
                    &mut techn_attrs_buf,
                    format_args!(" class=\"{p}-c4-techn\" fill=\"var(--_m)\"{}", style.text),
                );
                let body = fragment(|f| {
                    write!(
                        f,
                        "{}{}",
                        label.svg(Point::new(cx, top + label.height / 2.0), &style.text),
                        techn.svg(
                            Point::new(cx, top + label.height + techn.height / 2.0),
                            techn_attrs
                        )
                    )
                });
                link(
                    &mut content,
                    body,
                    rel.link.as_deref(),
                    id,
                    &mut label_links,
                    ctx,
                );
                content.push_str("</g>");
            }
            content.push_str("</g>");
        }
        content.push_str("</g>");
        label_links.append(&mut links);
        let links = label_links;
        let svg = svg_document(
            "c4",
            width,
            height,
            size,
            ctx,
            db.titles.accessible(),
            &content,
        );
        Ok((
            Rendered {
                svg,
                width,
                height,
                links,
            },
            lanes.squeezed,
        ))
    }
}
