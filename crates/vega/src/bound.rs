//! vega-scenegraph's item bounds, by mark type.

use crate::{
    scene::{Bounds, Item, Mark},
    text,
    value::Value,
};
use std::f64::consts::{FRAC_PI_2, SQRT_2};

/// `boundStroke`: the bounds grown by half the stroke's width.
fn stroke(bounds: &mut Bounds, item: &Item) {
    let opacity = item.get("opacity");
    let stroke_opacity = item.get("strokeOpacity");
    let zero = |v: &Value| matches!(v, Value::Number(n) if *n == 0.0);
    if item.get("stroke").truthy() && !zero(opacity) && !zero(stroke_opacity) {
        let width = item.number("strokeWidth").unwrap_or(1.0);
        let square = matches!(item.get("strokeCap"), Value::String(c) if &**c == "square");
        bounds.expand(if square { SQRT_2 } else { 1.0 } * width / 2.0);
    }
}

/// The bounds of `item`, a `marktype` item.
pub fn item(marktype: &str, item: &Item) -> Bounds {
    let mut bounds = Bounds::default();
    match marktype {
        "rect" => {
            let (x, y) = (item.or_zero("x"), item.or_zero("y"));
            let or_zero = |v: f64| if v.is_nan() { 0.0 } else { v };
            bounds.set(
                x,
                y,
                or_zero(x + item.get("width").to_number()),
                or_zero(y + item.get("height").to_number()),
            );
            stroke(&mut bounds, item);
        }
        "rule" => {
            let (x1, y1) = (item.or_zero("x"), item.or_zero("y"));
            bounds.set(
                x1,
                y1,
                item.number("x2").unwrap_or(x1),
                item.number("y2").unwrap_or(y1),
            );
            stroke(&mut bounds, item);
        }
        "text" => text(&mut bounds, item),
        "group" => {
            let clip = item.get("clip").truthy();
            if !clip {
                for mark in &item.items {
                    bounds.union(&mark.bounds);
                }
            }
            let (width, height) = (item.or_zero("width"), item.or_zero("height"));
            if (clip || width != 0.0 || height != 0.0) && !item.get("noBound").truthy() {
                bounds.add(0.0, 0.0).add(width, height);
            }
            stroke(&mut bounds, item);
            bounds.translate(item.or_zero("x"), item.or_zero("y"));
        }
        other => unreachable!("a {other} mark is not built"),
    }
    bounds
}

fn text(bounds: &mut Bounds, item: &Item) {
    let mut h = text::font_size(item);
    let (mut x, mut y) = (item.or_zero("x"), item.or_zero("y"));
    let r = item.or_zero("radius");
    if r != 0.0 {
        let t = item.or_zero("theta") - FRAC_PI_2;
        x += r * t.cos();
        y += r * t.sin();
    }
    let mut dx = item.or_zero("dx");
    let dy = item.or_zero("dy") + text::offset(item) - d3::format::number::round(0.8 * h);
    let w = match text::lines(item) {
        Ok(lines) => {
            h += text::line_height(item) * (lines.len() - 1) as f64;
            lines
                .iter()
                .fold(0.0, |w: f64, line| w.max(text::width(item, line)))
        }
        Err(line) => text::width(item, &line),
    };
    match item.get("align") {
        Value::String(a) if &**a == "center" => dx -= w / 2.0,
        Value::String(a) if &**a == "right" => dx -= w,
        _ => {}
    }
    let (x1, y1) = (dx + x, dy + y);
    bounds.set(x1, y1, x1 + w, y1 + h);
    let angle = item.or_zero("angle");
    if angle != 0.0 {
        bounds.rotate(angle.to_radians(), x, y);
    }
}

/// `boundMark`: the union of the items' bounds, each item's own kept.
pub fn mark(mark: &mut Mark) {
    let mut bounds = Bounds::default();
    for item in &mut mark.items {
        item.bounds = self::item(&mark.marktype, item);
        bounds.union(&item.bounds);
    }
    mark.bounds = bounds;
}
