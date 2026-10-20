//! Pele's `diagrams/ishikawa/render.ts`.

use super::{
    db::IshikawaDb,
    types::{NodeId, Nodes},
};
use crate::diagrams::Render;
use crate::geom::{Point, Size};
use crate::svg::builder::EscText;
use crate::svg::edges::Marker;
use crate::{Config, Ctx, Error};
use crate::{
    Rendered, js,
    svg::{
        builder::{TITLE, num},
        edges::ARROW,
        root::{Accessible, svg_document},
        theme::RADIUS,
    },
    text::{Host, Label, Style},
};
use std::fmt::Write as _;

/// How far a bone moves toward the tail for each unit it rises from the
/// spine.
const SLANT: f64 = 0.5;
const FIRST_ROW: f64 = 14.0;
const ROW_GAP: f64 = 6.0;
const BONE_END: f64 = 8.0;
const TWIG_STEP: f64 = 22.0;
const STUB: f64 = 18.0;
const TEXT_GAP: f64 = 5.0;
const PAIR_GAP: f64 = 26.0;
const HEAD_GAP: f64 = 34.0;
const TAIL: f64 = 28.0;
const BOX_PAD_X: f64 = 10.0;
const BOX_PAD_Y: f64 = 5.0;

/// `Math.hypot(SLANT, 1)`.
fn length() -> f64 {
    SLANT.hypot(1.0)
}

/// A cause below a category. Bones at even depths run level, from the slanted
/// bone above them toward the tail; bones at odd depths slant away from the
/// spine, from the level bone above them.
struct Bone {
    label: Label,
    depth: usize,
    parent: Option<usize>,
    /// Its place among its siblings, counted from the spine outward.
    slot: usize,
    kids: usize,
    /// How far its row is from the spine.
    rise: f64,
    /// Where it meets its parent, measured level from the category's bone: 0
    /// on it, negative toward the tail.
    at: f64,
    /// Where its text ends.
    text_end: f64,
}

struct Category {
    node: NodeId,
    label: Label,
    frame: Size,
    bones: Vec<Bone>,
    /// The length of its bone, as distance from the spine.
    rise: f64,
    /// How far its causes reach toward the tail, measured level from its bone.
    reach: f64,
}

struct Frame {
    node: NodeId,
    depth: usize,
    parent: Option<usize>,
    slot: usize,
    /// Set when the bone was made and only its row is still to come.
    bone: Option<usize>,
}

impl Category {
    /// Gives every cause of a category a row of its own, so that no two texts can
    /// meet, and measures how far the whole of it reaches. A level bone takes the
    /// row nearest the spine and its twigs the rows beyond; a slanted twig takes
    /// the row beyond everything that hangs off it. The outline may be nested as
    /// deep as its indentation goes, so the walk keeps a stack of its own.
    fn arrange(
        nodes: &Nodes,
        node: NodeId,
        up: bool,
        measure: &impl Host,
        size: f64,
        small: f64,
    ) -> Self {
        let label = Label::layout(Some(&nodes.get(node).text), false, measure, size, 180.0, 0);
        let line_height = js::round(small * 1.5);
        let mut bones: Vec<Bone> = Vec::new();
        let mut rows: Vec<usize> = Vec::new();
        let mut stack: Vec<Frame> = Vec::new();
        // Above the spine the first cause goes farthest out, so that the causes
        // read from the top down.
        let push =
            |stack: &mut Vec<Frame>, children: &[NodeId], depth: usize, parent: Option<usize>| {
                let n = children.len();
                for slot in (0..n).rev() {
                    stack.push(Frame {
                        node: children[if up { n - 1 - slot } else { slot }],
                        depth,
                        parent,
                        slot,
                        bone: None,
                    });
                }
            };
        push(&mut stack, &nodes.get(node).children, 2, None);
        while let Some(mut frame) = stack.pop() {
            if let Some(bone) = frame.bone {
                rows.push(bone);
                continue;
            }
            let index = bones.len();
            let source = nodes.get(frame.node);
            bones.push(Bone {
                label: Label::layout(Some(&source.text), false, measure, small, 200.0, 0),
                depth: frame.depth,
                parent: frame.parent,
                slot: frame.slot,
                kids: source.children.len(),
                rise: 0.0,
                at: 0.0,
                text_end: 0.0,
            });
            let depth = frame.depth;
            if depth % 2 == 0 {
                rows.push(index);
            } else {
                frame.bone = Some(index);
                stack.push(frame);
            }
            push(&mut stack, &source.children, depth + 1, Some(index));
        }

        let mut rise = FIRST_ROW;
        for &index in &rows {
            let height = bones[index].label.height.max(line_height) + ROW_GAP;
            bones[index].rise = rise + height / 2.0;
            rise += height;
        }
        let box_w = label.width + 2.0 * BOX_PAD_X;
        let mut reach = box_w / 2.0;
        for i in 0..bones.len() {
            let parent = bones[i]
                .parent
                .map(|parent| (bones[parent].at, bones[parent].kids));
            let bone = &mut bones[i];
            if bone.depth.is_multiple_of(2) {
                bone.at = parent.map_or(0.0, |(at, _)| at);
                bone.text_end = bone.at
                    - (if bone.kids > 0 {
                        (bone.kids as f64 + 1.0) * TWIG_STEP
                    } else {
                        STUB
                    })
                    - TEXT_GAP;
            } else {
                // Twigs stand along their level bone, the one nearest the spine
                // nearest the tail.
                let (at, kids) = parent.unwrap_or_default();
                bone.at = at - (kids as f64 - bone.slot as f64) * TWIG_STEP;
                bone.text_end = bone.at - TEXT_GAP;
            }
            reach = reach.max(bone.label.width - bone.text_end);
        }
        Category {
            node,
            frame: Size::new(box_w, label.height + 2.0 * BOX_PAD_Y),
            label,
            bones,
            rise: rise + BONE_END,
            reach,
        }
    }
}

