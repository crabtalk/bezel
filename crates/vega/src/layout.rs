//! vega-view-transforms' layout: axes placed around a view and their titles
//! beside their labels, and overlapping labels hidden.

use crate::{
    Error,
    scale::Scale,
    scene::{Bounds, Item, Mark},
    spec::Overlap,
    text,
    value::Value,
    view::compare,
};
use serde_json::Value as Json;

/// `ViewLayout`'s `layoutGroup` for `group`: its axes placed. Legends and
/// titles are not laid out. Answers the bounds of everything in the view,
/// from its origin.
pub fn view(group: &mut Item) -> Bounds {
    let width = group.or_zero("width").max(0.0);
    let height = group.or_zero("height").max(0.0);
    let mut view = Bounds::default();
    view.set(0.0, 0.0, width, height);
    let (mut x_bounds, mut y_bounds) = (view, view);
    for mark in &mut group.items {
        match mark.role.as_str() {
            "axis" => {
                let y_axis = matches!(mark.items[0].get("orient"), Value::String(o) if &**o == "left" || &**o == "right");
                let b = axis(mark, width, height);
                if y_axis {
                    x_bounds.union(&b);
                } else {
                    y_bounds.union(&b);
                }
            }
            "frame" | "scope" | "row-header" | "row-footer" | "row-title" | "column-header"
            | "column-footer" | "column-title" => {
                x_bounds.union(&mark.bounds);
                y_bounds.union(&mark.bounds);
            }
            _ => {
                view.union(&mark.bounds);
            }
        }
    }
    view.union(&x_bounds).union(&y_bounds);
    if group.get("clip").truthy() {
        view.set(0.0, 0.0, width, height);
    }
    view
}

/// The indices of an axis group's ticks, labels and title marks, as its
/// datum says which parts it draws.
fn indices(datum: &Value) -> [Option<usize>; 3] {
    let part = |name: &str| datum.get(&name.into()).is_ok_and(|v| v.truthy());
    let mut index = usize::from(part("grid"));
    let mut next = |on: bool| {
        on.then(|| {
            index += 1;
            index - 1
        })
    };
    let ticks = next(part("ticks"));
    let labels = next(part("labels"));
    let title = part("title").then_some(index + usize::from(part("domain")));
    [ticks, labels, title]
}

/// `multiLineOffset`: how far a text's lines below the first reach.
fn multi_line_offset(item: &Item) -> f64 {
    match text::lines(item) {
        Ok(lines) => (lines.len() - 1) as f64 * text::line_height(item),
        Err(_) => 0.0,
    }
}

/// `axisLayout`: the axis group placed on its side of a `width` by
/// `height` view, its title beside its labels; the axis mark's bounds.
fn axis(mark: &mut Mark, width: f64, height: f64) -> Bounds {
    let item = &mut mark.items[0];
    let delta = item.number("translate").unwrap_or(0.5);
    let orient = item.get("orient").to_string();
    let [ticks, labels, title_index] = indices(&item.datum);
    let range = item.get("range").to_number();
    let offset = item.get("offset").to_number();
    let position = item.or_zero("position");
    let min_extent = item.get("minExtent").to_number();
    let max_extent = item.get("maxExtent").to_number();
    let title_padding = item.get("titlePadding").to_number();
    let mut bounds = Bounds::default();
    for i in [ticks, labels].into_iter().flatten() {
        bounds.union(&item.items[i].bounds);
    }
    let clamp = |v: f64| min_extent.max(max_extent.min(v));
    let mut title = title_index;
    let (x, y);
    let mut place_title =
        |item: &mut Item, s: f64, dl: f64, y_axis: bool, sign: f64, bounds: &mut Bounds| {
            if let Some(i) = title.take() {
                let title_mark = &mut item.items[i];
                let t = &mut title_mark.items[0];
                if t.get("auto").truthy() {
                    let v = sign * (s + dl + title_padding);
                    let (mut dx, mut dy) = (0.0, 0.0);
                    if y_axis {
                        dx = t.or_zero("x") - v;
                        t.set("x", Value::Number(v));
                    } else {
                        dy = t.or_zero("y") - v;
                        t.set("y", Value::Number(v));
                    }
                    t.bounds.translate(-dx, -dy);
                    title_mark.bounds = t.bounds;
                }
                bounds.union(&item.items[i].items[0].bounds);
            }
        };
    let dl = title_index.map_or(0.0, |i| multi_line_offset(&item.items[i].items[0]));
    match orient.as_str() {
        "top" => {
            x = position;
            y = -offset;
            let s = clamp(-bounds.y1);
            bounds.add(0.0, -s).add(range, 0.0);
            place_title(item, s, dl, false, -1.0, &mut bounds);
        }
        "left" => {
            x = -offset;
            y = position;
            let s = clamp(-bounds.x1);
            bounds.add(-s, 0.0).add(0.0, range);
            place_title(item, s, dl, true, -1.0, &mut bounds);
        }
        "right" => {
            x = width + offset;
            y = position;
            let s = clamp(bounds.x2);
            bounds.add(0.0, 0.0).add(s, range);
            place_title(item, s, dl, true, 1.0, &mut bounds);
        }
        "bottom" => {
            x = position;
            y = height + offset;
            let s = clamp(bounds.y2);
            bounds.add(0.0, 0.0).add(range, s);
            place_title(item, s, 0.0, false, 1.0, &mut bounds);
        }
        _ => {
            x = item.or_zero("x");
            y = item.or_zero("y");
        }
    }
    bounds.translate(x, y);
    if item.get("stroke").truthy() {
        let width = item.number("strokeWidth").unwrap_or(1.0);
        bounds.expand(width / 2.0);
    }
    item.set("x", Value::Number(x + delta));
    item.set("y", Value::Number(y + delta));
    item.bounds = bounds;
    mark.bounds = bounds;
    bounds
}

