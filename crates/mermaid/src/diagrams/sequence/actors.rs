//! Pele's `diagrams/sequence/actors.ts`.

use crate::geom::{Point, Size};
use std::fmt;

use crate::{
    svg::{
        builder::{fragment, num},
        theme::RADIUS,
    },
    text::Label,
};

const PAD_X: f64 = 16.0;
const PAD_Y: f64 = 10.0;
const GLYPH: f64 = 34.0;
const GLYPH_GAP: f64 = 4.0;
const STACK: f64 = 5.0;
const CAP: f64 = 6.0;
const SHAPE: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE: &str = " fill=\"none\" stroke=\"var(--_b)\"";

/// How a participant is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorKind {
    Participant,
    Actor,
    Boundary,
    Control,
    Entity,
    Database,
    Collections,
    Queue,
}

impl ActorKind {
    /// The kind `name` names, a box for a name there is none of.
    pub fn from_name(name: &str) -> Self {
        match name {
            "actor" => ActorKind::Actor,
            "boundary" => ActorKind::Boundary,
            "control" => ActorKind::Control,
            "entity" => ActorKind::Entity,
            "database" => ActorKind::Database,
            "collections" => ActorKind::Collections,
            "queue" => ActorKind::Queue,
            _ => ActorKind::Participant,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ActorKind::Participant => "participant",
            ActorKind::Actor => "actor",
            ActorKind::Boundary => "boundary",
            ActorKind::Control => "control",
            ActorKind::Entity => "entity",
            ActorKind::Database => "database",
            ActorKind::Collections => "collections",
            ActorKind::Queue => "queue",
        }
    }

    /// A figure stands above its label. The other kinds hold the label
    /// inside.
    pub fn is_figure(self) -> bool {
        matches!(
            self,
            ActorKind::Actor
                | ActorKind::Boundary
                | ActorKind::Control
                | ActorKind::Entity
                | ActorKind::Database
        )
    }

    /// The size of the participant around `label`, at least `min` unless it
    /// is a figure.
    pub fn size(self, label: &Label, min: Size, padding_x: Option<f64>) -> Size {
        let padding_x = padding_x.unwrap_or(PAD_X);
        if self.is_figure() {
            return Size::new(
                40f64.max(label.width + 8.0),
                GLYPH
                    + if label.height > 0.0 {
                        GLYPH_GAP + label.height
                    } else {
                        0.0
                    },
            );
        }
        let extra = match self {
            ActorKind::Collections => STACK,
            ActorKind::Queue => 2.0 * CAP,
            _ => 0.0,
        };
        Size::new(
            min.w.max(label.width + 2.0 * padding_x) + extra,
            min.h.max(label.height + 2.0 * PAD_Y)
                + if self == ActorKind::Collections {
                    STACK
                } else {
                    0.0
                },
        )
    }

    /// Draws the figure whose glyph has its top centre at `at`.
    fn figure(self, f: &mut fmt::Formatter<'_>, at: Point) -> fmt::Result {
        let Point { x: cx, y: top } = at;
        let x = num(cx);
        let y = |dy: f64| num(top + dy);
        match self {
            ActorKind::Actor => write!(
                f,
                "<circle cx=\"{x}\" cy=\"{}\" r=\"6\"{SHAPE}/><path d=\"M{x},{}V{}M{},{}H{}M{},{}L{x},{}L{},{}\"{LINE}/>",
                y(7.0),
                y(13.0),
                y(24.0),
                num(cx - 10.0),
                y(17.0),
                num(cx + 10.0),
                num(cx - 8.0),
                y(34.0),
                y(24.0),
                num(cx + 8.0),
                y(34.0)
            ),
            ActorKind::Boundary => write!(
                f,
                "<path d=\"M{},{}V{}M{},{}H{}\"{LINE}/><circle cx=\"{}\" cy=\"{}\" r=\"11\"{SHAPE}/>",
                num(cx - 19.0),
                y(6.0),
                y(28.0),
                num(cx - 19.0),
                y(17.0),
                num(cx - 8.0),
                num(cx + 3.0),
                y(17.0)
            ),
            ActorKind::Control => write!(
                f,
                "<circle cx=\"{x}\" cy=\"{}\" r=\"11\"{SHAPE}/><path d=\"M{},{}L{},{}L{},{}\"{LINE}/>",
                y(21.0),
                num(cx + 5.0),
                y(5.0),
                num(cx - 1.0),
                y(10.0),
                num(cx + 5.0),
                y(15.0)
            ),
            ActorKind::Entity => write!(
                f,
                "<circle cx=\"{x}\" cy=\"{}\" r=\"11\"{SHAPE}/><path d=\"M{},{}H{}\"{LINE}/>",
                y(15.0),
                num(cx - 11.0),
                y(30.0),
                num(cx + 11.0)
            ),
            _ => {
                let l = num(cx - 14.0);
                let r = num(cx + 14.0);
                write!(
                    f,
                    "<path d=\"M{l},{}V{}A14,5 0 0 0 {r},{}V{}A14,5 0 0 0 {l},{}Z\"{SHAPE}/><path d=\"M{l},{}A14,5 0 0 0 {r},{}\"{LINE}/>",
                    y(7.0),
                    y(27.0),
                    y(27.0),
                    y(7.0),
                    y(7.0),
                    y(7.0),
                    y(7.0)
                )
            }
        }
    }

    /// Draws the participant of `size` whose bounding box has its top centre
    /// at `at`.
    pub fn draw(self, at: Point, size: Size, label: &Label) -> impl fmt::Display + '_ {
        fragment(move |f| {
            let p = crate::PREFIX;
            let Point { x: cx, y: top } = at;
            let Size { w, h } = size;
            let text = format!(" class=\"{p}-label\"");
            if self.is_figure() {
                self.figure(f, at)?;
                return write!(
                    f,
                    "{}",
                    label.svg(
                        Point::new(cx, top + GLYPH + GLYPH_GAP + label.height / 2.0),
                        &text
                    )
                );
            }
            let left = cx - w / 2.0;
            if self == ActorKind::Collections {
                let bw = w - STACK;
                let bh = h - STACK;
                return write!(
                    f,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE}/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE}/>{}",
                    num(left + STACK),
                    num(top),
                    num(bw),
                    num(bh),
                    num(left),
                    num(top + STACK),
                    num(bw),
                    num(bh),
                    label.svg(Point::new(left + bw / 2.0, top + STACK + bh / 2.0), &text)
                );
            }
            if self == ActorKind::Queue {
                let l = num(left + CAP);
                let r = num(left + w - CAP);
                let b = num(top + h);
                let ry = num(h / 2.0);
                let t = num(top);
                return write!(
                    f,
                    "<path d=\"M{l},{t}H{r}A{CAP},{ry} 0 0 1 {r},{b}H{l}A{CAP},{ry} 0 0 1 {l},{t}Z\"{SHAPE}/><path d=\"M{r},{t}A{CAP},{ry} 0 0 0 {r},{b}\"{LINE}/>{}",
                    label.svg(Point::new(cx - CAP / 2.0, top + h / 2.0), &text)
                );
            }
            write!(
                f,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE}/>{}",
                num(left),
                num(top),
                num(w),
                num(h),
                label.svg(Point::new(cx, top + h / 2.0), &text)
            )
        })
    }
}