impl Render for IshikawaDb {
    fn render(&self, _config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.turn_to_fit(|down| {
            if down {
                self.draw_outline(ctx)
            } else {
                self.draw_fishbone(ctx)
            }
        }))
    }
}

impl IshikawaDb {
    fn draw_fishbone(&self, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let p = crate::PREFIX;
        let nodes = &self.nodes;
        let title = Label::layout(
            self.title.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);

        let mut body = String::new();
        let mut min_x = 0f64;
        let mut max_x = 0f64;
        let mut min_y = 0f64;
        let mut max_y = 0f64;
        if let Some(root) = self.root {
            let root = nodes.get(root);
            let head = Label::layout(Some(&root.text), false, measure, size, 200.0, Style::BOLD);
            let head_h = (head.height + 24.0).max(48.0);
            let head_w = head.width + 24.0 + head_h / 2.0;
            let mut categories: Vec<Category> = root
                .children
                .iter()
                .enumerate()
                .map(|(i, &node)| Category::arrange(nodes, node, i % 2 == 0, measure, size, small))
                .collect();
            // Even a category without causes has a bone long enough to carry its
            // name clear of the head.
            for category in &mut categories {
                category.rise = category.rise.max(head_h / 2.0 + 14.0);
            }

            // Categories meet the spine in pairs, one above and one below, each
            // pair as far toward the tail from the one before as the wider of the
            // two needs.
            let mut xs: Vec<f64> = Vec::new();
            let mut x = -HEAD_GAP;
            let mut i = 0;
            while i < categories.len() {
                if i > 0 {
                    let mut room = 0f64;
                    for side in 0..2 {
                        if let Some(before) = categories.get(i - 2 + side) {
                            let next = categories
                                .get(i + side)
                                .map_or(0.0, |next| next.frame.w / 2.0);
                            room = room.max(before.reach + next);
                        }
                    }
                    x -= room + PAIR_GAP;
                }
                xs.push(x);
                xs.push(x);
                i += 2;
            }
            let tail = if categories.is_empty() {
                -HEAD_GAP
            } else {
                x - TAIL
            };

            max_x = head_w;
            min_x = tail;
            min_y = -head_h / 2.0;
            max_y = head_h / 2.0;
            let r = head_h / 2.0;
            let _ = write!(
                body,
                "<path class=\"{p}-ishikawa-spine\" d=\"M{},0H{}\" fill=\"none\" stroke=\"var(--_l)\"/>{}",
                num(tail),
                num(-(ARROW - 1.0)),
                Marker::Arrow.draw(Point::new(0.0, 0.0), Point::new(1.0, 0.0), "var(--_l)")
            );
            let groups = &mut body;
            let mut d = String::new();
            let mut texts = String::new();
            let length = length();
            for (i, category) in categories.iter().enumerate() {
                let sign = if i % 2 == 0 { -1.0 } else { 1.0 };
                let ax = xs[i];
                let px = |at: f64, rise: f64| ax + at - rise * SLANT;
                let end_x = px(0.0, category.rise);
                let end_y = sign * category.rise;
                // The bone stops short of the spine to leave its arrowhead a
                // clean point.
                let trim = ARROW - 1.0;
                d.clear();
                texts.clear();
                let _ = write!(
                    d,
                    "M{},{}L{},{}",
                    num(ax - (SLANT / length) * trim),
                    num((sign * trim) / length),
                    num(end_x),
                    num(end_y)
                );
                for bone in &category.bones {
                    let y = sign * bone.rise;
                    if bone.depth.is_multiple_of(2) {
                        let _ = write!(
                            d,
                            "M{},{}H{}",
                            num(px(bone.at, bone.rise)),
                            num(y),
                            num(px(bone.text_end + TEXT_GAP, bone.rise))
                        );
                    } else {
                        let from = bone
                            .parent
                            .map_or(0.0, |parent| category.bones[parent].rise);
                        let _ = write!(
                            d,
                            "M{},{}L{},{}",
                            num(px(bone.at, from)),
                            num(sign * from),
                            num(px(bone.at, bone.rise)),
                            num(y)
                        );
                    }
                    let right = px(bone.text_end, bone.rise);
                    let _ = write!(
                        texts,
                        "{}",
                        bone.label.svg(
                            Point::new(right - bone.label.width / 2.0, y),
                            if bone.depth > 2 {
                                " fill=\"var(--_m)\""
                            } else {
                                ""
                            }
                        )
                    );
                    min_x = min_x.min(right - bone.label.width);
                }
                let box_y = end_y + sign * (category.frame.h / 2.0);
                min_x = min_x.min(end_x - category.frame.w / 2.0);
                max_x = max_x.max(end_x + category.frame.w / 2.0);
                min_y = min_y.min(box_y - category.frame.h / 2.0);
                max_y = max_y.max(box_y + category.frame.h / 2.0);
                let _ = write!(
                    groups,
                    "<g class=\"{p}-ishikawa-category\" data-id=\"{}\"><path class=\"{p}-ishikawa-bones\" d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>",
                    EscText(&nodes.get(category.node).text)
                );
                let _ = write!(
                    groups,
                    "{}",
                    Marker::Arrow.draw(
                        Point::new(ax, 0.0),
                        Point::new(SLANT / length, -sign / length),
                        "var(--_l)"
                    )
                );
                let _ = write!(
                    groups,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
                    num(end_x - category.frame.w / 2.0),
                    num(box_y - category.frame.h / 2.0),
                    num(category.frame.w),
                    num(category.frame.h)
                );
                let _ = write!(
                    groups,
                    "{}",
                    category
                        .label
                        .svg(Point::new(end_x, box_y), crate::class!("label"))
                );
                if !texts.is_empty() {
                    let _ = write!(
                        groups,
                        "<g class=\"{p}-ishikawa-causes\" font-size=\"{}\">{texts}</g>",
                        js::Number(small)
                    );
                }
                groups.push_str("</g>");
            }

            let _ = write!(
                body,
                "<g class=\"{p}-ishikawa-head\" data-id=\"{}\"><path d=\"M0,{}H{}A{},{} 0 0 1 {},{}H0Z\" fill=\"var(--_s)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/>",
                EscText(&root.text),
                num(-r),
                num(head_w - r),
                num(r),
                num(r),
                num(head_w - r),
                num(r)
            );
            let _ = write!(
                body,
                "{}",
                head.svg(
                    Point::new(12.0 + head.width / 2.0, 0.0),
                    crate::class!("label", "font-weight=\"var(--_hw)\"")
                )
            );
            body.push_str("</g>");
        }

        let inner = (max_x - min_x).max(title.width);
        let width = (inner + 2.0 * pad).ceil();
        let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
        let ox = pad + (inner - (max_x - min_x)) / 2.0 - min_x;
        let oy = pad + title_height - min_y;
        let mut content = String::with_capacity(body.len() + 512);
        let _ = write!(
            content,
            "{}",
            title.svg(Point::new(width / 2.0, pad + title.height / 2.0), TITLE)
        );
        if !body.is_empty() {
            let _ = write!(
                content,
                "<g transform=\"translate({},{})\">{body}</g>",
                num(ox),
                num(oy)
            );
        }
        let svg = svg_document(
            "ishikawa",
            width,
            height,
            size,
            ctx,
            Accessible::default(),
            &content,
        );
        Rendered {
            svg,
            width,
            height,
            links: Vec::new(),
        }
    }

