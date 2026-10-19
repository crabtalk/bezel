//! Pele's `diagrams/c4/render.ts`.

use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
};

use super::{
    db::C4Model,
    layout::{HEAD_LEFT, HEAD_TOP, layout_c4},
    route::{C4Box, HEAD, LANE, Lanes, crosses, cyl_ry, queue_rx, route},
    types::C4Ref,
};
use crate::{
    Config, Error, Kind, Link, Options, Rendered,
    diagrams::common::{
        fit_width::{title_room, try_tighten},
        literal_label::left_label_svg,
    },
    js::{self, Lazy, max, min},
    svg::{
        builder::{esc, esc_text, label_svg, num},
        edges::{ARROW, marker, marker_trim},
        root::{Accessible, svg_document},
        shapes::draw_shape,
        theme::{RADIUS, ResolvedStyle, resolve_style},
    },
    text::{Style, layout_label},
    util::url::{linked, sanitize_url},
};

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

fn link(
    body: &str,
    url: Option<&str>,
    id: &str,
    links: &mut Vec<Link>,
    options: &Options,
) -> String {
    let Some(url) = url.filter(|url| !url.is_empty()) else {
        return body.to_string();
    };
    // Entity encoding has hidden any `#name;` in the address. Put it back, so
    // that the check sees what a browser would.
    let url = if url.contains("\u{b6}\u{df}") {
        RE_SEMI.replace(&RE_HASH.replace(url, |_| "#".into()), |_| ";".into())
    } else {
        url.to_string()
    };
    linked(body, &sanitize_url(&url), id, links, options, "")
}

pub fn render_c4(db: &C4Model, config: &Config, options: &Options) -> Result<Rendered, Error> {
    try_tighten(options, |tight| {
        if !tight {
            return Ok(draw(db, config, options, false, NARROW_GAP)?.0);
        }
        let (mut best, mut squeezed) = draw(db, config, options, true, NARROW_GAP)?;
        let mut gap = NARROW_GAP + WIDEN;
        while squeezed > 0 && gap <= NARROW_GAP + 3.0 * WIDEN {
            let (wider, left) = draw(db, config, options, true, gap)?;
            if left < squeezed {
                best = wider;
                squeezed = left;
            }
            gap += WIDEN;
        }
        Ok(best)
    })
}

/// The extent of what is drawn.
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn grow(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.min_x = min(self.min_x, x0);
        self.min_y = min(self.min_y, y0);
        self.max_x = max(self.max_x, x1);
        self.max_y = max(self.max_y, y1);
    }
}

fn covered(taken: &[f64], x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let mut area = 0.0;
    for t in taken.as_chunks::<4>().0 {
        let w = min(x1, t[2]) - max(x0, t[0]);
        if w <= 0.0 {
            continue;
        }
        let h = min(y1, t[3]) - max(y0, t[1]);
        if h > 0.0 {
            area += w * h;
        }
    }
    area
}

fn offset(value: Option<f64>) -> f64 {
    match value {
        Some(value) if js::truthy(value) => max(-MAX_OFFSET, min(MAX_OFFSET, value)),
        _ => 0.0,
    }
}

