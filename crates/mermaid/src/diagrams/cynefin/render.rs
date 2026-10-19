//! Pele's `diagrams/cynefin/render.ts`.

use crate::svg::builder::{EscText, LabelSvg};
use std::{collections::HashMap, fmt::Write as _};

use super::{
    boundaries::{
        FOLD, HORIZON, WaveOptions, backward, cliff, curve_path, ellipse_path, forward,
        resolve_seed, wave,
    },
    model::CynefinModel,
};
use crate::{
    Config, Rendered,
    config::Value,
    js,
    svg::{
        builder::{Point, TITLE, fragment, num},
        edges::{marker, marker_trim},
        root::{Accessible, svg_document},
        theme::{RADIUS, series_color},
    },
    text::{Label, Style, layout_label},
};
use crate::{Ctx, Host};

/// [domain, column, row, series color]
const QUADRANTS: [(&str, f64, f64, i64); 4] = [
    ("complex", 0.0, 0.0, 0),
    ("complicated", 1.0, 0.0, 1),
    ("chaotic", 0.0, 1.0, 3),
    ("clear", 1.0, 1.0, 2),
];
const CONFUSION_COLOR: i64 = 4;
const MAX_CONFUSION_ITEMS: usize = 3;
const ITEM_MAX_WIDTH: f64 = 200.0;
const ITEM_PAD_X: f64 = 10.0;
const ITEM_PAD_Y: f64 = 4.0;
const ITEM_GAP: f64 = 6.0;
const HEAD_GAP: f64 = 10.0;
const MARGIN: f64 = 20.0;
const CLEARANCE: f64 = 10.0;
const STEPS: usize = 48;
const CONFUSION: usize = 4;

/// [name, decision model, kind of practice]
fn meta(name: &str) -> (&'static str, &'static str, &'static str) {
    match name {
        "complex" => ("Complex", "Probe → Sense → Respond", "Emergent Practices"),
        "complicated" => ("Complicated", "Sense → Analyse → Respond", "Good Practices"),
        "clear" => ("Clear", "Sense → Categorise → Respond", "Best Practices"),
        "chaotic" => ("Chaotic", "Act → Sense → Respond", "Novel Practices"),
        _ => ("Confusion", "", "Disorder"),
    }
}

struct Badge {
    text: String,
    label: Label,
    w: f64,
    h: f64,
    overflow: bool,
}

struct Block {
    name: &'static str,
    title: Label,
    subtitles: Vec<Label>,
    badges: Vec<Badge>,
    w: f64,
    h: f64,
    cx: f64,
    cy: f64,
}

fn setting(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    match config
        .section("cynefin")
        .and_then(|section| section.get(key))
    {
        Some(Value::Number(value)) if *value >= min && *value <= max => *value,
        _ => fallback,
    }
}

