//! Pele's `diagrams/cynefin/render.ts`.

use crate::config::within;
use crate::geom::Point;
use crate::svg::builder::EscText;
use crate::svg::edges::Marker;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{collections::HashMap, fmt::Write as _};

use super::{
    boundaries::{FOLD, HORIZON, WaveOptions, cliff, ellipse_path, resolve_seed, wave},
    model::{CynefinModel, Domain},
};
use crate::diagrams::Render;
use crate::geom::Size;
use crate::{Config, Ctx, Error, Host};
use crate::{
    Rendered, js,
    svg::{
        builder::{TITLE, fragment, num},
        root::svg_document,
        theme::{RADIUS, series_color},
    },
    text::{Label, Style},
};

/// Each quadrant's domain and its column and row.
const QUADRANTS: [(Domain, Point); 4] = [
    (Domain::Complex, Point::new(0.0, 0.0)),
    (Domain::Complicated, Point::new(1.0, 0.0)),
    (Domain::Chaotic, Point::new(0.0, 1.0)),
    (Domain::Clear, Point::new(1.0, 1.0)),
];
const MAX_CONFUSION_ITEMS: usize = 3;
const ITEM_MAX_WIDTH: f64 = 200.0;
const ITEM_PAD_X: f64 = 10.0;
const ITEM_PAD_Y: f64 = 4.0;
const ITEM_GAP: f64 = 6.0;
const HEAD_GAP: f64 = 10.0;
const MARGIN: f64 = 20.0;
const CLEARANCE: f64 = 10.0;
const STEPS: usize = 48;

impl Domain {
    /// Its title, decision model and kind of practice.
    fn meta(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Domain::Complex => ("Complex", "Probe → Sense → Respond", "Emergent Practices"),
            Domain::Complicated => ("Complicated", "Sense → Analyse → Respond", "Good Practices"),
            Domain::Clear => ("Clear", "Sense → Categorise → Respond", "Best Practices"),
            Domain::Chaotic => ("Chaotic", "Act → Sense → Respond", "Novel Practices"),
            Domain::Confusion => ("Confusion", "", "Disorder"),
        }
    }

    fn color(self) -> i64 {
        match self {
            Domain::Complex => 0,
            Domain::Complicated => 1,
            Domain::Clear => 2,
            Domain::Chaotic => 3,
            Domain::Confusion => 4,
        }
    }
}

struct Badge {
    text: String,
    label: Label,
    size: Size,
    overflow: bool,
}

/// A domain's text, laid out in [`Domain::ALL`] order.
struct Block {
    domain: Domain,
    title: Label,
    subtitles: Vec<Label>,
    badges: Vec<Badge>,
    size: Size,
    center: Point,
}

/// Mermaid's `cynefin` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CynefinConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub show_domain_descriptions: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub boundary_amplitude: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub seed: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub height: Option<f64>,
}

