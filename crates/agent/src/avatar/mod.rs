//! The blob avatar: deterministic identity from a name, ported from blobatar
//! (MIT). Same name, same face, at any size — the colors come from the seed,
//! never from the environment.

pub mod color;
pub mod geometry;

use geometry::{Art, Path2D, Seg};
use gpui::{
    AnyElement, BorderStyle, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Point, Styled, Window,
    canvas, point, px, quad, size, transparent_black,
};

/// `agent::avatar("Sara")` — the canvas fills its layout bounds, so size it
/// with a parent element. The geometry and palette are computed once here and
/// moved into the paint closure; the canvas adapts the 100×100 design space.
pub fn avatar(name: &str) -> AnyElement {
    let art = Art::from_name(name);
    let head = color::to_hsla(art.head);
    let eye = color::to_hsla(art.eye);
    canvas(
        move |_bounds: Bounds<Pixels>, _window, _cx| (),
        move |bounds, (), window, _cx| paint_avatar(window, bounds, &art, head, eye),
    )
    .size_full()
    .into_any_element()
}

fn paint_avatar(window: &mut Window, bounds: Bounds<Pixels>, art: &Art, head: Hsla, eye: Hsla) {
    let s = bounds.size.width.to_f64().min(bounds.size.height.to_f64()) / 100.0;
    let ox = bounds.origin.x.to_f64() + (bounds.size.width.to_f64() - 100.0 * s) / 2.0;
    let oy = bounds.origin.y.to_f64() + (bounds.size.height.to_f64() - 100.0 * s) / 2.0;
    let map = |x: f64, y: f64| {
        point(
            px(ox as f32 + (x * s) as f32),
            px(oy as f32 + (y * s) as f32),
        )
    };
    window.paint_layer(bounds, |window| {
        for c in &art.petals {
            let d = (c.r * 2.0 * s) as f32;
            let b = Bounds {
                origin: point(
                    px(ox as f32 + ((c.cx - c.r) * s) as f32),
                    px(oy as f32 + ((c.cy - c.r) * s) as f32),
                ),
                size: size(px(d), px(d)),
            };
            // Fully rounded corners → disk.
            window.paint_quad(quad(
                b,
                px((c.r * s) as f32),
                head,
                px(0.),
                transparent_black(),
                BorderStyle::default(),
            ));
        }
        for p in &art.extra {
            paint_fill(window, p, head, &map);
        }
        paint_fill(window, &art.body, head, &map);
        for e in &art.eyes {
            paint_fill(window, e, eye, &map);
        }
    });
}

/// Each path painted separately — merging disjoint subpaths into one `Path`
/// changes gpui's two-stage compositing and brightens overlaps (orbs/paint.rs).
fn paint_fill(
    window: &mut Window,
    path: &Path2D,
    color: Hsla,
    map: &dyn Fn(f64, f64) -> Point<Pixels>,
) {
    let mut b = PathBuilder::fill();
    for seg in &path.segs {
        match seg {
            Seg::Move { x, y } => b.move_to(map(*x, *y)),
            Seg::Line { x, y } => b.line_to(map(*x, *y)),
            Seg::Quad { cx, cy, x, y } => b.curve_to(map(*x, *y), map(*cx, *cy)),
            Seg::Cubic {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => {
                b.cubic_bezier_to(map(*x, *y), map(*c1x, *c1y), map(*c2x, *c2y));
            }
        }
    }
    b.close();
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}