    /// The outline drawn as an indented list, for a drawing turned to fit.
    fn draw_outline(&self, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let p = crate::PREFIX;
        let nodes = &self.nodes;
        let room = 140f64.max(ctx.options.max_width.unwrap_or(320.0) - 2.0 * pad);
        let title = Label::layout(
            self.title.as_deref(),
            false,
            measure,
            size,
            ctx.title_room(),
            Style::BOLD,
        );
        let mut order: Vec<Entry> = Vec::new();
        let mut pending: Vec<(NodeId, usize, Option<usize>)> = Vec::new();
        if let Some(root) = self.root {
            pending.push((root, 0, None));
        }
        let mut depth = 0;
        while let Some((node, entry_depth, parent)) = pending.pop() {
            let index = order.len();
            order.push(Entry {
                node,
                depth: entry_depth,
                parent,
                at: Point::default(),
                size: Size::default(),
            });
            depth = depth.max(entry_depth);
            for &child in nodes.get(node).children.iter().rev() {
                pending.push((child, entry_depth + 1, Some(index)));
            }
        }
        let indent = 20f64.min(2f64.max((room - 120.0) / (depth.max(1) as f64)));
        let mut y = pad + title.spaced_height(12.0);
        let mut width = title.width;
        let mut shapes = String::new();
        let mut edges = String::new();
        let small_attrs = format!(" class=\"{p}-label\" font-size=\"{}\"", js::Number(small));
        for k in 0..order.len() {
            let entry = &order[k];
            let entry_depth = entry.depth as f64;
            let x = pad + entry_depth * indent;
            let label_room = 60f64.max(room - entry_depth * indent - 20.0);
            let text = &nodes.get(entry.node).text;
            let label = Label::layout(
                Some(text),
                false,
                measure,
                if entry.depth < 2 { size } else { small },
                label_room,
                if entry.depth < 2 { Style::BOLD } else { 0 },
            )
            .broken(measure, label_room);
            let w = label.width + 20.0;
            let h = label.height + 12.0;
            let top = y;
            y += h + 12.0;
            width = width.max(x - pad + w);
            let framed = entry.depth < 2;
            let class = match entry.depth {
                0 => "head",
                1 => "category",
                _ => "cause",
            };
            let _ = write!(
                shapes,
                "<g class=\"{p}-node {p}-ishikawa-{class}\" data-id=\"{}\">",
                EscText(text)
            );
            if framed {
                let _ = write!(
                    shapes,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
                    num(x),
                    num(top),
                    num(w),
                    num(h),
                    if entry.depth == 0 { 12 } else { 4 }
                );
            }
            let attrs = if entry.depth < 2 {
                crate::class!("label", "font-weight=\"var(--_hw)\"")
            } else {
                &small_attrs
            };
            let _ = write!(
                shapes,
                "{}</g>",
                label.svg(Point::new(x + w / 2.0, top + h / 2.0), attrs)
            );
            if let Some(parent) = entry.parent {
                let parent = &order[parent];
                let gx = parent.at.x + indent / 2.0;
                let _ = write!(
                    edges,
                    "<path class=\"{p}-edge\" d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\"/>",
                    num(gx),
                    num(parent.at.y + parent.size.h),
                    num(top + h / 2.0),
                    num(x)
                );
            }
            let entry = &mut order[k];
            entry.at = Point::new(x, top);
            entry.size = Size::new(w, h);
        }
        let total_width = (width + 2.0 * pad).ceil();
        let height = ((pad + title.height).max(y - 12.0) + pad).ceil();
        let mut content = String::with_capacity(edges.len() + shapes.len() + 512);
        let _ = write!(
            content,
            "{}{edges}{shapes}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );
        Rendered {
            svg: svg_document(
                "ishikawa",
                total_width,
                height,
                size,
                ctx,
                Accessible::default(),
                &content,
            ),
            width: total_width,
            height,
            links: Vec::new(),
        }
    }
}

struct Entry {
    node: NodeId,
    depth: usize,
    parent: Option<usize>,
    at: Point,
    size: Size,
}
