//! Pele's `diagrams/mindmap/render.ts`.

use super::db::{MindmapModel, MindmapNode};
use crate::config::positive;
use crate::diagrams::Render;
use crate::geom::{Point, Side, Size};
use crate::svg::builder::{EscText, fragment};
use crate::{Config, Error};
use crate::{Ctx, Host};
use crate::{
    Rendered,
    diagrams::common::icon::icon_svg,
    js,
    svg::{
        builder::{TITLE, num},
        root::{Accessible, svg_document},
        shapes::Shape,
        theme::{SeriesColor, class_names, series_color},
    },
    text::{Label, Style, has_icon},
};
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::fmt::Write as _;

/// The root's shape when it has no delimiters. Any other node without them
/// is text on a line.
const ROOT_SHAPE: Shape = Shape::Stadium;

const GAP_X: f64 = 36.0;
const GAP_Y: f64 = 8.0;
/// Between two subtrees, of which at least one has children of its own.
const GROUP_GAP: f64 = 18.0;
const BRANCH_GAP_X: f64 = 52.0;
const BRANCH_GAP_Y: f64 = 28.0;
const TEXT_PAD: f64 = 6.0;
const PILL_PAD: f64 = 12.0;
const TINT: f64 = 0.22;
const ICON_GAP: f64 = 6.0;
/// Running down, each level is set in by this much, and links leave a node
/// this far from its left edge.
const INDENT: f64 = 28.0;
const STEM: f64 = 12.0;
const BEND: f64 = 8.0;
const ROW_GAP: f64 = 10.0;

/// Mermaid's `mindmap` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MindmapConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub max_node_width: Option<f64>,
}

impl Render for MindmapModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.turn_to_fit(|down| self.draw(&config.mindmap, ctx, down)))
    }
}

/// Where each subtree sits among its siblings: `rel` is a subtree's offset,
/// `own` the node's offset inside its band, `kids` the offset of its
/// children's bands, `extent` the height of its band.
struct Bands {
    extent: Vec<f64>,
    rel: Vec<f64>,
    own: Vec<f64>,
    kids: Vec<f64>,
    stacked: f64,
}

impl Bands {
    /// Stacks subtrees top to bottom and returns the height at which their
    /// parent's anchor belongs.
    fn stack(
        &mut self,
        nodes: &[MindmapNode],
        anchor: &[f64],
        list: &[usize],
        from: usize,
        to: usize,
        gap: f64,
    ) -> f64 {
        let mut y = 0.0;
        let mut last = 0.0;
        for k in from..to {
            let c = list[k];
            self.rel[c] = y;
            let grouped = !nodes[c].children.is_empty()
                || (k + 1 < to && !nodes[list[k + 1]].children.is_empty());
            last = if grouped { gap.max(GROUP_GAP) } else { gap };
            y += self.extent[c] + last;
        }
        self.stacked = y - last;
        let a = list[from];
        let b = list[to - 1];
        (self.rel[a] + self.own[a] + anchor[a] + self.rel[b] + self.own[b] + anchor[b]) / 2.0
    }
}

