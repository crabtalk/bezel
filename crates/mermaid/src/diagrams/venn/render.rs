//! Pele's `diagrams/venn/render.ts`.

use crate::geom::Point;
use crate::js::JsValue;
use crate::svg::builder::EscText;
use indexmap::{IndexMap, IndexSet};
use std::fmt::Write as _;

use super::{
    db::{VennDb, normalize_text},
    layout::{Circle, Spot, find_spot, layout_circles, region_path},
    types::VennSubset,
};
use crate::{
    Config, Error, Rendered,
    config::Value,
    diagrams::Render,
    js,
    svg::{
        builder::{TITLE, num},
        root::{Accessible, svg_document},
        theme::{ResolvedStyle, resolve_style, series_color},
    },
    text::{Label, Style},
};
use crate::{Ctx, Host};

const RADIUS: f64 = 100.0;
const MAX_GROWTH: f64 = 1.6;

/// A line written in a region. The id is that of a text.
struct Line {
    id: Option<String>,
    raw: String,
    size: f64,
    base: u8,
    label: Label,
}

struct Region {
    sets: Vec<String>,
    inside: Vec<usize>,
    spot: Spot,
    /// What is written in the region: its label first, then its texts.
    lines: Vec<Line>,
    style: ResolvedStyle,
}

/// One key for a list of set ids. Ids may hold any character, so they are not
/// simply joined.
fn key_of(ids: &[String]) -> String {
    if ids.len() == 1 {
        ids[0].clone()
    } else {
        Value::Array(ids.iter().cloned().map(Value::String).collect()).to_json()
    }
}

