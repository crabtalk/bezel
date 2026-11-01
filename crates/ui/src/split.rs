//! Two panes with a draggable divider. The host owns and persists the ratio.

use crate::widgets::{self, Layout, SplitStyle};
use gpui::{
    AnyElement, App, Axis, DragMoveEvent, ElementId, Empty, IntoElement, MouseButton, Pixels,
    Refineable, RenderOnce, StyleRefinement, Window, div, prelude::*, px,
};
use theme::Theme;

#[derive(Clone)]
struct Drag(ElementId);

#[derive(Default)]
struct Gesture {
    held: bool,
    offset: Pixels,
}

/// Fill the available space with two panes. Set the new ratio in `on_resize`
/// and redraw the host. The container must accommodate both pane minimums
/// plus [`widgets::SPLIT_HANDLE_HIT`].
#[derive(IntoElement)]
pub struct Split {
    id: ElementId,
    axis: Axis,
    first: AnyElement,
    second: AnyElement,
    ratio: f32,
    min: Pixels,
    resized: Option<Box<Resize>>,
    style: StyleRefinement,
}

type Resize = dyn Fn(&f32, &mut Window, &mut App);

impl Split {
    pub fn horizontal(
        id: impl Into<ElementId>,
        first: impl IntoElement,
        second: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            axis: Axis::Horizontal,
            first: first.into_any_element(),
            second: second.into_any_element(),
            ratio: 0.5,
            min: px(0.0),
            resized: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn vertical(
        id: impl Into<ElementId>,
        first: impl IntoElement,
        second: impl IntoElement,
    ) -> Self {
        Self {
            axis: Axis::Vertical,
            ..Self::horizontal(id, first, second)
        }
    }

    /// Share of pane space assigned to the first pane, excluding the divider.
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio.clamp(0.0, 1.0);
        self
    }

    pub fn min(mut self, min: Pixels) -> Self {
        self.min = min.max(px(0.0));
        self
    }

    pub fn on_resize(mut self, resized: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.resized = Some(Box::new(resized));
        self
    }
}

impl Styled for Split {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Split {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let gesture = window.use_keyed_state(self.id.clone(), cx, |_, _| Gesture::default());
        let held = gesture.read(cx).held;
        let pressed = gesture.clone();
        let released = gesture.clone();
        let axis = self.axis;
        let min = self.min;
        let id = self.id.clone();
        let pane = |child, weight| {
            div()
                .flex_grow(weight)
                .flex_shrink(1.0)
                .flex_basis(px(0.0))
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .map(|pane| match axis {
                    Axis::Horizontal => pane.min_w(min).h_full(),
                    Axis::Vertical => pane.min_h(min).w_full(),
                })
                .child(child)
        };
        let handle = Theme::of(cx)
            .split_handle(axis, SplitStyle::Line { dragging: held })
            .id("divider")
            .on_drag(Drag(id.clone()), move |_, offset, _, cx| {
                pressed.update(cx, |gesture, cx| {
                    gesture.held = true;
                    gesture.offset = match axis {
                        Axis::Horizontal => offset.x,
                        Axis::Vertical => offset.y,
                    } - px(widgets::SPLIT_HANDLE_HIT / 2.0);
                    cx.notify();
                });
                cx.new(|_| Empty)
            });
        let mut frame = div()
            .id(self.id)
            .size_full()
            .flex()
            .map(|frame| match axis {
                Axis::Horizontal => frame.flex_row(),
                Axis::Vertical => frame.flex_col(),
            })
            .when_some(self.resized, |frame, resized| {
                frame.on_drag_move(move |event: &DragMoveEvent<Drag>, window, cx| {
                    if event.drag(cx).0 != id {
                        return;
                    }
                    let mut bounds = event.bounds;
                    let half = px(widgets::SPLIT_HANDLE_HIT / 2.0);
                    let extent = match axis {
                        Axis::Horizontal => {
                            bounds.origin.x += half + gesture.read(cx).offset;
                            bounds.size.width -= half * 2.0;
                            bounds.size.width
                        }
                        Axis::Vertical => {
                            bounds.origin.y += half + gesture.read(cx).offset;
                            bounds.size.height -= half * 2.0;
                            bounds.size.height
                        }
                    };
                    if extent <= px(0.0) {
                        return;
                    }
                    let ratio =
                        widgets::axis_fraction(event.event.position, bounds, axis, min / extent);
                    resized(&ratio, window, cx);
                })
            })
            .on_mouse_up(MouseButton::Left, {
                let released = released.clone();
                move |_, _, cx| {
                    released.update(cx, |gesture, cx| {
                        if std::mem::take(&mut gesture.held) {
                            cx.notify();
                        }
                    })
                }
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                released.update(cx, |gesture, cx| {
                    if std::mem::take(&mut gesture.held) {
                        cx.notify();
                    }
                })
            })
            .child(pane(self.first, self.ratio))
            .child(handle)
            .child(pane(self.second, 1.0 - self.ratio));
        frame.style().refine(&self.style);
        frame
    }
}
