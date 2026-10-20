//! Geometry shared by the layouts and the drawing: points, sizes, the sides
//! of a box, and the direction a graph flows in.

use std::ops::{Add, AddAssign, Mul, Neg, Sub};

/// A position, or a vector between two.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// The coordinate along `axis`.
    pub fn along(self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
        }
    }

    pub fn along_mut(&mut self, axis: Axis) -> &mut f64 {
        match axis {
            Axis::X => &mut self.x,
            Axis::Y => &mut self.y,
        }
    }

    /// Halfway between this point and `other`.
    pub fn midpoint(self, other: Point) -> Point {
        Point::new((self.x + other.x) / 2.0, (self.y + other.y) / 2.0)
    }

    /// Whether `other` is within 0.01 of this point on both axes.
    pub fn near(self, other: Point) -> bool {
        (self.x - other.x).abs() < 0.01 && (self.y - other.y).abs() < 0.01
    }
}

impl Add for Point {
    type Output = Point;

    fn add(self, other: Point) -> Point {
        Point::new(self.x + other.x, self.y + other.y)
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, other: Point) {
        self.x += other.x;
        self.y += other.y;
    }
}

impl Sub for Point {
    type Output = Point;

    fn sub(self, other: Point) -> Point {
        Point::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f64> for Point {
    type Output = Point;

    fn mul(self, k: f64) -> Point {
        Point::new(self.x * k, self.y * k)
    }
}

impl Neg for Point {
    type Output = Point;

    fn neg(self) -> Point {
        Point::new(-self.x, -self.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

impl Size {
    pub const fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }

    /// The same box turned a quarter: `w` and `h` swapped.
    pub fn turned(self) -> Self {
        Self::new(self.h, self.w)
    }
}

/// One of the two axes of the drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    pub fn other(self) -> Self {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

/// A side of a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Side::Top => Side::Bottom,
            Side::Right => Side::Left,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
        }
    }

    /// Top and bottom.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// The way a graph's ranks run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    TopToBottom,
    BottomToTop,
    LeftToRight,
    RightToLeft,
}

impl Direction {
    /// The direction Mermaid's `TB`, `BT`, `LR` or `RL` names; top to bottom
    /// for anything else.
    pub fn from_name(name: &str) -> Self {
        match name {
            "BT" => Direction::BottomToTop,
            "LR" => Direction::LeftToRight,
            "RL" => Direction::RightToLeft,
            _ => Direction::TopToBottom,
        }
    }

    /// Top to bottom or bottom to top.
    pub fn is_vertical(self) -> bool {
        matches!(self, Direction::TopToBottom | Direction::BottomToTop)
    }

    /// Bottom to top or right to left.
    pub fn is_reversed(self) -> bool {
        matches!(self, Direction::BottomToTop | Direction::RightToLeft)
    }

    /// The axis the ranks follow each other along.
    pub fn axis(self) -> Axis {
        if self.is_vertical() { Axis::Y } else { Axis::X }
    }

    /// The side of a box the flow enters it by.
    pub fn start_side(self) -> Side {
        match self {
            Direction::TopToBottom => Side::Top,
            Direction::BottomToTop => Side::Bottom,
            Direction::LeftToRight => Side::Left,
            Direction::RightToLeft => Side::Right,
        }
    }
}

/// A point of an edge's route, and the axis the route runs along into it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoutePoint {
    pub at: Point,
    pub axis: Axis,
}

impl RoutePoint {
    pub const fn new(at: Point, axis: Axis) -> Self {
        Self { at, axis }
    }
}
