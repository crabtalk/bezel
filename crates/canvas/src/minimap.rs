//! A map of the whole canvas, the part in view outlined. Press or drag in it
//! to look there.
//!
//! ```ignore
//! div().absolute().right(px(12.)).bottom(px(12.)).w(px(160.)).h(px(110.))
//!     .child(canvas::minimap(&view, cx))
//! ```

use std::{cell::Cell, rc::Rc};

use gpui::{
    App, BorderStyle, Bounds, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    Pixels, Point, canvas as painter, div, fill, outline, point, prelude::*, px, size,
};
use theme::Theme;

use crate::CanvasView;

/// Room left around the map, in pixels.
const INSET: f32 = 6.0;

/// Where the map painted: the canvas origin on screen, and pixels per unit.
#[derive(Clone, Copy)]
struct Scale {
    origin: Point<f32>,
    per: f32,
}

impl Scale {
    fn to_canvas(self, at: Point<Pixels>) -> (f32, f32) {
        (
            (at.x.as_f32() - self.origin.x) / self.per,
            (at.y.as_f32() - self.origin.y) / self.per,
        )
    }
}

/// A map of `view`'s canvas, to size and place beside it.
pub fn minimap(view: &Entity<CanvasView>, cx: &App) -> impl IntoElement + use<> {
    let editor = view.read(cx).editor();
    let theme = Theme::of(cx);
    let boxes: Vec<(f32, f32, f32, f32)> = editor
        .painted()
        .nodes
        .iter()
        .map(|n| (n.x as f32, n.y as f32, n.width as f32, n.height as f32))
        .collect();
    let seen = editor.visible();
    let (ink, frame) = (theme.text_faint, theme.accent);
    let scale: Rc<Cell<Option<Scale>>> = Rc::default();
    let look = Rc::new({
        let (view, scale) = (view.downgrade(), scale.clone());
        move |at: Point<Pixels>, cx: &mut App| {
            if let Some(scale) = scale.get() {
                let at = scale.to_canvas(at);
                let _ = view.update(cx, |view, cx| {
                    view.update_editor(cx, |editor| editor.center_on(at))
                });
            }
        }
    });
    let press = look.clone();
    div()
        .size_full()
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, _, cx: &mut App| {
                cx.stop_propagation();
                press(event.position, cx);
            },
        )
        .on_mouse_move(move |event: &MouseMoveEvent, _, cx: &mut App| {
            if event.pressed_button == Some(MouseButton::Left) {
                look(event.position, cx);
            }
        })
        .child(
            painter(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let world = boxes.iter().copied().chain(seen).reduce(
                        |(ax, ay, aw, ah), (bx, by, bw, bh)| {
                            let (x0, y0) = (ax.min(bx), ay.min(by));
                            (
                                x0,
                                y0,
                                (ax + aw).max(bx + bw) - x0,
                                (ay + ah).max(by + bh) - y0,
                            )
                        },
                    );
                    let Some((wx, wy, ww, wh)) = world else {
                        return;
                    };
                    let room = (
                        bounds.size.width.as_f32() - 2.0 * INSET,
                        bounds.size.height.as_f32() - 2.0 * INSET,
                    );
                    let per = (room.0 / ww.max(1.0)).min(room.1 / wh.max(1.0));
                    let origin = point(
                        bounds.origin.x.as_f32() + INSET + (room.0 - ww * per) / 2.0 - wx * per,
                        bounds.origin.y.as_f32() + INSET + (room.1 - wh * per) / 2.0 - wy * per,
                    );
                    scale.set(Some(Scale { origin, per }));
                    let rect = |(x, y, w, h): (f32, f32, f32, f32)| {
                        Bounds::new(
                            point(px(origin.x + x * per), px(origin.y + y * per)),
                            size(px((w * per).max(1.0)), px((h * per).max(1.0))),
                        )
                    };
                    for b in &boxes {
                        window.paint_quad(fill(rect(*b), ink));
                    }
                    if let Some(seen) = seen {
                        window.paint_quad(outline(rect(seen), frame, BorderStyle::Solid));
                    }
                },
            )
            .size_full(),
        )
}
