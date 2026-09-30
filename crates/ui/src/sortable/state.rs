use gpui::{
    App, Axis, Bounds, FocusHandle, Pixels, Point, ScrollHandle, SharedString, Size, Window, point,
    px,
};
use motion::{Painter, TAB_SLIDE};
use web_time::Instant;

use super::{Carry, CarryTarget, Move, Position};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
pub(super) struct Model<L, I> {
    pub id: L,
    pub axis: Axis,
    pub kind: SharedString,
    pub gap: Pixels,
    pub items: Vec<I>,
}

pub(super) struct List<L, I> {
    pub model: Model<L, I>,
    pub origin: Point<Pixels>,
    pub viewport: Bounds<Pixels>,
    pub scroll: ScrollHandle,
}

#[derive(Clone)]
pub(super) struct Drag<L, I> {
    pub item: I,
    pub from: Position<L>,
    pub to: Option<Position<L>>,
    pub size: Size<Pixels>,
    pub press: Point<Pixels>,
    pub pointer: Point<Pixels>,
    pub grab: Point<Pixels>,
    pub origin: Point<Pixels>,
    pub started: bool,
    pub detached: bool,
}

impl<L, I> Drag<L, I> {
    pub fn carry_bounds(&self) -> Bounds<Pixels> {
        let size = gpui::size(self.size.width.min(px(180.)), self.size.height.min(px(32.)));
        let grab = point(self.grab.x.min(size.width), self.grab.y.min(size.height));
        Bounds::new(self.pointer - grab, size)
    }
}

pub(super) struct Slot<I> {
    pub id: I,
    pub serial: usize,
    pub bounds: Option<Bounds<Pixels>>,
    pub painted: Point<Pixels>,
    pub origin: Point<Pixels>,
    pub floating: bool,
    pub returning: bool,
    pub slide: Option<Slide>,
}

pub(super) struct Slide {
    pub from: Point<Pixels>,
    pub since: Instant,
}

impl Slide {
    pub fn offset(&self, now: Instant) -> Option<Point<Pixels>> {
        let total = TAB_SLIDE.total().mul_f32(motion::speed_scale());
        let elapsed = now.saturating_duration_since(self.since);
        if elapsed >= total {
            return None;
        }
        Some(self.from * (1. - TAB_SLIDE.progress(elapsed.as_secs_f32() / total.as_secs_f32())))
    }
}

pub(super) struct State<L, I> {
    pub painter: Painter,
    pub lists: Vec<List<L, I>>,
    pub slots: Vec<Slot<I>>,
    pub drag: Option<Drag<L, I>>,
    pub axis_locked: bool,
    pub docking: Option<Rc<dyn CarryTarget<I>>>,
    pub focus: Option<FocusHandle>,
    pub previous_focus: Option<FocusHandle>,
    pub focus_captured: bool,
    pub suppress_release: bool,
    next_serial: usize,
    drift_since: Option<Instant>,
}