/// vega-scale's `tickCount` with a minimum step between ticks.
pub fn tick_count(scale: &Scale, count: f64, min_step: f64) -> f64 {
    let domain = scale.domain();
    let (Some(first), Some(last)) = (domain.first(), domain.last()) else {
        return count;
    };
    let (a, b) = (first.to_number(), last.to_number());
    let (lo, hi) = (a.min(b), a.max(b));
    let steps = ((hi - lo) / min_step).floor();
    let mut count = count.min(
        if steps == 0.0 || steps.is_nan() {
            1.0
        } else {
            steps
        } + 1.0,
    );
    if matches!(scale, Scale::Linear(_)) && lo < hi {
        while count > 1.0 && d3::array::tick_step(lo, hi, count) < min_step {
            count -= 1.0;
        }
    }
    count
}

/// Whether `a` and `b` overlap, closer than `sep` pixels.
fn intersect(a: &Bounds, b: &Bounds, sep: f64) -> bool {
    sep > (b.x1 - a.x2)
        .max(a.x1 - b.x2)
        .max(b.y1 - a.y2)
        .max(a.y1 - b.y2)
}

/// vega-view-transforms' `Overlap`: items whose bounds overlap their
/// neighbours' hidden, by `parity` or `greedy`, keeping the first and last.
pub fn overlap<'s>(
    mark: &mut Mark,
    def: &Overlap,
    scale: &dyn Fn(&str) -> Option<&'s Scale>,
) -> Result<(), Error> {
    if mark.items.is_empty() {
        return Ok(());
    }
    let method = match &def.method {
        Json::String(m) if m == "greedy" => "greedy",
        m if !crate::guide::truthy(m) => return Ok(()),
        _ => "parity",
    };
    let sep = def
        .separation
        .as_ref()
        .and_then(Json::as_f64)
        .unwrap_or(0.0);
    let mut source: Vec<usize> = (0..mark.items.len())
        .filter(|&i| {
            let b = &mark.items[i].bounds;
            b.width() > 1.0 && b.height() > 1.0
        })
        .collect();
    if source.is_empty() {
        return Ok(());
    }
    if let Some(order) = &def.order {
        let field = order
            .strip_prefix("datum.")
            .ok_or_else(|| Error::Unsupported(format!("overlap order {order}")))?;
        let key = |i: usize| mark.items[i].datum.get(&field.into()).unwrap_or_default();
        source.sort_by(|&a, &b| compare(&key(a), &key(b)));
    }
    for &i in &source {
        mark.items[i].set("opacity", Value::Number(1.0));
    }
    let bounds = |mark: &Mark, i: usize| mark.items[i].bounds;
    let has_overlap = |mark: &Mark, items: &[usize]| {
        items
            .windows(2)
            .any(|w| intersect(&bounds(mark, w[0]), &bounds(mark, w[1]), sep))
    };
    let mut items = source.clone();
    if items.len() >= 3 && has_overlap(mark, &items) {
        loop {
            items = match method {
                "greedy" => {
                    let mut kept: Vec<usize> = Vec::new();
                    for (n, &i) in items.iter().enumerate() {
                        if n == 0
                            || !intersect(
                                &bounds(mark, *kept.last().unwrap()),
                                &bounds(mark, i),
                                sep,
                            )
                        {
                            kept.push(i);
                        } else {
                            mark.items[i].set("opacity", Value::Number(0.0));
                        }
                    }
                    kept
                }
                _ => {
                    let mut kept = Vec::new();
                    for (n, &i) in items.iter().enumerate() {
                        if n % 2 == 1 {
                            mark.items[i].set("opacity", Value::Number(0.0));
                        } else {
                            kept.push(i);
                        }
                    }
                    kept
                }
            };
            if !(items.len() >= 3 && has_overlap(mark, &items)) {
                break;
            }
        }
        let last = *source.last().unwrap();
        if items.len() < 3 && !mark.items[last].get("opacity").truthy() {
            if items.len() > 1 {
                let peek = *items.last().unwrap();
                mark.items[peek].set("opacity", Value::Number(0.0));
            }
            mark.items[last].set("opacity", Value::Number(1.0));
        }
    }
    if let Some(Json::Object(bound)) = &def.bound {
        let tolerance = bound
            .get("tolerance")
            .map(Value::from)
            .map(|t| t.to_number());
        let scale = bound.get("scale").and_then(Json::as_str).and_then(scale);
        if let (Some(scale), Some(tolerance)) = (scale, tolerance)
            && tolerance >= 0.0
        {
            let range = scale.range();
            let (r0, r1) = (range[0], range[range.len() - 1]);
            let mut b = Bounds::default();
            match bound.get("orient").and_then(Json::as_str) {
                Some("top" | "bottom") => b.set(r0, f64::NEG_INFINITY, r1, f64::INFINITY),
                _ => b.set(f64::NEG_INFINITY, r0, f64::INFINITY, r1),
            };
            b.expand(if tolerance == 0.0 { 1.0 } else { tolerance });
            for &i in &source {
                if !b.encloses(&mark.items[i].bounds) {
                    mark.items[i].set("opacity", Value::Number(0.0));
                }
            }
        }
    }
    let mut union = Bounds::default();
    for &i in &source {
        if mark.items[i].get("opacity").truthy() {
            union.union(&mark.items[i].bounds);
        }
    }
    mark.bounds = union;
    Ok(())
}