impl Render for VennDb {
    fn render(&self, _config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let tiny = js::round(size * 0.8125);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let p = crate::PREFIX;

        let circles = layout_circles(&self.subsets);
        let mut index: IndexMap<&str, usize> = IndexMap::new();
        for (i, c) in circles.iter().enumerate() {
            index.insert(&c.id, i);
        }

        // Styles for the same targets add up, later declarations winning.
        let mut styles: IndexMap<String, IndexMap<String, String>> = IndexMap::new();
        for style in &self.styles {
            let merged = styles.entry(key_of(&style.targets)).or_default();
            for (property, value) in &style.styles {
                merged.insert(property.clone(), value.clone());
            }
        }
        let style_of = |key: &str| -> ResolvedStyle {
            let Some(declared) = styles.get(key) else {
                return ResolvedStyle::default();
            };
            let list: Vec<String> = declared
                .iter()
                .map(|(property, value)| format!("{property}:{value}"))
                .collect();
            resolve_style(&list)
        };

        // Circles that touch each circle; only those can cut into a region of it.
        let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); circles.len()];
        for a in 0..circles.len() {
            for b in a + 1..circles.len() {
                if circles[a].center.distance(circles[b].center) < circles[a].r + circles[b].r {
                    neighbors[a].push(b);
                    neighbors[b].push(a);
                }
            }
        }

        // One region for each set and each stated overlap; a later statement
        // about the same sets replaces an earlier one.
        let mut stated: IndexMap<String, &VennSubset> = IndexMap::new();
        for subset in &self.subsets {
            stated.insert(key_of(&subset.sets), subset);
        }
        let mut regions: IndexMap<String, Region> = IndexMap::new();
        for (key, subset) in stated {
            let sets: Vec<String> = subset
                .sets
                .iter()
                .collect::<IndexSet<_>>()
                .into_iter()
                .cloned()
                .collect();
            let inside: Vec<usize> = sets.iter().map(|id| index[id.as_str()]).collect();
            let mut smallest = inside[0];
            for &i in &inside {
                if circles[i].r < circles[smallest].r {
                    smallest = i;
                }
            }
            let style = style_of(&key);
            let text = subset
                .label
                .clone()
                .or_else(|| (subset.sets.len() == 1).then(|| subset.sets[0].clone()));
            let base = (if style.bold { Style::BOLD } else { 0 })
                | (if style.italic { Style::ITALIC } else { 0 });
            let outside: Vec<usize> = neighbors[smallest]
                .iter()
                .copied()
                .filter(|i| !inside.contains(i))
                .collect();
            let mut spot = find_spot(&circles, &inside, &outside);
            // A region that other circles cover, or all but cover, is written
            // across them instead.
            if spot.margin < circles[smallest].r * 0.2 {
                spot = find_spot(&circles, &inside, &[]);
            }
            let mut region = Region {
                sets,
                inside,
                spot,
                lines: Vec::new(),
                style,
            };
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                let font_size = if region.sets.len() == 1 { size } else { small };
                let label = Label::layout(Some(&text), false, measure, font_size, 4000.0, base);
                region.lines.push(Line {
                    id: None,
                    raw: text,
                    size: font_size,
                    base,
                    label,
                });
            }
            regions.insert(key, region);
        }
        for node in &self.text_nodes {
            let raw = node.label.clone().unwrap_or_else(|| node.id.clone());
            if let Some(region) = regions.get_mut(&key_of(&node.sets)) {
                let label = Label::layout(Some(&raw), false, measure, tiny, 4000.0, 0);
                region.lines.push(Line {
                    id: Some(node.id.clone()),
                    raw,
                    size: tiny,
                    base: 0,
                    label,
                });
            }
        }

        // Lengths so far are in units of the layout. The scale gives the largest
        // circle a set radius, and grows, within limits, until every region holds
        // what is written in it.
        let mut largest = 0.0;
        for c in &circles {
            if c.r > largest {
                largest = c.r;
            }
        }
        let mut scale = if largest > 0.0 { RADIUS / largest } else { 1.0 };
        let mut growth = 1f64;
        for region in regions.values() {
            if region.spot.margin <= 0.0 {
                continue;
            }
            let mut width = 0f64;
            let mut height = 0.0;
            for line in &region.lines {
                width = width.max(line.label.width);
                height += line.label.height;
            }
            let need = width.hypot(height) / 2.0 + 4.0;
            growth = growth.max((need / (region.spot.margin * scale)).min(MAX_GROWTH));
        }
        scale *= growth;

        // What is still too wide for its region is wrapped to it.
        for region in regions.values_mut() {
            let room = (region.spot.margin * scale * 1.8).max(72.0);
            for line in &mut region.lines {
                if line.label.width > room {
                    line.label =
                        Label::layout(Some(&line.raw), false, measure, line.size, room, line.base);
                }
            }
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for c in &circles {
            min_x = min_x.min((c.center.x - c.r) * scale);
            min_y = min_y.min((c.center.y - c.r) * scale);
            max_x = max_x.max((c.center.x + c.r) * scale);
            max_y = max_y.max((c.center.y + c.r) * scale);
        }
        for region in regions.values() {
            let mut wide = 0f64;
            let mut tall = 0.0;
            for line in &region.lines {
                wide = wide.max(line.label.width);
                tall += line.label.height;
            }
            let spot = region.spot.at * scale;
            min_x = min_x.min(spot.x - wide / 2.0);
            max_x = max_x.max(spot.x + wide / 2.0);
            min_y = min_y.min(spot.y - tall / 2.0);
            max_y = max_y.max(spot.y + tall / 2.0);
        }
        if circles.is_empty() {
            min_x = 0.0;
            min_y = 0.0;
            max_x = 0.0;
            max_y = 0.0;
        }

        let title_text = self.title.as_deref().map(normalize_text);
        let title = Label::layout(
            title_text.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);
        let inner = (max_x - min_x).max(title.width);
        let offset = Point::new(
            pad + (inner - (max_x - min_x)) / 2.0 - min_x,
            pad + title_height - min_y,
        );
        let width = (inner + 2.0 * pad).ceil();
        let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
        let placed: Vec<Circle> = circles
            .iter()
            .map(|c| Circle {
                id: c.id.clone(),
                center: c.center * scale + offset,
                r: c.r * scale,
            })
            .collect();

        let mut sets = String::new();
        for (i, c) in placed.iter().enumerate() {
            let color = series_color(i as i64);
            let shape = regions
                .get(&c.id)
                .map_or("", |region| region.style.shape.as_str());
            let _ = write!(
                sets,
                "<circle class=\"{p}-venn-set\" data-id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{color}\" fill-opacity=\"0.2\" stroke=\"{color}\"{shape}/>",
                EscText(&c.id),
                num(c.center.x),
                num(c.center.y),
                num(c.r)
            );
        }

        let mut overlaps = String::new();
        let mut labels = String::new();
        let mut attrs = String::new();
        for region in regions.values() {
            let joined = region.sets.join("|");
            let id = EscText(&joined);
            // An overlap is only drawn when a style gives it a look of its own;
            // otherwise the circles show it.
            if region.sets.len() > 1 && !region.style.shape.is_empty() {
                let outline: Vec<Circle> =
                    region.inside.iter().map(|&i| placed[i].clone()).collect();
                let d = region_path(&outline, num);
                if !d.is_empty() {
                    let _ = write!(
                        overlaps,
                        "<path class=\"{p}-venn-overlap\" data-id=\"{id}\" d=\"{d}\" fill=\"none\"{}/>",
                        region.style.shape
                    );
                }
            }
            let mut total = 0.0;
            for line in &region.lines {
                total += line.label.height;
            }
            if total == 0.0 {
                continue;
            }
            let at = region.spot.at * scale + offset;
            let mut y = at.y - total / 2.0;
            let _ = write!(labels, "<g class=\"{p}-venn-region\" data-id=\"{id}\">");
            for line in &region.lines {
                attrs.clear();
                let _ = match &line.id {
                    None => {
                        let _ = write!(attrs, " class=\"{p}-label\"");
                        if line.size != size {
                            let _ = write!(attrs, " font-size=\"{}\"", js::Number(line.size));
                        }
                        write!(attrs, "{}", region.style.text)
                    }
                    Some(text_id) => write!(
                        attrs,
                        " class=\"{p}-venn-text\" data-id=\"{}\" font-size=\"{}\" fill=\"var(--_m)\"{}",
                        EscText(text_id),
                        js::Number(tiny),
                        style_of(text_id).text
                    ),
                };
                let _ = write!(
                    labels,
                    "{}",
                    line.label
                        .svg(Point::new(at.x, y + line.label.height / 2.0), &attrs)
                );
                y += line.label.height;
            }
            labels.push_str("</g>");
        }

        let mut body = String::with_capacity(sets.len() + overlaps.len() + labels.len() + 512);
        let _ = write!(
            body,
            "{}",
            title.svg(Point::new(width / 2.0, pad + title.height / 2.0), TITLE)
        );
        let _ = write!(body, "<g class=\"{p}-venn-sets\">{sets}</g>");
        if !overlaps.is_empty() {
            let _ = write!(body, "<g class=\"{p}-venn-overlaps\">{overlaps}</g>");
        }
        if !labels.is_empty() {
            let _ = write!(body, "<g class=\"{p}-venn-labels\">{labels}</g>");
        }
        let svg = svg_document(
            "venn",
            width,
            height,
            size,
            ctx,
            Accessible::default(),
            &body,
        );
        Ok(Rendered {
            svg,
            width,
            height,
            links: Vec::new(),
        })
    }
}
