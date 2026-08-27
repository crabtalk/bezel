//! The same face on a grid: one silhouette sampled per cell, the eyes punched
//! through rather than painted, so the surface shows where they are and the
//! sprite survives being dimmed in a list.

use crate::avatar::{Eye, Face, Shape};
use gpui::{
    AnyElement, Bounds, Hsla, IntoElement, Pixels, Styled, Window, canvas, fill, point, px, size,
};
use theme::Theme;

/// Cells across the sprite.
pub const GRID: usize = 8;

/// The lid closes the eye to a sliver rather than to nothing, so a blink keeps
/// its shape on the way down.
const SQUEEZE: f32 = 0.92;

/// One row per byte, bit `x` set where the body covers cell `x`.
fn cells(shape: &Shape, eyes: &[Eye; 2], lid: f32) -> [u8; GRID] {
    // Unlike the smooth painter, which insets its body inside the box: a
    // mascot fills its grid, so the furthest the outline reaches is the edge.
    let span = shape
        .outline()
        .iter()
        .fold(0.0f32, |far, (x, y)| far.max(x.hypot(*y)));

    let mut rows = [0u8; GRID];
    for (y, row) in rows.iter_mut().enumerate() {
        let uy = unit(y, span);
        for x in 0..GRID {
            let ux = unit(x, span);
            // Sound because the profile floors above zero, so the body is
            // star-shaped and a point is inside iff it is nearer than the reach
            // in its own direction.
            if ux.hypot(uy) <= shape.reach(ux, uy) {
                *row |= 1 << x;
            }
        }
    }
    for eye in eyes {
        punch(&mut rows, eye, lid, span);
    }
    rows
}

/// The centre of cell `i` in the shape's own units.
fn unit(i: usize, span: f32) -> f32 {
    ((i as f32 + 0.5) / GRID as f32 * 2.0 - 1.0) * span
}

fn punch(rows: &mut [u8; GRID], eye: &Eye, lid: f32, span: f32) {
    let ry = (eye.ry * (1.0 - lid * SQUEEZE)).max(f32::EPSILON);
    let (sin, cos) = eye.rot.to_radians().sin_cos();
    let mut hit = false;
    for (y, row) in rows.iter_mut().enumerate() {
        for x in 0..GRID {
            let (dx, dy) = (unit(x, span) - eye.cx, unit(y, span) - eye.cy);
            let (a, b) = (dx * cos + dy * sin, dy * cos - dx * sin);
            if (a / eye.rx).abs().powf(eye.n) + (b / ry).abs().powf(eye.n) <= 1.0 {
                *row &= !(1 << x);
                hit = true;
            }
        }
    }
    if hit || lid >= 0.5 {
        return;
    }
    // At eight cells an open eye can fall between every centre and punch
    // nothing, leaving a blank body. Walk in until a cell is claimed — the
    // centre is always inside, so this terminates.
    for step in 0..=GRID {
        let k = 1.0 - step as f32 / GRID as f32;
        let (x, y) = (cell(eye.cx * k, span), cell(eye.cy * k, span));
        if rows[y] >> x & 1 == 1 {
            rows[y] &= !(1 << x);
            return;
        }
    }
}

/// Which cell a coordinate falls in.
fn cell(u: f32, span: f32) -> usize {
    (((u / span + 1.0) / 2.0 * GRID as f32) as usize).min(GRID - 1)
}

/// The element, filling its layout bounds.
pub fn mascot(face: &Face, t: f32) -> AnyElement {
    // Drift and breath are both under a cell at this size, so only the blink
    // survives the trip; the outline is the resting one, since a wobble that
    // cannot move a whole cell only flickers the ones on the edge.
    let rows = cells(
        &face.shape,
        &face.eyes.place(&face.shape, 0.0),
        face.motion.beat(t).lid,
    );
    let tint = face.color;
    canvas(
        move |_: Bounds<Pixels>, _, _| (),
        move |bounds, (), window, cx| {
            let color = tint.unwrap_or(Theme::of(cx).accent);
            paint(window, bounds, &rows, color);
        },
    )
    .size_full()
    .into_any_element()
}

fn paint(window: &mut Window, bounds: Bounds<Pixels>, rows: &[u8; GRID], color: Hsla) {
    let scale = window.scale_factor();
    let snap = |v: f32| (v * scale).round() / scale;
    let box_side = bounds.size.width.min(bounds.size.height).to_f64() as f32;
    // A whole number of device pixels a side, or the grid shimmers.
    let side = ((box_side * scale / GRID as f32).floor() / scale).max(1.0 / scale);
    let sprite = side * GRID as f32;
    let left = snap(bounds.center().x.to_f64() as f32 - sprite / 2.0);
    let top = snap(bounds.center().y.to_f64() as f32 - sprite / 2.0);

    for (y, row) in rows.iter().enumerate() {
        let mut x = 0;
        while x < GRID {
            if row >> x & 1 == 0 {
                x += 1;
                continue;
            }
            // Filled cells merge into one quad a run, so a solid row is one.
            let mut run = 1;
            while x + run < GRID && row >> (x + run) & 1 == 1 {
                run += 1;
            }
            window.paint_quad(fill(
                Bounds::new(
                    point(px(left + side * x as f32), px(top + side * y as f32)),
                    size(px(side * run as f32), px(side)),
                ),
                color,
            ));
            x += run;
        }
    }
}
