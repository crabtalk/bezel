//! Pele's `diagrams/c4/layout.ts`.

use std::collections::HashMap;

use super::{
    db::C4Model,
    route::{C4Box, HEAD, LANE},
    types::{C4Element, C4Ref, ShapeBase, Store},
};
use crate::{
    geom::{Point, Size},
    js::{max, min},
    svg::shapes::Shape,
    text::{Host, Label, Style},
};

const SHAPE_W: f64 = 216.0;
const SHAPE_MIN_H: f64 = 64.0;
const PAD_X: f64 = 14.0;
const PAD_Y: f64 = 12.0;
const GAP_X: f64 = 100.0;
const GAP_Y: f64 = 88.0;
/// Room for a relation to run along the inside of a boundary.
pub const GROUP_PAD: f64 = LANE + 8.0;
const GROUP_GAP: f64 = 48.0;
/// Where a boundary's heading starts, from its top-left corner.
pub const HEAD_LEFT: f64 = 16.0;
pub const HEAD_TOP: f64 = 10.0;

/// Shape keywords Mermaid accepts in $shape, $sprite or $tags, and the
/// shared shapes they name.
const FORMS: [(&str, Shape); 9] = [
    ("person", Shape::Person),
    ("box", Shape::Rect),
    ("rounded", Shape::Rect),
    ("component", Shape::Rect),
    ("cylinder", Shape::Cylinder),
    ("database", Shape::Cylinder),
    ("db", Shape::Cylinder),
    ("queue", Shape::HorizontalCylinder),
    ("pipe", Shape::HorizontalCylinder),
];

pub struct Node {
    pub bx: C4Box,
    pub shape: usize,
    /// Type, name, technology, description.
    pub labels: Vec<Label>,
    pub text_h: f64,
}

pub struct Group {
    pub bx: C4Box,
    pub boundary: usize,
    pub kids: Vec<usize>,
    pub nodes: Vec<Node>,
    /// Name, type, description.
    pub labels: Vec<Label>,
    pub head: f64,
    pub head_w: f64,
    pub head_x: f64,
}

pub struct Layout {
    /// Every boundary's group, by its index in the model's `boundaries`.
    pub groups: Vec<Group>,
    /// The boundaries that are drawn, parents before children. The first is
    /// the unnamed top level.
    pub order: Vec<usize>,
    pub boxes: HashMap<C4Ref, C4Box>,
}

fn form(name: &str) -> Option<Shape> {
    let name = name.to_lowercase();
    FORMS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|&(_, form)| form)
}

impl C4Element {
    /// The shared shape a shape element is drawn as.
    fn form(&self) -> Shape {
        let explicit = form(self.shape.as_deref().unwrap_or(""))
            .or_else(|| form(self.sprite.as_deref().unwrap_or("")));
        if let Some(explicit) = explicit {
            return explicit;
        }
        if let Some(tags) = self.tags.as_deref().filter(|tags| !tags.is_empty()) {
            for tag in tags.split(',') {
                if let Some(form) = form(crate::js::trim(tag)) {
                    return form;
                }
            }
        }
        match self.kind {
            Some(kind) if kind.base == ShapeBase::Person => Shape::Person,
            Some(kind) => match kind.store {
                Some(Store::Db) => Shape::Cylinder,
                Some(Store::Queue) => Shape::HorizontalCylinder,
                None => Shape::Rect,
            },
            None => Shape::Rect,
        }
    }
}

