//! Pele's `diagrams/sequence/actors.ts`.

use crate::{
    svg::{
        builder::{label_svg, num},
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
const KINDS: [&str; 8] = [
    "participant",
    "actor",
    "boundary",
    "control",
    "entity",
    "database",
    "collections",
    "queue",
];
const FIGURES: [&str; 5] = ["actor", "boundary", "control", "entity", "database"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorSize {
    pub w: f64,
    pub h: f64,
}

pub fn actor_kind(kind: &str) -> &'static str {
    KINDS
        .into_iter()
        .find(|known| *known == kind)
        .unwrap_or("participant")
}

/// A figure stands above its label. The other kinds hold the label inside.
pub fn is_figure(kind: &str) -> bool {
    FIGURES.contains(&kind)
}

pub fn actor_size(
    kind: &str,
    label: &Label,
    min_w: f64,
    min_h: f64,
    padding_x: Option<f64>,
) -> ActorSize {
    let padding_x = padding_x.unwrap_or(PAD_X);
    if is_figure(kind) {
        return ActorSize {
            w: 40f64.max(label.width + 8.0),
            h: GLYPH
                + if label.height > 0.0 {
                    GLYPH_GAP + label.height
                } else {
                    0.0
                },
        };
    }
    let extra = match kind {
        "collections" => STACK,
        "queue" => 2.0 * CAP,
        _ => 0.0,
    };
    ActorSize {
        w: min_w.max(label.width + 2.0 * padding_x) + extra,
        h: min_h.max(label.height + 2.0 * PAD_Y) + if kind == "collections" { STACK } else { 0.0 },
    }
}

fn figure(kind: &str, cx: f64, top: f64) -> String {
    let x = num(cx);
    let y = |dy: f64| num(top + dy);
    match kind {
        "actor" => format!(
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
        "boundary" => format!(
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
        "control" => format!(
            "<circle cx=\"{x}\" cy=\"{}\" r=\"11\"{SHAPE}/><path d=\"M{},{}L{},{}L{},{}\"{LINE}/>",
            y(21.0),
            num(cx + 5.0),
            y(5.0),
            num(cx - 1.0),
            y(10.0),
            num(cx + 5.0),
            y(15.0)
        ),
        "entity" => format!(
            "<circle cx=\"{x}\" cy=\"{}\" r=\"11\"{SHAPE}/><path d=\"M{},{}H{}\"{LINE}/>",
            y(15.0),
            num(cx - 11.0),
            y(30.0),
            num(cx + 11.0)
        ),
        _ => {
            let l = num(cx - 14.0);
            let r = num(cx + 14.0);
            format!(
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

/// Draws a participant whose bounding box has its top center at (cx, top).
pub fn draw_actor(kind: &str, cx: f64, top: f64, size: ActorSize, label: &Label) -> String {
    let p = crate::PREFIX;
    let ActorSize { w, h } = size;
    let text = format!(" class=\"{p}-label\"");
    if is_figure(kind) {
        return figure(kind, cx, top)
            + &label_svg(
                label,
                cx,
                top + GLYPH + GLYPH_GAP + label.height / 2.0,
                &text,
            );
    }
    let left = cx - w / 2.0;
    if kind == "collections" {
        let bw = w - STACK;
        let bh = h - STACK;
        let back = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE}/>",
            num(left + STACK),
            num(top),
            num(bw),
            num(bh)
        );
        let front = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE}/>",
            num(left),
            num(top + STACK),
            num(bw),
            num(bh)
        );
        return back + &front + &label_svg(label, left + bw / 2.0, top + STACK + bh / 2.0, &text);
    }
    if kind == "queue" {
        let l = num(left + CAP);
        let r = num(left + w - CAP);
        let b = num(top + h);
        let ry = num(h / 2.0);
        let t = num(top);
        return format!(
            "<path d=\"M{l},{t}H{r}A{CAP},{ry} 0 0 1 {r},{b}H{l}A{CAP},{ry} 0 0 1 {l},{t}Z\"{SHAPE}/><path d=\"M{r},{t}A{CAP},{ry} 0 0 0 {r},{b}\"{LINE}/>",
        ) + &label_svg(label, cx - CAP / 2.0, top + h / 2.0, &text);
    }
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{SHAPE}/>",
        num(left),
        num(top),
        num(w),
        num(h)
    ) + &label_svg(label, cx, top + h / 2.0, &text)
}