pub(super) fn along(axis: Axis, point: Point<Pixels>) -> Pixels {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

fn length(axis: Axis, size: Size<Pixels>) -> Pixels {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

impl<L: Clone + PartialEq, I: Clone + PartialEq> State<L, I> {
    pub fn new(painter: Painter) -> Self {
        Self {
            painter,
            lists: Vec::new(),
            slots: Vec::new(),
            drag: None,
            next_serial: 0,
            axis_locked: false,
            docking: None,
            focus: None,
            previous_focus: None,
            focus_captured: false,
            suppress_release: false,
            drift_since: None,
        }
    }

    pub fn list(&self, id: &L) -> Option<&List<L, I>> {
        self.lists.iter().find(|list| &list.model.id == id)
    }
    pub fn list_mut(&mut self, id: &L) -> Option<&mut List<L, I>> {
        self.lists.iter_mut().find(|list| &list.model.id == id)
    }
    pub fn slot(&self, id: &I) -> Option<&Slot<I>> {
        self.slots.iter().find(|slot| &slot.id == id)
    }
    pub fn slot_mut(&mut self, id: &I) -> Option<&mut Slot<I>> {
        self.slots.iter_mut().find(|slot| &slot.id == id)
    }

    pub fn sync(&mut self, models: Vec<Model<L, I>>, axis_locked: bool, cx: &mut App) {
        // A host edit invalidates the original indices; never commit a stale move.
        if self.drag.is_some() && !self.lists.iter().map(|list| &list.model).eq(models.iter()) {
            self.cancel(cx);
        }
        self.axis_locked = axis_locked;
        let mut old = std::mem::take(&mut self.lists);
        self.lists = models
            .into_iter()
            .map(|model| {
                if let Some(index) = old.iter().position(|list| list.model.id == model.id) {
                    let mut list = old.remove(index);
                    list.model = model;
                    list.viewport = Bounds::default();
                    list
                } else {
                    List {
                        model,
                        origin: Point::default(),
                        viewport: Bounds::default(),
                        scroll: ScrollHandle::new(),
                    }
                }
            })
            .collect();
        self.slots.retain(|slot| {
            self.lists
                .iter()
                .any(|list| list.model.items.contains(&slot.id))
        });
    }

    pub fn serial(&mut self, id: &I) -> usize {
        if let Some(slot) = self.slot(id) {
            return slot.serial;
        }
        let serial = self.next_serial;
        self.next_serial += 1;
        self.slots.push(Slot {
            id: id.clone(),
            serial,
            bounds: None,
            painted: Point::default(),
            origin: Point::default(),
            floating: false,
            returning: false,
            slide: None,
        });
        serial
    }

    pub fn press(&mut self, id: &I, pointer: Point<Pixels>, cx: &mut App) {
        let Some(slot) = self.slot(id) else { return };
        let Some(bounds) = slot.bounds else { return };
        let Some((list, index)) = self.lists.iter().find_map(|list| {
            list.model
                .items
                .iter()
                .position(|item| item == id)
                .map(|index| (list, index))
        }) else {
            return;
        };
        let from = Position {
            list: list.model.id.clone(),
            index,
        };
        self.drag = Some(Drag {
            item: id.clone(),
            from: from.clone(),
            to: Some(from),
            size: bounds.size,
            press: pointer,
            pointer,
            grab: pointer - slot.painted,
            origin: slot.painted,
            started: false,
            detached: false,
        });
        self.suppress_release = false;
        cx.stop_propagation();
    }

    pub fn sample(&mut self, pointer: Point<Pixels>, cx: &mut App) -> bool {
        let Some(drag) = self.drag.as_mut() else {
            return false;
        };
        drag.pointer = pointer;
        if !drag.started && (pointer - drag.press).magnitude() <= 2. {
            return false;
        }
        drag.started = true;
        self.aim();
        self.report_carry(cx);
        self.painter.notify(cx);
        true
    }

    fn target(&self, drag: &Drag<L, I>) -> Option<usize> {
        let kind = &self.list(&drag.from.list)?.model.kind;
        let mut pointer = drag.pointer;
        if self.axis_locked && self.docking.is_some() && !drag.detached {
            let source = self.list(&drag.from.list)?;
            match source.model.axis {
                Axis::Horizontal => {
                    pointer.y = pointer
                        .y
                        .clamp(source.viewport.top(), source.viewport.bottom())
                }
                Axis::Vertical => {
                    pointer.x = pointer
                        .x
                        .clamp(source.viewport.left(), source.viewport.right())
                }
            }
        }
        self.lists.iter().rposition(|list| {
            &list.model.kind == kind
                && list.viewport.size.width > px(0.)
                && list.viewport.size.height > px(0.)
                && list.viewport.contains(&pointer)
        })
    }

    /// Target the visible list, using preview slot centres (not animated paint).
    pub fn aim(&mut self) -> bool {
        let Some(drag) = self.drag.as_ref().filter(|drag| drag.started) else {
            return false;
        };
        let detached = if self.axis_locked && self.docking.is_some() {
            self.list(&drag.from.list).is_some_and(|source| {
                let (at, start, end) = match source.model.axis {
                    Axis::Horizontal => (
                        drag.pointer.y,
                        source.viewport.top(),
                        source.viewport.bottom(),
                    ),
                    Axis::Vertical => (
                        drag.pointer.x,
                        source.viewport.left(),
                        source.viewport.right(),
                    ),
                };
                (drag.detached && !source.viewport.contains(&drag.pointer))
                    || at < start - px(12.)
                    || at > end + px(12.)
            })
        } else {
            self.target(drag).is_none()
        };
        self.drag.as_mut().unwrap().detached = detached;
        let drag = self.drag.as_ref().unwrap();
        let target = self.target(drag).map(|index| &self.lists[index]);
        let to = target.map(|list| {
            let axis = list.model.axis;
            let centre = along(axis, drag.pointer - drag.grab) + length(axis, drag.size) / 2.;
            let gap = drag
                .to
                .as_ref()
                .filter(|to| to.list == list.model.id)
                .map(|to| to.index);
            let mut cursor = along(axis, list.origin);
            let mut index = 0;
            for (at, id) in list
                .model
                .items
                .iter()
                .filter(|id| **id != drag.item)
                .enumerate()
            {
                if gap == Some(at) {
                    cursor += length(axis, drag.size) + list.model.gap;
                }
                let Some(bounds) = self.slot(id).and_then(|slot| slot.bounds) else {
                    break;
                };
                let extent = length(axis, bounds.size);
                if centre <= cursor + extent / 2. {
                    break;
                }
                index += 1;
                cursor += extent + list.model.gap;
            }
            Position {
                list: list.model.id.clone(),
                index,
            }
        });
        if to == drag.to {
            return false;
        }
        self.drag.as_mut().unwrap().to = to;
        true
    }

    pub fn carry(&self) -> Option<Carry<I>> {
        let drag = self.drag.as_ref().filter(|drag| drag.started)?;
        Some(Carry {
            item: drag.item.clone(),
            pointer: drag.pointer,
            bounds: drag.carry_bounds(),
            detached: drag.detached,
        })
    }

    pub fn report_carry(&self, cx: &mut App) {
        if let Some(target) = &self.docking
            && let Some(carry) = self.carry()
        {
            target.update(carry, cx);
        }
    }

    pub fn restore_focus(&mut self, window: &mut Window, cx: &mut App) {
        if !std::mem::take(&mut self.focus_captured) {
            return;
        }
        let previous = self.previous_focus.take();
        if self
            .focus
            .as_ref()
            .is_some_and(|focus| focus.is_focused(window))
        {
            match previous {
                Some(previous) => window.focus(&previous, cx),
                None => window.blur(cx),
            }
        }
    }

    pub fn cancel(&mut self, cx: &mut App) {
        if let Some(drag) = self.drag.take() {
            if let Some(target) = &self.docking {
                target.cancel(cx);
            }
            self.suppress_release = drag.started;
            self.painter.notify(cx);
        }
        self.drift_since = None;
    }

    pub fn release(
        &mut self,
        pointer: Point<Pixels>,
        cx: &mut App,
    ) -> Option<(Drag<L, I>, Option<Move<L, I>>)> {
        if self.drag.as_ref().is_some_and(|drag| drag.started) {
            self.sample(pointer, cx);
        }
        let drag = self.drag.take()?;
        self.drift_since = None;
        if !drag.started {
            return None;
        }
        self.painter.notify(cx);
        let moved = drag
            .to
            .as_ref()
            .filter(|to| **to != drag.from)
            .map(|to| Move {
                item: drag.item.clone(),
                from: drag.from.clone(),
                to: to.clone(),
            });
        Some((drag, moved))
    }

    pub fn drift(&mut self, cx: &mut App) {
        let Some(drag) = self.drag.as_ref().filter(|drag| drag.started) else {
            self.drift_since = None;
            return;
        };
        let Some(index) = self.target(drag) else {
            self.drift_since = None;
            return;
        };
        let list = &self.lists[index];
        let now = cx.background_executor().now();
        let dt = self
            .drift_since
            .replace(now)
            .map(|since| now.saturating_duration_since(since).as_secs_f32().min(0.05))
            .unwrap_or(0.);
        let axis = list.model.axis;
        let start = along(axis, list.viewport.origin);
        let end = start + length(axis, list.viewport.size);
        // A small edge zone leaves short lists a useful non-scrolling middle.
        let edge = px(24.).min((end - start) / 4.);
        let at = along(axis, drag.pointer);
        let speed = if at < start + edge {
            ((start + edge - at) / edge) * 600.
        } else if at > end - edge {
            -((at - end + edge) / edge) * 600.
        } else {
            0.
        };
        let old = list.scroll.offset();
        let max = along(axis, list.scroll.max_offset());
        let value = (along(axis, old) + px(speed * dt)).clamp(-max, px(0.));
        let can_move =
            (speed > 0. && along(axis, old) < px(0.)) || (speed < 0. && along(axis, old) > -max);
        if can_move {
            let offset = match axis {
                Axis::Horizontal => point(value, old.y),
                Axis::Vertical => point(old.x, value),
            };
            list.scroll.set_offset(offset);
            self.painter
                .lease(120., std::time::Duration::from_millis(100), cx);
        } else {
            self.drift_since = None;
        }
    }
}