fn measure(
    shape: &C4Element,
    at: usize,
    measurer: &impl Host,
    size: f64,
    small: f64,
    width: f64,
) -> Node {
    let form = shape.form();
    let cap_w = if form == Shape::HorizontalCylinder {
        36.0
    } else {
        0.0
    };
    let cap_h = match form {
        Shape::Person => HEAD,
        Shape::Cylinder => 24.0,
        _ => 0.0,
    };
    let inner = width - 2.0 * PAD_X - cap_w;
    let kind = format!(
        "«{}»",
        shape
            .kind
            .map_or(String::new(), |kind| kind.to_string().replace('_', " "))
    );
    let techn = shape
        .techn
        .as_deref()
        .filter(|techn| !techn.is_empty())
        .map(|techn| format!("[{techn}]"));
    let labels = vec![
        Label::layout(Some(&kind), false, measurer, small - 1.0, inner, 0),
        Label::layout(
            Some(&shape.label),
            false,
            measurer,
            size,
            inner,
            Style::BOLD,
        ),
        Label::layout(techn.as_deref(), false, measurer, small, inner, 0),
        Label::layout(shape.descr.as_deref(), false, measurer, small, inner, 0),
    ];
    let mut tw: f64 = 0.0;
    let mut text_h = if labels[3].height > 0.0 { 4.0 } else { 0.0 };
    for label in &labels {
        tw = max(tw, label.width);
        text_h += label.height;
    }
    Node {
        bx: C4Box {
            at: Point::default(),
            size: Size::new(
                max(width, tw + 2.0 * PAD_X + cap_w),
                max(SHAPE_MIN_H, text_h + 2.0 * PAD_Y + cap_h),
            ),
            form,
            id: C4Ref::Shape(at),
        },
        shape: at,
        labels,
        text_h,
    }
}

/// Places boxes left to right in rows of `per_row`, from (0, 0), and returns
/// the size the rows take. Shapes are stretched to the height of their row;
/// boundaries keep their own.
fn rows(boxes: &mut [&mut C4Box], per_row: f64, gap_x: f64, gap_y: f64, stretch: bool) -> Size {
    let mut width: f64 = 0.0;
    let mut y = 0.0;
    let len = boxes.len();
    let mut from = 0;
    while from < len {
        let to = min(len as f64, from as f64 + per_row) as usize;
        let mut x = 0.0;
        let mut row_h: f64 = 0.0;
        for b in &boxes[from..to] {
            row_h = max(row_h, b.size.h);
        }
        for b in &mut boxes[from..to] {
            b.at = Point::new(x, y);
            if stretch {
                b.size.h = row_h;
            }
            x += b.size.w + gap_x;
        }
        width = max(width, x - gap_x);
        y += row_h + gap_y;
        from = to;
    }
    Size::new(width, if len > 0 { y - gap_y } else { 0.0 })
}