pub fn render_cynefin(model: &CynefinModel, config: &Config, ctx: &Ctx<'_, impl Host>) -> Rendered {
    let p = crate::PREFIX;
    let size = ctx.options.font_size;
    let small = js::round(size * 0.8125);
    let item_size = js::round(size * 0.875);
    let measure = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);
    let show_descriptions = config
        .section("cynefin")
        .and_then(|section| section.get("showDomainDescriptions"))
        != Some(&Value::Bool(false));
    let amplitude = setting(config, "boundaryAmplitude", 8.0, 0.0, 50.0);
    let seed = resolve_seed(
        setting(config, "seed", 0.0, -1e15, 1e15),
        ctx.options.id_prefix.as_str(),
    );
    let item_line = js::round(item_size * 1.5);

    let badge = |text: String, overflow: bool| -> Badge {
        let label = layout_label(Some(&text), false, measure, item_size, ITEM_MAX_WIDTH, 0);
        Badge {
            w: label.width + 2.0 * ITEM_PAD_X,
            h: label.height.max(item_line) + 2.0 * ITEM_PAD_Y,
            text,
            label,
            overflow,
        }
    };

    let block = |name: &'static str| -> Block {
        let meta = meta(name);
        let title = layout_label(Some(meta.0), false, measure, size, 4000.0, Style::BOLD);
        let mut subtitles = Vec::new();
        if show_descriptions {
            for text in [meta.1, meta.2] {
                if !text.is_empty() {
                    subtitles.push(layout_label(
                        Some(text),
                        false,
                        measure,
                        small,
                        4000.0,
                        Style::ITALIC,
                    ));
                }
            }
        }
        let items: &[_] = model
            .domains
            .get(name)
            .map_or(&[], |domain| domain.items.as_slice());
        let shown = if name == "confusion" && items.len() > MAX_CONFUSION_ITEMS {
            MAX_CONFUSION_ITEMS
        } else {
            items.len()
        };
        let mut badges = Vec::new();
        for item in &items[..shown] {
            badges.push(badge(item.label.clone(), false));
        }
        if shown < items.len() {
            badges.push(badge(format!("+{} more", items.len() - shown), true));
        }
        let mut w = title.width;
        let mut h = title.height;
        for s in &subtitles {
            w = w.max(s.width);
            h += s.height;
        }
        if !badges.is_empty() {
            h += HEAD_GAP - ITEM_GAP;
        }
        for b in &badges {
            w = w.max(b.w);
            h += ITEM_GAP + b.h;
        }
        Block {
            name,
            title,
            subtitles,
            badges,
            w,
            h,
            cx: 0.0,
            cy: 0.0,
        }
    };

    // Four quadrants, then confusion at `CONFUSION`.
    let mut blocks: Vec<Block> = QUADRANTS.iter().map(|q| block(q.0)).collect();
    blocks.push(block("confusion"));
    let rx = 88f64.max(blocks[CONFUSION].w * 0.71 + 8.0);
    let ry = 64f64.max(blocks[CONFUSION].h * 0.71 + 6.0);

    let mut quad_w = setting(config, "width", 640.0, 100.0, 10000.0) / 2.0;
    let mut quad_h = setting(config, "height", 440.0, 100.0, 10000.0) / 2.0;
    for b in &blocks[..CONFUSION] {
        quad_w = quad_w.max(b.w + 2.0 * MARGIN);
        quad_h = quad_h.max(b.h + 2.0 * MARGIN);
    }
    // A block sits in the middle of its quadrant, so its inner corner must
    // stay clear of the ellipse.
    for b in &blocks[..CONFUSION] {
        let dx = (quad_w - b.w) / 2.0;
        let dy = (quad_h - b.h) / 2.0;
        if (dx / (rx + CLEARANCE)).powi(2) + (dy / (ry + CLEARANCE)).powi(2) >= 1.0 {
            continue;
        }
        let wider = b.w + 2.0 * (rx + CLEARANCE);
        let taller = b.h + 2.0 * (ry + CLEARANCE);
        if wider * quad_h <= quad_w * taller {
            quad_w = quad_w.max(wider);
        } else {
            quad_h = quad_h.max(taller);
        }
    }

    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let width = 2.0 * quad_w;
    let height = 2.0 * quad_h;
    let x0 = pad;
    let y0 = pad + title_height;
    let cx = x0 + quad_w;
    let cy = y0 + quad_h;
    let right = x0 + width;
    let bottom = y0 + height;

    let fold = wave(
        y0,
        cy,
        cx,
        seed,
        amplitude,
        WaveOptions {
            segments: 4,
            pin_end: true,
            ..FOLD
        },
    );
    let west = wave(
        x0,
        cx,
        cy,
        seed + 100.0,
        amplitude,
        WaveOptions {
            segments: 4,
            pin_end: true,
            ..HORIZON
        },
    );
    let east = wave(
        cx,
        right,
        cy,
        seed + 200.0,
        amplitude,
        WaveOptions {
            segments: 4,
            pin_start: true,
            ..HORIZON
        },
    );
    let drop = cliff(cx, cy, bottom, width * 0.03);
    let at = |curve: &[f64], end: bool| -> Point {
        if end {
            Point(curve[curve.len() - 2], curve[curve.len() - 1])
        } else {
            Point(curve[0], curve[1])
        }
    };

    let region = |f: &mut String, name: &str| {
        let _ = match name {
            "complex" => write!(
                f,
                "M{},{}L{}{}{}Z",
                num(x0),
                num(y0),
                at(&fold, false),
                forward(&fold),
                backward(&west)
            ),
            "complicated" => write!(
                f,
                "M{}L{},{}L{}{}{}Z",
                at(&fold, false),
                num(right),
                num(y0),
                at(&east, true),
                backward(&east),
                backward(&fold)
            ),
            "chaotic" => write!(
                f,
                "M{}{}{}L{},{}Z",
                at(&west, false),
                forward(&west),
                forward(&drop),
                num(x0),
                num(bottom)
            ),
            _ => write!(
                f,
                "M{}{}L{},{}L{}{}Z",
                at(&east, false),
                forward(&east),
                num(right),
                num(bottom),
                at(&drop, true),
                backward(&drop)
            ),
        };
    };

    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let mut content = String::with_capacity(16 * 1024);
    let _ = write!(
        content,
        "{}<g class=\"{p}-cynefin-regions\">",
        LabelSvg(&title, x0 + width / 2.0, pad + title.height / 2.0, TITLE)
    );
    for (name, _, _, color) in QUADRANTS {
        let _ = write!(
            content,
            "<path class=\"{p}-cynefin-region\" data-id=\"{name}\" d=\""
        );
        region(&mut content, name);
        let _ = write!(
            content,
            "\" fill=\"{}\" fill-opacity=\"0.14\"/>",
            series_color(color)
        );
    }
    content.push_str("</g>");
    let _ = write!(
        content,
        "<g class=\"{p}-cynefin-boundaries\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"6 3\"><path d=\"{}\"/><path d=\"{}\"/><path d=\"{}\"/><path class=\"{p}-cynefin-cliff\" d=\"{}\" stroke=\"var(--_l)\" stroke-width=\"2\" stroke-dasharray=\"none\" stroke-linecap=\"round\"/></g>",
        curve_path(&fold),
        curve_path(&west),
        curve_path(&east),
        curve_path(&drop)
    );
    let oval = ellipse_path(cx, cy, rx, ry);
    let _ = write!(
        content,
        "<path d=\"{oval}\" fill=\"var(--_bg)\"/><path class=\"{p}-cynefin-region\" data-id=\"confusion\" d=\"{oval}\" fill=\"{}\" fill-opacity=\"0.18\" stroke=\"var(--_b)\" stroke-dasharray=\"4 2\"/>",
        series_color(CONFUSION_COLOR)
    );

    let subtitle_attrs = format!(
        " class=\"{p}-cynefin-subtitle\" font-size=\"{}\" font-style=\"italic\" fill=\"var(--_m)\"",
        num(small)
    );
    let item_attrs = format!(" class=\"{p}-label\" font-size=\"{}\"", num(item_size));
    let overflow_attrs = format!("{item_attrs} fill=\"var(--_m)\"");
    let draw_block = |out: &mut String, b: &Block| {
        let mut y = b.cy - b.h / 2.0;
        let _ = write!(
            out,
            "<g class=\"{p}-cynefin-domain\" data-id=\"{}\">{}",
            b.name,
            LabelSvg(
                &b.title,
                b.cx,
                y + b.title.height / 2.0,
                crate::class!("label", "font-weight=\"var(--_hw)\""),
            )
        );
        y += b.title.height;
        for s in &b.subtitles {
            let _ = write!(
                out,
                "{}",
                LabelSvg(s, b.cx, y + s.height / 2.0, &subtitle_attrs)
            );
            y += s.height;
        }
        y += HEAD_GAP - ITEM_GAP;
        for item in &b.badges {
            y += ITEM_GAP;
            let _ = write!(
                out,
                "<g class=\"{p}-node{}\"{}><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{}/>{}</g>",
                if item.overflow {
                    concat!(" ", crate::prefix!(), "-cynefin-overflow")
                } else {
                    ""
                },
                fragment(|f| if item.overflow {
                    Ok(())
                } else {
                    write!(f, " data-id=\"{}\"", EscText(&item.text))
                }),
                num(b.cx - item.w / 2.0),
                num(y),
                num(item.w),
                num(item.h),
                if item.overflow {
                    " stroke-dasharray=\"3 2\""
                } else {
                    ""
                },
                LabelSvg(
                    &item.label,
                    b.cx,
                    y + item.h / 2.0,
                    if item.overflow {
                        &overflow_attrs
                    } else {
                        &item_attrs
                    }
                )
            );
            y += item.h;
        }
        out.push_str("</g>");
    };

    let mut by_name: HashMap<&str, usize> = HashMap::new();
    for (index, &(name, col, row, _)) in QUADRANTS.iter().enumerate() {
        let b = &mut blocks[index];
        b.cx = x0 + quad_w * (col + 0.5);
        b.cy = y0 + quad_h * (row + 0.5);
        by_name.insert(name, index);
        draw_block(&mut content, &blocks[index]);
    }
    blocks[CONFUSION].cx = cx;
    blocks[CONFUSION].cy = cy;
    by_name.insert("confusion", CONFUSION);
    draw_block(&mut content, &blocks[CONFUSION]);

    let inside = |b: usize, x: f64, y: f64| -> bool {
        if b == CONFUSION {
            ((x - cx) / (rx + 4.0)).powi(2) + ((y - cy) / (ry + 4.0)).powi(2) <= 1.0
        } else {
            let b = &blocks[b];
            (x - b.cx).abs() <= b.w / 2.0 + 8.0 && (y - b.cy).abs() <= b.h / 2.0 + 8.0
        }
    };

    let edges_at = content.len();
    let edge_attrs = format!(" class=\"{p}-edge-label\" font-size=\"{}\"", num(small));
    let mut seen: HashMap<String, f64> = HashMap::new();
    for transition in &model.transitions {
        let (Some(&fi), Some(&ti)) = (
            by_name.get(transition.from.as_str()),
            by_name.get(transition.to.as_str()),
        ) else {
            continue;
        };
        if fi == ti {
            continue;
        }
        let (from, to) = (&blocks[fi], &blocks[ti]);
        let key = if transition.from < transition.to {
            format!("{} {}", transition.from, transition.to)
        } else {
            format!("{} {}", transition.to, transition.from)
        };
        let repeat = seen.get(&key).copied().unwrap_or(0.0);
        seen.insert(key, repeat + 1.0);

        let dx = to.cx - from.cx;
        let dy = to.cy - from.cy;
        let len = or_one(js::hypot(dx, dy));
        let mx = (from.cx + to.cx) / 2.0;
        let my = (from.cy + to.cy) / 2.0;
        // Curves bow away from the middle of the diagram, which keeps them
        // off the ellipse.
        let outward = if -dy * (mx - cx) + dx * (my - cy) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let nx = (-dy / len) * outward;
        let ny = (dx / len) * outward;
        // A diagonal runs through the middle, so it bows far enough to pass
        // the ellipse.
        let diagonal = fi != CONFUSION && ti != CONFUSION && dx.abs() > 1.0 && dy.abs() > 1.0;
        let bow = (if diagonal {
            2.0 * (js::hypot(rx * nx, ry * ny) + 16.0)
        } else {
            len * 0.15
        }) + repeat * 56.0;
        let px = mx + nx * bow;
        let py = my + ny * bow;
        let point = |t: f64| -> (f64, f64) {
            let u = 1.0 - t;
            (
                u * u * from.cx + 2.0 * u * t * px + t * t * to.cx,
                u * u * from.cy + 2.0 * u * t * py + t * t * to.cy,
            )
        };
        let crossing = |b: usize, mut lo: f64, mut hi: f64, leaving: bool| -> f64 {
            for _ in 0..16 {
                let mid = (lo + hi) / 2.0;
                let (x, y) = point(mid);
                if inside(b, x, y) == leaving {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo + hi) / 2.0
        };
        let steps = STEPS as f64;
        let at_step = |i: usize| point(i as f64 / steps);
        let mut i = 1;
        while i <= STEPS && {
            let (x, y) = at_step(i);
            inside(fi, x, y)
        } {
            i += 1;
        }
        if i > STEPS {
            continue;
        }
        let t0 = crossing(fi, (i - 1) as f64 / steps, i as f64 / steps, true);
        while i <= STEPS && {
            let (x, y) = at_step(i);
            !inside(ti, x, y)
        } {
            i += 1;
        }
        let t1 = if i > STEPS {
            1.0
        } else {
            crossing(ti, (i - 1) as f64 / steps, i as f64 / steps, false)
        };

        let (sx, sy) = point(t0);
        let (mut ex, mut ey) = point(t1);
        let blossom = |a: f64, b: f64, c: f64| -> f64 {
            (1.0 - t0) * (1.0 - t1) * a + ((1.0 - t0) * t1 + t0 * (1.0 - t1)) * b + t0 * t1 * c
        };
        let qx = blossom(from.cx, px, to.cx);
        let qy = blossom(from.cy, py, to.cy);
        let el = or_one(js::hypot(ex - qx, ey - qy));
        let ux = (ex - qx) / el;
        let uy = (ey - qy) / el;
        let head = marker("arrow_point", ex, ey, ux, uy, "var(--_l)");
        ex -= ux * marker_trim("arrow_point");
        ey -= uy * marker_trim("arrow_point");

        if content.len() == edges_at {
            let _ = write!(content, "<g class=\"{p}-edges\">");
        }
        let _ = write!(
            content,
            "<g class=\"{p}-edge\" data-id=\"{}-{}\"><path d=\"M{},{}Q{},{} {},{}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}",
            transition.from,
            transition.to,
            num(sx),
            num(sy),
            num(qx),
            num(qy),
            num(ex),
            num(ey)
        );
        if let Some(raw) = transition.label.as_deref().filter(|l| !l.is_empty()) {
            let label = layout_label(Some(raw), false, measure, small, ITEM_MAX_WIDTH, 0);
            let (lx, ly) = point((t0 + t1) / 2.0);
            let away = (nx.abs() * label.width) / 2.0 + (ny.abs() * label.height) / 2.0 + 6.0;
            let half = label.width / 2.0 + 4.0;
            let tx = (lx + nx * away)
                .max(x0 + half)
                .min((right - half).max(x0 + half));
            let _ = write!(
                content,
                "{}",
                LabelSvg(&label, tx, ly + ny * away, &edge_attrs)
            );
        }
        content.push_str("</g>");
    }
    if content.len() != edges_at {
        content.push_str("</g>");
    }

    Rendered {
        svg: svg_document(
            "cynefin",
            total_width,
            total_height,
            size,
            ctx,
            Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}

/// `x || 1` for a number.
fn or_one(x: f64) -> f64 {
    if js::truthy(x) { x } else { 1.0 }
}