fn draw(
    db: &C4Model,
    _config: &Config,
    options: &Options,
    tight: bool,
    gap: f64,
) -> Result<(Rendered, usize), Error> {
    let p = crate::PREFIX;
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.8125);
    let measurer = options.measurer();
    let measurer = &*measurer;
    let pad = options.padding.unwrap_or(8.0);
    let mut links: Vec<Link> = Vec::new();
    let room = tight.then(|| max(80.0, options.max_width.unwrap_or(320.0) - 2.0 * pad));
    let mut layout = layout_c4(db, measurer, size, small, room, gap);
    let root = layout.order[0];

    let mut bounds = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: layout.groups[root].bx.w,
        max_y: layout.groups[root].bx.h,
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
    let mut taken: Vec<f64> = Vec::new();
    for &at in &layout.order {
        for node in &layout.groups[at].nodes {
            let b = node.bx;
            nodes.push(b);
            taken.extend([b.x + EDGE, b.y + EDGE, b.x + b.w - EDGE, b.y + b.h - EDGE]);
        }
    }
    let rels = db.rels.len() as f64;
    let careful =
        rels * (nodes.len() as f64 + layout.order.len() as f64 + 3.0 * rels) <= CARE_BUDGET;

    // Every line is drawn before any label is placed, so that labels can keep
    // off the arrowheads.
    let mut edges_out = String::new();
    let trim = marker_trim("arrow_point");
    let mut lanes = Lanes::default();
    for &at in &layout.order {
        if at != root {
            lanes.frame(&layout.groups[at].bx);
        }
    }
    let mut routes = Vec::with_capacity(db.rels.len());
    for (index, rel) in db.rels.iter().enumerate() {
        let a = ends[index * 2];
        let b = ends[index * 2 + 1];
        let style = user_style(None, rel.line_color.as_deref(), None);
        let color = style
            .stroke
            .as_deref()
            .filter(|stroke| !stroke.is_empty())
            .map_or_else(|| "var(--_l)".to_string(), esc);
        let start_arrow = rel.ty == "birel" || rel.ty == "rel_b";
        let end_arrow = rel.ty != "rel_b";
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
        bounds.grow(
            min(r.x0, r.x1),
            min(r.y0, r.y1),
            max(r.x0, r.x1),
            max(r.y0, r.y1),
        );
        if start_arrow && careful {
            taken.extend([r.sx - ARROW, r.sy - ARROW, r.sx + ARROW, r.sy + ARROW]);
        }
        if end_arrow && careful {
            taken.extend([r.ex - ARROW, r.ey - ARROW, r.ex + ARROW, r.ey + ARROW]);
        }
        let _ = write!(
            edges_out,
            "<g class=\"{p}-edge\" data-id=\"{}\"{}><path d=\"{}\"/>",
            esc_text(&format!("{}-{}", rel.from, rel.to)),
            style.line,
            r.d
        );
        if start_arrow {
            edges_out += &marker("arrow_point", r.sx, r.sy, r.sdx, r.sdy, &color);
        }
        if end_arrow {
            edges_out += &marker("arrow_point", r.ex, r.ey, r.edx, r.edy, &color);
        }
        edges_out += "</g>";
        routes.push(r);
    }

    // A boundary's heading goes at the left, the middle or the right of its
    // top edge: the first of them no line crosses.
    for &at in &layout.order {
        let group = &mut layout.groups[at];
        if !careful || group.head_w == 0.0 {
            continue;
        }
        let top = group.bx.y + HEAD_TOP;
        let bottom = group.bx.y + group.head - LANE;
        let mut fewest = f64::INFINITY;
        for x in [
            HEAD_LEFT,
            (group.bx.w - group.head_w) / 2.0,
            group.bx.w - HEAD_LEFT - group.head_w,
        ] {
            let mut count = 0.0;
            for r in &routes {
                if crosses(
                    &r.points,
                    group.bx.x + x - 4.0,
                    top,
                    group.bx.x + x + group.head_w + 4.0,
                    bottom,
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
        taken.extend([
            group.bx.x + group.head_x - 4.0,
            top,
            group.bx.x + group.head_x + group.head_w + 4.0,
            bottom,
        ]);
    }

    let mut labels_out = String::new();
    for (index, rel) in db.rels.iter().enumerate() {
        let r = &routes[index];
        let id = format!("{}-{}", rel.from, rel.to);
        let style = user_style(None, None, rel.text_color.as_deref());
        let text = if db.c4_type.as_deref() == Some("C4Dynamic") {
            format!("{}: {}", index + 1, rel.label)
        } else {
            rel.label.clone()
        };
        let label = layout_label(Some(&text), false, measurer, small, r.room, 0);
        let techn_text = rel
            .techn
            .as_deref()
            .filter(|techn| !techn.is_empty())
            .map(|techn| format!("[{techn}]"));
        let techn = layout_label(
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
        let beside = r.nx.abs() * (lw / 2.0) + r.ny.abs() * (lh / 2.0) + 2.0;
        let aside = if js::truthy(r.lean) {
            max(0.0, min(r.lean, beside - 14.0))
        } else {
            beside
        };
        // Manual offsets are tuned to the original spacing. Reposition compact
        // labels beside their routed arrows so an offset cannot leave them
        // floating outside a boundary.
        let mut dx = r.nx * aside + if tight { 0.0 } else { offset(rel.offset_x) };
        let mut dy = r.ny * aside + if tight { 0.0 } else { offset(rel.offset_y) };
        let mut at = 0.5;
        if careful {
            // Slide along the line to the spot that covers least. Offsets from
            // the diagram are kept unless a spot on the line itself covers
            // less than any they lead to: they are often tuned for Mermaid's
            // spacing.
            let mut least = f64::INFINITY;
            let moved = dx != r.nx * aside || dy != r.ny * aside;
            let passes = if moved { 2 } else { 1 };
            let mut pass = 0;
            while pass < passes && least > 0.0 {
                let px = if pass > 0 { r.nx * aside } else { dx };
                let py = if pass > 0 { r.ny * aside } else { dy };
                for t in SLIDE {
                    let cx = r.x0 + (r.x1 - r.x0) * t + px;
                    let cy = r.y0 + (r.y1 - r.y0) * t + py;
                    let area = covered(
                        &taken,
                        cx - lw / 2.0,
                        cy - lh / 2.0,
                        cx + lw / 2.0,
                        cy + lh / 2.0,
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
        let cx = r.x0 + (r.x1 - r.x0) * at + dx;
        let top = r.y0 + (r.y1 - r.y0) * at + dy - lh / 2.0;
        if careful {
            taken.extend([cx - lw / 2.0, top, cx + lw / 2.0, top + lh]);
        }
        bounds.grow(cx - lw / 2.0, top, cx + lw / 2.0, top + lh);
        let _ = write!(
            labels_out,
            "<g class=\"{p}-edge-label\" data-id=\"{}\">",
            esc_text(&id)
        );
        if let Some(descr) = rel.descr.as_deref().filter(|descr| !descr.is_empty()) {
            let _ = write!(labels_out, "<title>{}</title>", esc_text(descr));
        }
        let _ = write!(
            labels_out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>",
            num(cx - lw / 2.0),
            num(top),
            num(lw),
            num(lh)
        );
        let body = label_svg(&label, cx, top + label.height / 2.0, &style.text)
            + &label_svg(
                &techn,
                cx,
                top + label.height + techn.height / 2.0,
                &format!(" class=\"{p}-c4-techn\" fill=\"var(--_m)\"{}", style.text),
            );
        labels_out += &link(&body, rel.link.as_deref(), &id, &mut links, options);
        labels_out += "</g>";
    }

    let small_str = js::num_str(small);
    let mut clusters = String::new();
    let mut nodes_out = String::new();
    for &at in &layout.order {
        let group = &layout.groups[at];
        if at != root {
            let boundary = &db.boundaries[group.boundary];
            let style = user_style(
                boundary.bg_color.as_deref(),
                boundary.border_color.as_deref(),
                boundary.font_color.as_deref(),
            );
            let x = group.bx.x + group.head_x;
            let mut y = group.bx.y + HEAD_TOP;
            let mut text = left_label_svg(
                &group.labels[0],
                x,
                y,
                &format!(
                    " class=\"{p}-cluster-label\" font-weight=\"var(--_hw)\"{}",
                    style.text
                ),
            );
            y += group.labels[0].height;
            text += &left_label_svg(
                &group.labels[1],
                x,
                y,
                &format!(
                    " class=\"{p}-c4-type\" font-size=\"{small_str}\" fill=\"var(--_m)\"{}",
                    style.text
                ),
            );
            y += group.labels[1].height;
            text += &left_label_svg(
                &group.labels[2],
                x,
                y,
                &format!(
                    " class=\"{p}-c4-descr\" font-size=\"{small_str}\" fill=\"var(--_m)\"{}",
                    style.text
                ),
            );
            let node = boundary.node_type.as_deref().is_some_and(|t| !t.is_empty());
            let _ = write!(
                clusters,
                "<g class=\"{p}-cluster {p}-c4-{}\" data-id=\"{}\">",
                if node { "node" } else { "boundary" },
                esc_text(&boundary.alias)
            );
            let _ = write!(
                clusters,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"none\" stroke=\"var(--_b)\"{}{}/>",
                num(group.bx.x),
                num(group.bx.y),
                num(group.bx.w),
                num(group.bx.h),
                if node {
                    ""
                } else {
                    " stroke-dasharray=\"6 4\""
                },
                style.shape
            );
            clusters += &link(
                &text,
                boundary.link.as_deref(),
                &boundary.alias,
                &mut links,
                options,
            );
            clusters += "</g>";
        }
        for node in &group.nodes {
            let shape = &db.shapes[node.shape];
            let kind = shape.kind.as_deref().unwrap_or("");
            let external = kind.starts_with("external");
            let style = user_style(
                shape.bg_color.as_deref(),
                shape.border_color.as_deref(),
                shape.font_color.as_deref(),
            );
            let stroke = format!(
                " stroke=\"var(--_b)\"{}",
                if external {
                    " stroke-dasharray=\"4 3\""
                } else {
                    ""
                }
            );
            let C4Box { w, h, form, .. } = node.bx;
            let labels = &node.labels;
            let dx = if form == "h-cyl" {
                -queue_rx(h) / 2.0
            } else {
                0.0
            };
            let dy = match form {
                "person" => HEAD / 2.0,
                "cyl" => cyl_ry(w) / 2.0,
                _ => 0.0,
            };
            let muted = format!(" fill=\"var(--_m)\"{}", style.text);
            let mut y = dy - node.text_h / 2.0;
            let mut text = label_svg(
                &labels[0],
                dx,
                y + labels[0].height / 2.0,
                &format!(
                    " class=\"{p}-c4-type\" font-size=\"{}\"{muted}",
                    js::num_str(small - 1.0)
                ),
            );
            y += labels[0].height;
            text += &label_svg(
                &labels[1],
                dx,
                y + labels[1].height / 2.0,
                &format!(
                    " class=\"{p}-label\" font-weight=\"var(--_hw)\"{}",
                    style.text
                ),
            );
            y += labels[1].height;
            text += &label_svg(
                &labels[2],
                dx,
                y + labels[2].height / 2.0,
                &format!(" class=\"{p}-c4-techn\" font-size=\"{small_str}\"{muted}"),
            );
            y += labels[2].height + 4.0;
            text += &label_svg(
                &labels[3],
                dx,
                y + labels[3].height / 2.0,
                &format!(
                    " class=\"{p}-c4-descr\" font-size=\"{small_str}\"{}",
                    style.text
                ),
            );
            let _ = write!(
                nodes_out,
                "<g class=\"{p}-node {p}-shape-{form} {p}-c4-{kind}{}\" data-id=\"{}\" transform=\"translate({},{})\">",
                if external {
                    format!(" {p}-c4-external")
                } else {
                    String::new()
                },
                esc_text(&shape.alias),
                num(node.bx.x + w / 2.0),
                num(node.bx.y + h / 2.0)
            );
            let body = draw_shape(
                form,
                w,
                h,
                &format!(
                    " fill=\"var({})\"{stroke}{}",
                    if external { "--_bg" } else { "--_s" },
                    style.shape
                ),
                &format!("{stroke}{}", style.line),
            ) + &text;
            nodes_out += &link(
                &body,
                shape.link.as_deref(),
                &shape.alias,
                &mut links,
                options,
            );
            nodes_out += "</g>";
        }
    }

    let title = layout_label(
        db.title.as_deref(),
        false,
        measurer,
        size,
        if tight { title_room(options) } else { 4000.0 },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let Bounds {
        min_x,
        min_y,
        max_x,
        max_y,
    } = bounds;
    let inner = max(max_x - min_x, title.width);
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
    let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
    let oy = pad + title_height - min_y;

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    );
    let _ = write!(
        content,
        "<g transform=\"translate({},{})\">",
        num(ox),
        num(oy)
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
    if !nodes_out.is_empty() {
        let _ = write!(content, "<g class=\"{p}-nodes\">{nodes_out}</g>");
    }
    if !labels_out.is_empty() {
        content +=
            &format!("<g class=\"{p}-edge-labels\" font-size=\"{small_str}\">{labels_out}</g>");
    }
    content += "</g>";
    let svg = svg_document(
        "c4",
        width,
        height,
        size,
        options,
        Accessible {
            acc_title: db.acc_title.as_deref(),
            acc_descr: db.acc_descr.as_deref(),
        },
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
