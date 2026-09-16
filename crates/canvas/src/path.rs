//! Where an edge runs, in canvas units: the geometry an edge kind answers, a
//! press hits and a label sits on, with no window in sight.

use gpui::{Point, point};

use crate::model::{End, Node, Side};

/// Where an edge meets a box, and the way out of the box there.
pub type Anchor = (Point<f32>, Point<f32>);

/// A node's box where it paints, in canvas units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// `node`'s box, painted at `at` — which a glide may have short of where
    /// the document puts it.
    pub fn of(node: &Node, at: (f32, f32)) -> Self {
        Self {
            x: at.0,
            y: at.1,
            w: node.width as f32,
            h: node.height as f32,
        }
    }

    /// Where a side sits, and the way out of the box there.
    pub fn anchor(self, side: Side) -> Anchor {
        match side {
            Side::Top => (point(self.x + self.w / 2.0, self.y), point(0.0, -1.0)),
            Side::Right => (
                point(self.x + self.w, self.y + self.h / 2.0),
                point(1.0, 0.0),
            ),
            Side::Bottom => (
                point(self.x + self.w / 2.0, self.y + self.h),
                point(0.0, 1.0),
            ),
            Side::Left => (point(self.x, self.y + self.h / 2.0), point(-1.0, 0.0)),
        }
    }
}

/// What an edge kind is handed: the boxes it joins, the sides it was told to
/// leave from, and the ends it wears.
#[derive(Clone, Copy, Debug)]
pub struct Ends {
    pub from: Rect,
    pub to: Rect,
    /// Where it meets each box, when a handle said so. A side is a coarser
    /// way of saying the same thing, and an anchor wins over one.
    pub from_anchor: Option<Anchor>,
    pub to_anchor: Option<Anchor>,
    pub from_side: Option<Side>,
    pub to_side: Option<Side>,
    pub from_end: End,
    pub to_end: End,
}

impl Ends {
    /// Between two boxes, with the spec's ends and nothing else said.
    pub fn between(from: Rect, to: Rect) -> Self {
        Self {
            from,
            to,
            from_anchor: None,
            to_anchor: None,
            from_side: None,
            to_side: None,
            from_end: End::None,
            to_end: End::Arrow,
        }
    }

    /// The sides the two boxes face each other on, for an edge that names
    /// none.
    pub fn facing(&self) -> (Side, Side) {
        let (a, b) = (self.from, self.to);
        if b.x >= a.x + a.w {
            (Side::Right, Side::Left)
        } else if b.x + b.w <= a.x {
            (Side::Left, Side::Right)
        } else if b.y >= a.y + a.h {
            (Side::Bottom, Side::Top)
        } else {
            (Side::Top, Side::Bottom)
        }
    }

    /// Each end's anchor and the way out of the box there.
    pub fn anchors(&self) -> (Anchor, Anchor) {
        let (facing_from, facing_to) = self.facing();
        (
            self.from_anchor
                .unwrap_or_else(|| self.from.anchor(self.from_side.unwrap_or(facing_from))),
            self.to_anchor
                .unwrap_or_else(|| self.to.anchor(self.to_side.unwrap_or(facing_to))),
        )
    }

    fn arrows(&self) -> (bool, bool) {
        (self.from_end == End::Arrow, self.to_end == End::Arrow)
    }
}

/// An edge where it paints: quadratic segments, each start, control, end.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub segments: Vec<(Point<f32>, Point<f32>, Point<f32>)>,
    /// The way out of each end, which an arrowhead points along.
    pub from_out: Point<f32>,
    pub to_out: Point<f32>,
    pub from_arrow: bool,
    pub to_arrow: bool,
}

impl Path {
    pub fn start(&self) -> Point<f32> {
        self.segments.first().map_or(point(0.0, 0.0), |seg| seg.0)
    }

    pub fn end(&self) -> Point<f32> {
        self.segments.last().map_or(point(0.0, 0.0), |seg| seg.2)
    }

    /// Halfway along it, where a label sits: the join between two halves, or
    /// the middle of the odd segment in the middle.
    pub fn middle(&self) -> Point<f32> {
        let count = self.segments.len();
        if count == 0 {
            return point(0.0, 0.0);
        }
        if count % 2 == 0 {
            return self.segments[count / 2 - 1].2;
        }
        let (a, c, b) = self.segments[count / 2];
        along(a, c, b, 0.5)
    }

    /// The point at `t` along it, 0 at its start and 1 at its end.
    pub fn at(&self, t: f32) -> Point<f32> {
        let count = self.segments.len();
        if count == 0 {
            return point(0.0, 0.0);
        }
        let spread = (t.clamp(0.0, 1.0) * count as f32).min(count as f32 - f32::EPSILON);
        let ix = (spread.floor() as usize).min(count - 1);
        let (a, c, b) = self.segments[ix];
        along(a, c, b, spread - ix as f32)
    }

    /// A box it stays inside: left, top, right, bottom.
    pub fn hull(&self) -> (f32, f32, f32, f32) {
        self.segments
            .iter()
            .flat_map(|(a, c, b)| [*a, *c, *b])
            .fold(
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
            )
    }

    /// How far `at` is from it.
    pub fn distance(&self, at: Point<f32>) -> f32 {
        /// How finely each segment is walked.
        const STEPS: usize = 16;
        let mut nearest = f32::MAX;
        for &(a, c, b) in &self.segments {
            let mut last = a;
            for step in 1..=STEPS {
                let next = along(a, c, b, step as f32 / STEPS as f32);
                nearest = nearest.min(to_segment(at, last, next));
                last = next;
            }
        }
        nearest
    }
}

/// Two quadratic halves bending out of each end: the spec's edge.
pub fn curve(ends: &Ends) -> Path {
    let ((from, from_out), (to, to_out)) = ends.anchors();
    let reach = (to.x - from.x).abs().max((to.y - from.y).abs()) / 2.0;
    let c0 = point(from.x + from_out.x * reach, from.y + from_out.y * reach);
    let c1 = point(to.x + to_out.x * reach, to.y + to_out.y * reach);
    let mid = point((c0.x + c1.x) / 2.0, (c0.y + c1.y) / 2.0);
    let (from_arrow, to_arrow) = ends.arrows();
    Path {
        segments: vec![(from, c0, mid), (mid, c1, to)],
        from_out,
        to_out,
        from_arrow,
        to_arrow,
    }
}

/// A straight line between the anchors.
pub fn line(ends: &Ends) -> Path {
    let ((from, from_out), (to, to_out)) = ends.anchors();
    let mid = point((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    let (from_arrow, to_arrow) = ends.arrows();
    Path {
        segments: vec![(from, mid, to)],
        from_out,
        to_out,
        from_arrow,
        to_arrow,
    }
}

/// A point along a quadratic.
fn along(a: Point<f32>, c: Point<f32>, b: Point<f32>, t: f32) -> Point<f32> {
    let u = 1.0 - t;
    point(
        u * u * a.x + 2.0 * u * t * c.x + t * t * b.x,
        u * u * a.y + 2.0 * u * t * c.y + t * t * b.y,
    )
}

/// How far `p` is from the segment `a`–`b`.
fn to_segment(p: Point<f32>, a: Point<f32>, b: Point<f32>) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
    };
    ((p.x - a.x - t * dx).powi(2) + (p.y - a.y - t * dy).powi(2)).sqrt()
}