impl C4Model {
    /// Mermaid's grid: in each boundary, its shapes in rows of c4ShapeInRow, then
    /// the boundaries inside it in rows of c4BoundaryInRow, each sized around its
    /// own content.
    pub fn layout(
        &self,
        measurer: &impl Host,
        size: f64,
        small: f64,
        room: Option<f64>,
        gap: f64,
    ) -> Layout {
        let db = self;
        let group_pad = if room.is_none() { GROUP_PAD } else { 12.0 };
        let mut groups: Vec<Group> = Vec::with_capacity(db.boundaries.len());
        for (at, boundary) in db.boundaries.iter().enumerate() {
            let label_room = match room {
                None => 4000.0,
                Some(room) => max(80.0, room - 2.0 * HEAD_LEFT - 2.0 * GROUP_PAD * 2.0),
            };
            let labels = if boundary.parent.is_some() {
                let ty = boundary
                    .ty
                    .as_deref()
                    .filter(|ty| !ty.is_empty())
                    .map(|ty| format!("[{ty}]"));
                vec![
                    Label::layout(
                        Some(&boundary.label),
                        false,
                        measurer,
                        size,
                        label_room,
                        Style::BOLD,
                    ),
                    Label::layout(ty.as_deref(), false, measurer, small, label_room, 0),
                    Label::layout(
                        boundary.descr.as_deref(),
                        false,
                        measurer,
                        small,
                        label_room,
                        0,
                    ),
                ]
            } else {
                Vec::new()
            };
            let mut head = 0.0;
            let mut head_w: f64 = 0.0;
            for label in &labels {
                head += label.height;
                head_w = max(head_w, label.width);
            }
            groups.push(Group {
                bx: C4Box {
                    at: Point::default(),
                    size: Size::default(),
                    form: Shape::Rect,
                    id: C4Ref::Boundary(at),
                },
                boundary: at,
                kids: Vec::new(),
                nodes: Vec::new(),
                labels,
                head: if head > 0.0 {
                    head + HEAD_TOP + 6.0 + LANE
                } else {
                    group_pad
                },
                head_w,
                head_x: HEAD_LEFT,
            });
        }
        let root = 0;
        for at in 0..groups.len() {
            if let Some(parent) = db.boundaries[at].parent {
                groups[parent].kids.push(at);
            }
        }
        // A boundary that ends up inside itself is not reachable from the top,
        // as in Mermaid.
        let mut order: Vec<usize> = vec![root];
        let mut depths: HashMap<usize, f64> = HashMap::from([(root, 0.0)]);
        let mut i = 0;
        while i < order.len() {
            let parent = order[i];
            let depth = depths[&parent];
            for k in 0..groups[parent].kids.len() {
                let kid = groups[parent].kids[k];
                depths.insert(kid, depth + 1.0);
                order.push(kid);
            }
            i += 1;
        }
        for (at, shape) in db.shapes.iter().enumerate() {
            if let Some(parent) = shape.parent {
                let depth = depths.get(&parent).copied().unwrap_or(0.0);
                let node_width = match room {
                    None => SHAPE_W,
                    Some(room) => max(100.0, min(SHAPE_W, room - 2.0 * depth * group_pad - 32.0)),
                };
                let node = measure(shape, at, measurer, size, small, node_width);
                groups[parent].nodes.push(node);
            }
        }

        let per_row = max(1.0, db.shape_in_row);
        let groups_per_row = max(1.0, db.boundary_in_row);
        let mut kids_top: HashMap<usize, f64> = HashMap::new();
        for &at in order.iter().rev() {
            let gap_x = if room.is_none() { GAP_X } else { gap };
            let count = |boxes: &[C4Box], limit: f64, gap: f64| -> f64 {
                let Some(room) = room else {
                    return limit;
                };
                let mut widest: f64 = 1.0;
                for b in boxes {
                    widest = max(widest, b.size.w);
                }
                max(
                    1.0,
                    min(
                        limit,
                        ((room + gap - 2.0 * group_pad) / (widest + gap)).floor(),
                    ),
                )
            };
            let node_boxes: Vec<C4Box> = groups[at].nodes.iter().map(|node| node.bx).collect();
            let node_count = count(&node_boxes, per_row, gap_x);
            let Size {
                w: grid_w,
                h: grid_h,
            } = {
                let mut boxes: Vec<&mut C4Box> = groups[at]
                    .nodes
                    .iter_mut()
                    .map(|node| &mut node.bx)
                    .collect();
                rows(&mut boxes, node_count, gap_x, GAP_Y, true)
            };
            let kids = groups[at].kids.clone();
            let mut kid_boxes: Vec<C4Box> = kids.iter().map(|&kid| groups[kid].bx).collect();
            let kid_count = count(&kid_boxes, groups_per_row, GROUP_GAP);
            let Size {
                w: kids_w,
                h: kids_h,
            } = {
                let mut boxes: Vec<&mut C4Box> = kid_boxes.iter_mut().collect();
                rows(&mut boxes, kid_count, GROUP_GAP, GROUP_GAP, false)
            };
            for (&kid, b) in kids.iter().zip(&kid_boxes) {
                groups[kid].bx = *b;
            }
            let between = if grid_h > 0.0 && kids_h > 0.0 {
                GROUP_GAP
            } else {
                0.0
            };
            let frame = if at == root { 0.0 } else { group_pad };
            kids_top.insert(at, grid_h + between);
            let group = &mut groups[at];
            group.bx.size = Size::new(
                max(
                    max(grid_w, kids_w),
                    group.head_w + 2.0 * HEAD_LEFT - 2.0 * frame,
                ) + 2.0 * frame,
                (if frame != 0.0 { group.head } else { frame }) + grid_h + between + kids_h + frame,
            );
        }

        let mut boxes = HashMap::new();
        for &at in &order {
            let Point { x: gx, y: gy } = groups[at].bx.at;
            let left = gx + if at == root { 0.0 } else { group_pad };
            let top = gy + if at == root { 0.0 } else { groups[at].head };
            for node in &mut groups[at].nodes {
                node.bx.at += Point::new(left, top);
                boxes.insert(node.bx.id, node.bx);
            }
            let below = kids_top[&at];
            for k in 0..groups[at].kids.len() {
                let kid = groups[at].kids[k];
                groups[kid].bx.at += Point::new(left, top + below);
            }
            if at != root {
                boxes.insert(groups[at].bx.id, groups[at].bx);
            }
        }
        Layout {
            groups,
            order,
            boxes,
        }
    }
}