impl MindmapModel {
    /// `down` draws the map as an outline: the root at the top, and each level
    /// set in under its parent.
    fn draw(&self, config: &MindmapConfig, ctx: &Ctx<'_, impl Host>, down: bool) -> Rendered {
        let p = crate::PREFIX;
        let label_attrs = crate::class!("label");
        let size = ctx.options.font_size;
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let wrap = positive(config.max_node_width, 200.0);
        let glyph = size * 1.25;

        let nodes = &self.nodes;
        let n = nodes.len();
        let mut parent = vec![0usize; n];
        let mut depth = vec![0i32; n];
        for i in 0..n {
            for &child in &nodes[i].children {
                parent[child] = i;
                depth[child] = depth[i] + 1;
            }
        }
        // An outline has the width the host gives it, less what its level is set
        // in.
        let room = match ctx.options.max_width {
            Some(max) if down && max > 0.0 => max - 2.0 * pad,
            _ => f64::INFINITY,
        };
        let mut labels: Vec<Label> = Vec::with_capacity(n);
        let mut shapes: Vec<Option<Shape>> = Vec::with_capacity(n);
        let mut sizes = vec![Size::default(); n];
        let mut dy = vec![0.0; n];
        // Where links meet the node, measured down from its top: its middle.
        let mut anchor = vec![0.0; n];
        // Whether the host has the node's icon. One it does not have takes no
        // room.
        let mut drawn = vec![false; n];

        for i in 0..n {
            let node = &nodes[i];
            drawn[i] = node
                .icon
                .as_deref()
                .is_some_and(|icon| has_icon(ctx.host, icon));
            let fit = wrap.min(72f64.max(
                room - f64::from(depth[i]) * INDENT
                    - 2.0 * TEXT_PAD
                    - if drawn[i] { glyph + ICON_GAP } else { 0.0 },
            ));
            let label = Label::layout(Some(&node.descr), true, measure, size, fit, 0);
            let tw = label.width
                + if drawn[i] {
                    glyph + if label.width > 0.0 { ICON_GAP } else { 0.0 }
                } else {
                    0.0
                };
            let th = label.height.max(js::round(size * 1.5));
            let mut shape = node.kind.shape().or_else(|| (i == 0).then_some(ROOT_SHAPE));
            // In an outline the root heads a column, and the trunk drops from
            // under its left end. The root is a pill there unless it is a box
            // already.
            if down && i == 0 && !matches!(shape, Some(Shape::Rect | Shape::Rounded)) {
                shape = Some(ROOT_SHAPE);
            }
            shapes.push(shape);
            match shape {
                None => {
                    // A node with no shape of its own is plain text when it ends
                    // a branch, and text in a tinted pill when the branch goes on
                    // from it.
                    let pill = !node.children.is_empty();
                    sizes[i] = Size::new(
                        tw + 2.0 * if pill { PILL_PAD } else { TEXT_PAD },
                        th + if pill { 6.0 } else { 0.0 },
                    );
                    anchor[i] = sizes[i].h / 2.0;
                }
                Some(shape) => {
                    let s = shape.size(Size::new(tw, th));
                    sizes[i] = s.size;
                    dy[i] = s.dy;
                    anchor[i] = s.size.h / 2.0;
                }
            }
            labels.push(label);
        }

        let mut bands = Bands {
            extent: vec![0.0; n],
            rel: vec![0.0; n],
            own: vec![0.0; n],
            kids: vec![0.0; n],
            stacked: 0.0,
        };

        // Nodes are in depth-first order, so going backwards visits children
        // before their parent.
        for i in (1..n).rev() {
            let children = &nodes[i].children;
            if children.is_empty() {
                bands.extent[i] = sizes[i].h;
                continue;
            }
            let top = bands.stack(nodes, &anchor, children, 0, children.len(), GAP_Y) - anchor[i];
            let low = 0f64.min(top);
            bands.extent[i] = bands.stacked.max(top + sizes[i].h) - low;
            bands.own[i] = top - low;
            bands.kids[i] = -low;
        }

        // Each node's top centre.
        let mut at = vec![Point::default(); n];
        let mut band = vec![0.0; n];
        let mut side = vec![0.0f64; n];
        // Running down: how far the trunk under each node has been drawn.
        let mut trunk = vec![0.0; n];

        if down {
            // Nodes are in depth-first order, which is the order of an outline's
            // rows.
            let mut row = 0.0;
            for i in 0..n {
                if i > 0 && parent[i] == 0 {
                    row += ROW_GAP;
                }
                at[i].x = f64::from(depth[i]) * INDENT + sizes[i].w / 2.0;
                at[i].y = row;
                row += sizes[i].h + ROW_GAP;
            }
        } else if n > 0 {
            // The first branches go right and the rest left, split where the two
            // sides are closest in height.
            let branches = &nodes[0].children;
            let count = branches.len();
            let mut total = 0.0;
            for &branch in branches {
                total += bands.extent[branch];
            }
            let mut split = count;
            let mut best = f64::INFINITY;
            let mut right = 0.0;
            for k in 1..count {
                right += bands.extent[branches[k - 1]];
                let diff =
                    (2.0 * right - total + (2.0 * k as f64 - count as f64) * BRANCH_GAP_Y).abs();
                if diff < best {
                    best = diff;
                    split = k;
                }
            }
            let (mut from, mut to, mut dir) = (0, split, 1.0);
            while from < count {
                let middle = bands.stack(nodes, &anchor, branches, from, to, BRANCH_GAP_Y);
                for &c in &branches[from..to] {
                    band[c] = bands.rel[c] - middle;
                    side[c] = dir;
                }
                (from, to, dir) = (to, count, -1.0);
            }
            at[0].y = -sizes[0].h / 2.0;
            for i in 0..n {
                if i > 0 {
                    at[i].y = band[i] + bands.own[i];
                }
                for &c in &nodes[i].children {
                    if i > 0 {
                        side[c] = side[i];
                        band[c] = band[i] + bands.kids[i] + bands.rel[c];
                    }
                    at[c].x = at[i].x
                        + side[c]
                            * (sizes[i].w / 2.0
                                + if i > 0 { GAP_X } else { BRANCH_GAP_X }
                                + sizes[c].w / 2.0);
                }
            }
        }

        let mut min_x = 0f64;
        let mut max_x = 0f64;
        let mut min_y = 0f64;
        let mut max_y = 0f64;
        let mut out = String::new();
        let mut edges = String::new();
        let mut body = String::new();
        // No colour before the first branch.
        let mut color: Option<SeriesColor> = None;
        let paint = |color: Option<SeriesColor>| {
            fragment(move |f| match color {
                Some(color) => write!(f, "{color}"),
                None => Ok(()),
            })
        };
        let mut tint = String::new();
        let mut stroke = String::new();
        let flush = |out: &mut String,
                     edges: &mut String,
                     body: &mut String,
                     color: Option<SeriesColor>,
                     section: Option<usize>| {
            if body.is_empty() {
                return;
            }
            let _ = write!(
                out,
                "<g class=\"{p}-branch {p}-section-{}\"><g class=\"{p}-edges\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\">{edges}</g>{body}</g>",
                section.unwrap_or(0),
                paint(color)
            );
            edges.clear();
            body.clear();
        };

        // The root is drawn last, on top of the links that leave it.
        for k in 1..=n {
            let i = k % n;
            let node = &nodes[i];
            let shape = shapes[i];
            let label = &labels[i];
            let cx = at[i].x;
            let top = at[i].y;
            min_x = min_x.min(cx - sizes[i].w / 2.0);
            max_x = max_x.max(cx + sizes[i].w / 2.0);
            min_y = min_y.min(top);
            max_y = max_y.max(top + sizes[i].h);

            if i == 0 || parent[i] == 0 {
                flush(&mut out, &mut edges, &mut body, color, nodes[k - 1].section);
            }
            if i > 0 && down {
                if parent[i] == 0 {
                    color = Some(series_color(node.section.unwrap_or(0) as i64));
                }
                let p = parent[i];
                // The link drops from under its parent and turns in to the node's
                // left. It starts where the link to the sibling above turned in, so
                // each stretch of a shared trunk has one color. Under the root's
                // rounded end, the trunk starts where that end has come down to the
                // bottom.
                let stem = if p == 0 {
                    STEM.max(sizes[0].h / 2.0).min(INDENT - BEND)
                } else {
                    STEM
                };
                let x1 = at[p].x - sizes[p].w / 2.0 + stem.min(sizes[p].w / 2.0);
                let y1 = if js::truthy(trunk[p]) {
                    trunk[p]
                } else {
                    at[p].y + sizes[p].h
                };
                let left = cx - sizes[i].w / 2.0;
                let x2 = left + shape.map_or(0.0, |shape| shape.inset(sizes[i], Side::Left, 0.0));
                let y2 = top + anchor[i];
                let bend = 0f64.max(BEND.min(x2 - x1).min(y2 - y1));
                // The next link carries on from where this one leaves the trunk,
                // so the trunk has no gap.
                trunk[p] = y2 - bend;
                let _ = write!(
                    edges,
                    "<path class=\"{}-edge\" d=\"M{},{}V{}Q{},{} {},{}H{}\"/>",
                    crate::PREFIX,
                    num(x1),
                    num(y1),
                    num(y2 - bend),
                    num(x1),
                    num(y2),
                    num(x1 + bend),
                    num(y2),
                    num(x2)
                );
            } else if i > 0 {
                if parent[i] == 0 {
                    color = Some(series_color(node.section.unwrap_or(0) as i64));
                }
                let p = parent[i];
                let dir = side[i];
                let from = if dir > 0.0 { Side::Right } else { Side::Left };
                let x1 = at[p].x
                    + dir
                        * (sizes[p].w / 2.0
                            - shapes[p].map_or(0.0, |shape| shape.inset(sizes[p], from, 0.0)));
                let y1 = at[p].y + anchor[p];
                let near = cx - (dir * sizes[i].w) / 2.0;
                let x2 = near
                    + shape.map_or(0.0, |shape| {
                        dir * shape.inset(sizes[i], from.opposite(), 0.0)
                    });
                let y2 = top + anchor[i];
                let mid = num((x1 + x2) / 2.0);
                let _ = write!(
                    edges,
                    "<path class=\"{}-edge\" d=\"M{},{}C{mid},{} {mid},{} {},{}\"/>",
                    crate::PREFIX,
                    num(x1),
                    num(y1),
                    num(y1),
                    num(y2),
                    num(x2),
                    num(y2)
                );
            }

            let pill = shape.is_none() && !node.children.is_empty();
            // Every node off the root is filled with a tint of its branch's color
            // and outlined in it, whether its shape was given in the source or is
            // the pill of a node that branches. The root is neutral.
            tint.clear();
            let _ = write!(
                tint,
                " fill=\"{0}\" fill-opacity=\"{1}\" stroke=\"{0}\"",
                paint(color),
                js::Number(TINT)
            );
            let kind = match shape {
                Some(shape) => shape.name(),
                None if pill => "branch",
                None => "text",
            };
            let group = if i > 0 { &mut body } else { &mut out };
            let _ = write!(
                group,
                "<g class=\"{p}-node {p}-shape-{kind}{}{}\" data-id=\"{}\" transform=\"translate({},{})\">",
                if i > 0 {
                    ""
                } else {
                    concat!(" ", crate::prefix!(), "-root")
                },
                class_names(node.class.as_deref().unwrap_or("")),
                EscText(&node.node_id),
                num(cx),
                num(top + sizes[i].h / 2.0),
            );
            if let Some(shape) = shape {
                if i > 0 {
                    stroke.clear();
                    let _ = write!(stroke, " stroke=\"{}\"", paint(color));
                    shape.draw(group, sizes[i], &tint, &stroke);
                } else {
                    shape.draw(
                        group,
                        sizes[i],
                        " fill=\"var(--_s)\" stroke=\"var(--_b)\"",
                        " stroke=\"var(--_b)\"",
                    );
                }
            } else if pill {
                let _ = write!(
                    group,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{tint}/>",
                    num(-sizes[i].w / 2.0),
                    num(-sizes[i].h / 2.0),
                    num(sizes[i].w),
                    num(sizes[i].h),
                    num(sizes[i].h / 2.0)
                );
            }
            let mut shift = 0.0;
            if let Some(icon) = node.icon.as_deref().filter(|icon| !icon.is_empty())
                && drawn[i]
            {
                shift = (glyph + if label.width > 0.0 { ICON_GAP } else { 0.0 }) / 2.0;
                let _ = write!(
                    group,
                    "{}",
                    icon_svg(
                        ctx.host,
                        icon,
                        Point::new(-shift - label.width / 2.0, dy[i] - glyph / 2.0),
                        glyph
                    )
                );
            }
            let _ = write!(
                group,
                "{}</g>",
                fragment(|f| if down && shape.is_none() {
                    write!(
                        f,
                        "{}",
                        label.left_svg(Point::new(shift - label.width / 2.0, dy[i]), label_attrs)
                    )
                } else {
                    write!(f, "{}", label.svg(Point::new(shift, dy[i]), label_attrs))
                })
            );
        }

        let title = Label::layout(
            self.title.as_deref(),
            false,
            measure,
            size,
            ctx.title_room(),
            Style::BOLD,
        );
        let title_height = title.spaced_height(16.0);
        let width = (max_x - min_x).max(title.width);
        let total_width = (width + 2.0 * pad).ceil();
        let total_height = (max_y - min_y + title_height + 2.0 * pad).ceil();
        let mut content = String::with_capacity(out.len() + 512);
        let _ = write!(
            content,
            "{}<g transform=\"translate({},{})\">{out}</g>",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                TITLE
            ),
            num(pad - min_x + (width - max_x + min_x) / 2.0),
            num(pad + title_height - min_y)
        );

        Rendered {
            svg: svg_document(
                "mindmap",
                total_width,
                total_height,
                size,
                ctx,
                Accessible::default(),
                &content,
            ),
            width: total_width,
            height: total_height,
            links: Vec::new(),
        }
    }
}