impl Render for CynefinModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let config = &config.cynefin;
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = js::round(size * 0.8125);
        let item_size = js::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let show_descriptions = config.show_domain_descriptions != Some(false);
        let amplitude = within(config.boundary_amplitude, 8.0, 0.0, 50.0);
        let seed = resolve_seed(
            within(config.seed, 0.0, -1e15, 1e15),
            ctx.options.id_prefix.as_str(),
        );
        let item_line = js::round(item_size * 1.5);

        let badge = |text: String, overflow: bool| -> Badge {
            let label = Label::layout(Some(&text), false, measure, item_size, ITEM_MAX_WIDTH, 0);
            Badge {
                size: Size::new(
                    label.width + 2.0 * ITEM_PAD_X,
                    label.height.max(item_line) + 2.0 * ITEM_PAD_Y,
                ),
                text,
                label,
                overflow,
            }
        };

        let block = |domain: Domain| -> Block {
            let meta = domain.meta();
            let title = Label::layout(Some(meta.0), false, measure, size, 4000.0, Style::BOLD);
            let mut subtitles = Vec::new();
            if show_descriptions {
                for text in [meta.1, meta.2] {
                    if !text.is_empty() {
                        subtitles.push(Label::layout(
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
            let items: &[_] = self
                .domains
                .get(&domain)
                .map_or(&[], |domain| domain.items.as_slice());
            let shown = if domain == Domain::Confusion && items.len() > MAX_CONFUSION_ITEMS {
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
                w = w.max(b.size.w);
                h += ITEM_GAP + b.size.h;
            }
            Block {
                domain,
                title,
                subtitles,
                badges,
                size: Size::new(w, h),
                center: Point::default(),
            }
        };

        let mut blocks: Vec<Block> = Domain::ALL.into_iter().map(block).collect();
        let confusion = &blocks[Domain::Confusion as usize];
        let radii = Point::new(
            88f64.max(confusion.size.w * 0.71 + 8.0),
            64f64.max(confusion.size.h * 0.71 + 6.0),
        );
        let (rx, ry) = (radii.x, radii.y);

        let mut quad_w = within(config.width, 640.0, 100.0, 10000.0) / 2.0;
        let mut quad_h = within(config.height, 440.0, 100.0, 10000.0) / 2.0;
        let quadrants = &blocks[..Domain::Confusion as usize];
        for b in quadrants {
            quad_w = quad_w.max(b.size.w + 2.0 * MARGIN);
            quad_h = quad_h.max(b.size.h + 2.0 * MARGIN);
        }
        // A block sits in the middle of its quadrant, so its inner corner must
        // stay clear of the ellipse.
        for b in quadrants {
            let dx = (quad_w - b.size.w) / 2.0;
            let dy = (quad_h - b.size.h) / 2.0;
            if (dx / (rx + CLEARANCE)).powi(2) + (dy / (ry + CLEARANCE)).powi(2) >= 1.0 {
                continue;
            }
            let wider = b.size.w + 2.0 * (rx + CLEARANCE);
            let taller = b.size.h + 2.0 * (ry + CLEARANCE);
            if wider * quad_h <= quad_w * taller {
                quad_w = quad_w.max(wider);
            } else {
                quad_h = quad_h.max(taller);
            }
        }

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);
        let width = 2.0 * quad_w;
        let height = 2.0 * quad_h;
        let x0 = pad;
        let y0 = pad + title_height;
        let center = Point::new(x0 + quad_w, y0 + quad_h);
        let right = x0 + width;
        let bottom = y0 + height;

        let fold = wave(
            y0,
            center.y,
            center.x,
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
            center.x,
            center.y,
            seed + 100.0,
            amplitude,
            WaveOptions {
                segments: 4,
                pin_end: true,
                ..HORIZON
            },
        );
        let east = wave(
            center.x,
            right,
            center.y,
            seed + 200.0,
            amplitude,
            WaveOptions {
                segments: 4,
                pin_start: true,
                ..HORIZON
            },
        );
        let drop = cliff(center, bottom, width * 0.03);

        let region = |f: &mut String, domain: Domain| {
            let _ = match domain {
                Domain::Complex => write!(
                    f,
                    "M{},{}L{}{}{}Z",
                    num(x0),
                    num(y0),
                    fold.start(),
                    fold.forward(),
                    west.backward()
                ),
                Domain::Complicated => write!(
                    f,
                    "M{}L{},{}L{}{}{}Z",
                    fold.start(),
                    num(right),
                    num(y0),
                    east.end(),
                    east.backward(),
                    fold.backward()
                ),
                Domain::Chaotic => write!(
                    f,
                    "M{}{}{}L{},{}Z",
                    west.start(),
                    west.forward(),
                    drop.forward(),
                    num(x0),
                    num(bottom)
                ),
                Domain::Clear | Domain::Confusion => write!(
                    f,
                    "M{}{}L{},{}L{}{}Z",
                    east.start(),
                    east.forward(),
                    num(right),
                    num(bottom),
                    drop.end(),
                    drop.backward()
                ),
            };
        };

        let total_width = (width.max(title.width) + 2.0 * pad).ceil();
        let total_height = (height + title_height + 2.0 * pad).ceil();
        let mut content = String::with_capacity(16 * 1024);
        let _ = write!(
            content,
            "{}<g class=\"{p}-cynefin-regions\">",
            title.svg(
                Point::new(x0 + width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );
        for (domain, _) in QUADRANTS {
            let _ = write!(
                content,
                "<path class=\"{p}-cynefin-region\" data-id=\"{}\" d=\"",
                domain.name()
            );
            region(&mut content, domain);
            let _ = write!(
                content,
                "\" fill=\"{}\" fill-opacity=\"0.14\"/>",
                series_color(domain.color())
            );
        }
        content.push_str("</g>");
        let _ = write!(
            content,
            "<g class=\"{p}-cynefin-boundaries\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"6 3\"><path d=\"{}\"/><path d=\"{}\"/><path d=\"{}\"/><path class=\"{p}-cynefin-cliff\" d=\"{}\" stroke=\"var(--_l)\" stroke-width=\"2\" stroke-dasharray=\"none\" stroke-linecap=\"round\"/></g>",
            fold.path(),
            west.path(),
            east.path(),
            drop.path()
        );
        let oval = ellipse_path(center, radii);
        let _ = write!(
            content,
            "<path d=\"{oval}\" fill=\"var(--_bg)\"/><path class=\"{p}-cynefin-region\" data-id=\"confusion\" d=\"{oval}\" fill=\"{}\" fill-opacity=\"0.18\" stroke=\"var(--_b)\" stroke-dasharray=\"4 2\"/>",
            series_color(Domain::Confusion.color())
        );

        let subtitle_attrs = format!(
            " class=\"{p}-cynefin-subtitle\" font-size=\"{}\" font-style=\"italic\" fill=\"var(--_m)\"",
            num(small)
        );
        let item_attrs = format!(" class=\"{p}-label\" font-size=\"{}\"", num(item_size));
        let overflow_attrs = format!("{item_attrs} fill=\"var(--_m)\"");
        let draw_block = |out: &mut String, b: &Block| {
            let mut y = b.center.y - b.size.h / 2.0;
            let _ = write!(
                out,
                "<g class=\"{p}-cynefin-domain\" data-id=\"{}\">{}",
                b.domain.name(),
                b.title.svg(
                    Point::new(b.center.x, y + b.title.height / 2.0),
                    crate::class!("label", "font-weight=\"var(--_hw)\"")
                )
            );
            y += b.title.height;
            for s in &b.subtitles {
                let _ = write!(
                    out,
                    "{}",
                    s.svg(Point::new(b.center.x, y + s.height / 2.0), &subtitle_attrs)
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
                    num(b.center.x - item.size.w / 2.0),
                    num(y),
                    num(item.size.w),
                    num(item.size.h),
                    if item.overflow {
                        " stroke-dasharray=\"3 2\""
                    } else {
                        ""
                    },
                    item.label.svg(
                        Point::new(b.center.x, y + item.size.h / 2.0),
                        if item.overflow {
                            &overflow_attrs
                        } else {
                            &item_attrs
                        }
                    )
                );
                y += item.size.h;
            }
            out.push_str("</g>");
        };

        for (domain, cell) in QUADRANTS {
            let b = &mut blocks[domain as usize];
            b.center = Point::new(x0 + quad_w * (cell.x + 0.5), y0 + quad_h * (cell.y + 0.5));
            draw_block(&mut content, b);
        }
        blocks[Domain::Confusion as usize].center = center;
        draw_block(&mut content, &blocks[Domain::Confusion as usize]);

        let inside = |b: usize, at: Point| -> bool {
            if b == Domain::Confusion as usize {
                ((at.x - center.x) / (rx + 4.0)).powi(2) + ((at.y - center.y) / (ry + 4.0)).powi(2)
                    <= 1.0
            } else {
                let b = &blocks[b];
                (at.x - b.center.x).abs() <= b.size.w / 2.0 + 8.0
                    && (at.y - b.center.y).abs() <= b.size.h / 2.0 + 8.0
            }
        };

        let edges_at = content.len();
        let edge_attrs = format!(" class=\"{p}-edge-label\" font-size=\"{}\"", num(small));
        let mut seen: HashMap<(Domain, Domain), f64> = HashMap::new();
        for transition in &self.transitions {
            let (fi, ti) = (transition.from as usize, transition.to as usize);
            if fi == ti {
                continue;
            }
            let (from, to) = (&blocks[fi], &blocks[ti]);
            let key = if fi < ti {
                (transition.from, transition.to)
            } else {
                (transition.to, transition.from)
            };
            let repeat = seen.get(&key).copied().unwrap_or(0.0);
            seen.insert(key, repeat + 1.0);

            let d = to.center - from.center;
            let len = js::hypot(d.x, d.y);
            let mid = from.center.midpoint(to.center);
            // Curves bow away from the middle of the diagram, which keeps them
            // off the ellipse.
            let outward = if -d.y * (mid.x - center.x) + d.x * (mid.y - center.y) >= 0.0 {
                1.0
            } else {
                -1.0
            };
            let normal = Point::new((-d.y / len) * outward, (d.x / len) * outward);
            // A diagonal runs through the middle, so it bows far enough to pass
            // the ellipse.
            let diagonal = fi != Domain::Confusion as usize
                && ti != Domain::Confusion as usize
                && d.x.abs() > 1.0
                && d.y.abs() > 1.0;
            let bow = (if diagonal {
                2.0 * (js::hypot(rx * normal.x, ry * normal.y) + 16.0)
            } else {
                len * 0.15
            }) + repeat * 56.0;
            let control = mid + normal * bow;
            let point = |t: f64| -> Point {
                let u = 1.0 - t;
                from.center * (u * u) + control * (2.0 * u * t) + to.center * (t * t)
            };
            let crossing = |b: usize, mut lo: f64, mut hi: f64, leaving: bool| -> f64 {
                for _ in 0..16 {
                    let mid = (lo + hi) / 2.0;
                    if inside(b, point(mid)) == leaving {
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
            while i <= STEPS && inside(fi, at_step(i)) {
                i += 1;
            }
            if i > STEPS {
                continue;
            }
            let t0 = crossing(fi, (i - 1) as f64 / steps, i as f64 / steps, true);
            while i <= STEPS && !inside(ti, at_step(i)) {
                i += 1;
            }
            let t1 = if i > STEPS {
                1.0
            } else {
                crossing(ti, (i - 1) as f64 / steps, i as f64 / steps, false)
            };

            let start = point(t0);
            let mut end = point(t1);
            let q = from.center * ((1.0 - t0) * (1.0 - t1))
                + control * ((1.0 - t0) * t1 + t0 * (1.0 - t1))
                + to.center * (t0 * t1);
            let el = js::or_one(js::hypot(end.x - q.x, end.y - q.y));
            let u = (end - q) / el;
            let head = Marker::Arrow.draw(end, u, "var(--_l)");
            end = end - u * Marker::Arrow.trim();

            if content.len() == edges_at {
                let _ = write!(content, "<g class=\"{p}-edges\">");
            }
            let _ = write!(
                content,
                "<g class=\"{p}-edge\" data-id=\"{}-{}\"><path d=\"M{start}Q{q} {end}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}",
                transition.from.name(),
                transition.to.name(),
            );
            if let Some(raw) = transition.label.as_deref().filter(|l| !l.is_empty()) {
                let label = Label::layout(Some(raw), false, measure, small, ITEM_MAX_WIDTH, 0);
                let at = point((t0 + t1) / 2.0);
                let away = (normal.x.abs() * label.width) / 2.0
                    + (normal.y.abs() * label.height) / 2.0
                    + 6.0;
                let half = label.width / 2.0 + 4.0;
                let tx = (at.x + normal.x * away)
                    .max(x0 + half)
                    .min((right - half).max(x0 + half));
                let _ = write!(
                    content,
                    "{}",
                    label.svg(Point::new(tx, at.y + normal.y * away), &edge_attrs)
                );
            }
            content.push_str("</g>");
        }
        if content.len() != edges_at {
            content.push_str("</g>");
        }

        Ok(Rendered {
            svg: svg_document(
                "cynefin",
                total_width,
                total_height,
                size,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width: total_width,
            height: total_height,
            links: Vec::new(),
        })
    }
}
