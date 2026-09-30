//! A tab carried along its strip: it follows the pointer, and the tabs it
//! passes slide aside into the place it left.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Div, ElementId, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderOnce, Stateful, Window, canvas, deferred,
    prelude::*, px,
};
use motion::{Painter, TAB_SLIDE};
use theme::Theme;
use web_time::Instant;

use super::{GAP, Strip};

/// Redraw rate while tabs slide.
const SLIDE_FPS: f32 = 120.0;

/// The tab being carried.
struct Carried<Id> {
    id: Id,
    /// Where the press landed, from the tab's left edge.
    grab: Pixels,
    /// The pointer's last `x`. `None` until the first drag move.
    pointer: Option<Pixels>,
    press: Point<Pixels>,
}

/// One tab's place, as the last frame measured it.
struct Slot<Id> {
    id: Id,
    /// Left edge of the slot, the offset taken out. `None` until measured.
    home: Option<Pixels>,
    width: Pixels,
    /// The offset the last render painted the tab at.
    painted: Pixels,
    slide: Option<Slide>,
}

/// A tab gliding from `from` back to its slot.
#[derive(Clone, Copy)]
struct Slide {
    from: Pixels,
    since: Instant,
}

impl Slide {
    /// The offset `now`, and `None` once the slide has landed.
    fn at(self, now: Instant) -> Option<Pixels> {
        let total = TAB_SLIDE.total().mul_f32(motion::speed_scale());
        let elapsed = now.saturating_duration_since(self.since);
        if elapsed >= total {
            return None;
        }
        let progress = TAB_SLIDE.progress(elapsed.as_secs_f32() / total.as_secs_f32());
        Some(self.from * (1.0 - progress))
    }
}

struct State<Id> {
    painter: Painter,
    carried: Option<Carried<Id>>,
    slots: Vec<Slot<Id>>,
    order: Strip<Id>,
    bounds: Bounds<Pixels>,
}

impl<Id: PartialEq> State<Id> {
    fn slot(&self, id: &Id) -> Option<&Slot<Id>> {
        self.slots.iter().find(|slot| slot.id == *id)
    }

    fn slot_mut(&mut self, id: &Id) -> Option<&mut Slot<Id>> {
        self.slots.iter_mut().find(|slot| slot.id == *id)
    }

    /// Where the carried tab sits against its slot, if `id` is the one carried.
    fn carried_offset(&self, id: &Id) -> Option<Pixels> {
        let carried = self.carried.as_ref().filter(|carried| carried.id == *id)?;
        let pointer = carried.pointer?;
        let home = self.slot(id)?.home?;
        Some(pointer - carried.grab - home)
    }
}

/// Persistent gesture and animation state, one per strip. Mount with [`Self::bar`].
/// The host applies each [`Move`] to its data before the next render.
pub struct Reorder<Id>(Rc<RefCell<State<Id>>>);

impl<Id> Clone for Reorder<Id> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Id: Clone + PartialEq + 'static> Reorder<Id> {
    /// `painter` is the view the strip renders in: it is redrawn while tabs
    /// slide.
    pub fn new(painter: Painter) -> Self {
        Self(Rc::new(RefCell::new(State {
            painter,
            carried: None,
            slots: Vec::new(),
            order: Strip::new(),
            bounds: Bounds::default(),
        })))
    }

    /// Render tabs in `strip` order. Each child is `(tab_id, tabs::tab(...))`.
    /// Do not attach gpui drag handlers; the bar owns the gesture.
    pub fn bar(
        &self,
        id: impl Into<ElementId>,
        strip: &Strip<Id>,
        tabs: impl IntoIterator<Item = (Id, Stateful<Div>)>,
    ) -> Bar<Id> {
        let mut state = self.0.borrow_mut();
        state.slots.retain(|slot| strip.contains(&slot.id));
        if state
            .carried
            .as_ref()
            .is_some_and(|held| !strip.contains(&held.id))
        {
            state.carried = None;
        }
        state.order = strip.clone();
        Bar {
            id: id.into(),
            reorder: self.clone(),
            tabs: tabs.into_iter().collect(),
            moved: None,
            outside: None,
        }
    }

    fn press(&self, id: Id, at: Point<Pixels>) {
        let mut state = self.0.borrow_mut();
        let Some(slot) = state.slot(&id) else { return };
        let Some(home) = slot.home else { return };
        let grab = at.x - home - slot.painted;
        state.carried = Some(Carried {
            id,
            grab,
            pointer: None,
            press: at,
        });
    }

    fn release(&self, cx: &mut App) -> Option<Id> {
        let mut state = self.0.borrow_mut();
        let carried = state.carried.take()?;
        let pointer = carried.pointer?;
        if let Some(slot) = state.slot_mut(&carried.id) {
            let from = pointer - carried.grab - slot.home.unwrap_or(pointer);
            slot.slide = (!cx.reduce_motion() && from != px(0.0)).then_some(Slide {
                from,
                since: cx.background_executor().now(),
            });
        }
        state.painter.notify(cx);
        Some(carried.id)
    }

    /// Carry the held tab to `pointer`, reordering `strip` as it passes its
    /// neighbours. `true` when the order changed.
    fn follow(&self, strip: &mut Strip<Id>, pointer: Point<Pixels>, cx: &App) -> bool {
        let mut state = self.0.borrow_mut();
        state.slots.retain(|slot| strip.contains(&slot.id));
        let Some(carried) = state.carried.as_mut() else {
            return false;
        };
        carried.pointer = Some(pointer.x);
        let (id, grab) = (carried.id.clone(), carried.grab);
        let Some(slot) = state.slot(&id) else {
            return false;
        };
        let (Some(home), width) = (slot.home, slot.width) else {
            return false;
        };

        let travel = (pointer.x - grab - home).as_f32();
        let (passed, left) = strip.carry(&id, travel, |tab| {
            state
                .slot(tab)
                .filter(|slot| slot.home.is_some())
                .map(|slot| slot.width.as_f32())
        });
        if passed.is_empty() {
            return false;
        }

        // Each tab passed moves one carried tab over, the other way; its
        // offset takes the move back out so it starts from where it was seen.
        let shift = px(travel.signum() * (width.as_f32() + GAP));
        let now = cx.background_executor().now();
        let still = cx.reduce_motion();
        for tab in &passed {
            if let Some(slot) = state.slot_mut(tab) {
                slot.home = slot.home.map(|home| home - shift);
                let seen = slot
                    .slide
                    .and_then(|slide| slide.at(now))
                    .unwrap_or_default();
                slot.slide = (!still).then_some(Slide {
                    from: seen + shift,
                    since: now,
                });
            }
        }
        if let Some(slot) = state.slot_mut(&id) {
            slot.home = Some(pointer.x - grab - px(left));
        }
        true
    }

    /// `tab`, placed: at the pointer while carried, sliding while it makes way,
    /// in its slot otherwise.
    fn tab(&self, id: &Id, tab: Stateful<Div>, theme: &Theme, cx: &mut App) -> AnyElement {
        let now = cx.background_executor().now();
        let mut state = self.0.borrow_mut();

        let carried = state.carried_offset(id);
        if state.slot(id).is_none() {
            state.slots.push(Slot {
                id: id.clone(),
                home: None,
                width: px(0.0),
                painted: px(0.0),
                slide: None,
            });
        }
        let painter = state.painter;
        let Some(slot) = state.slot_mut(id) else {
            unreachable!("pushed above");
        };
        let offset = match carried {
            Some(offset) => offset,
            None => match slot.slide.and_then(|slide| slide.at(now)) {
                Some(offset) => offset,
                None => {
                    slot.slide = None;
                    px(0.0)
                }
            },
        };
        slot.painted = offset;
        let sliding = slot.slide.is_some();
        drop(state);
        if sliding {
            painter.lease(SLIDE_FPS, TAB_SLIDE.total(), cx);
        }

        let measure = canvas(
            {
                let (state, id) = (self.0.clone(), id.clone());
                move |bounds: Bounds<Pixels>, _, _| {
                    if let Some(slot) = state.borrow_mut().slot_mut(&id) {
                        slot.home = Some(bounds.left() - slot.painted);
                        slot.width = bounds.size.width;
                    }
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        // Out over the tab's 1px border, so the box is the tab's own.
        .top(px(-1.0))
        .bottom(px(-1.0))
        .left(px(-1.0))
        .right(px(-1.0));

        let tab = tab.relative().left(offset).child(measure);
        match carried {
            Some(_) => deferred(tab.bg(theme.surface_raised)).into_any_element(),
            None => tab.into_any_element(),
        }
    }
}

/// A live move, using indices immediately before this event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: usize,
    pub to: usize,
}

/// A carried tab released outside its strip, in window coordinates.
#[derive(Clone, Debug)]
pub struct OutsideDrop<Id> {
    pub id: Id,
    pub position: Point<Pixels>,
}

type Moved = dyn Fn(&Move, &mut Window, &mut App);
type Outside<Id> = dyn Fn(&OutsideDrop<Id>, &mut Window, &mut App);

/// A strip that owns pointer capture, live ordering and slide animations.
#[derive(IntoElement)]
pub struct Bar<Id: Clone + PartialEq + 'static> {
    id: ElementId,
    reorder: Reorder<Id>,
    tabs: Vec<(Id, Stateful<Div>)>,
    moved: Option<Rc<Moved>>,
    outside: Option<Rc<Outside<Id>>>,
}

impl<Id: Clone + PartialEq + 'static> Bar<Id> {
    /// Apply this move to the host's strip/data synchronously. Accepts `cx.listener`.
    pub fn on_reorder(mut self, moved: impl Fn(&Move, &mut Window, &mut App) + 'static) -> Self {
        self.moved = Some(Rc::new(moved));
        self
    }

    /// Route a tab to another pane on release. Without this hook it stays in this strip.
    pub fn on_drop_outside(
        mut self,
        outside: impl Fn(&OutsideDrop<Id>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.outside = Some(Rc::new(outside));
        self
    }
}

impl<Id: Clone + PartialEq + 'static> RenderOnce for Bar<Id> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let enabled = self.moved.is_some();
        let mut bar = super::bar(self.id).relative();
        for (id, tab) in self.tabs {
            let tab = if enabled {
                let reorder = self.reorder.clone();
                let pressed = id.clone();
                tab.on_mouse_down(MouseButton::Left, move |event, _, _| {
                    reorder.press(pressed.clone(), event.position);
                })
            } else {
                tab
            };
            bar = bar.child(self.reorder.tab(&id, tab, &theme, cx));
        }
        let reorder = self.reorder;
        bar.child(
            canvas(
                {
                    let reorder = reorder.clone();
                    move |bounds, _, _| reorder.0.borrow_mut().bounds = bounds
                },
                move |_, _, window, _| {
                    let moving = reorder.clone();
                    let moved = self.moved;
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase != DispatchPhase::Capture {
                            return;
                        }
                        if event.pressed_button != Some(MouseButton::Left) {
                            moving.release(cx);
                            return;
                        }
                        let mut state = moving.0.borrow_mut();
                        let Some(held) = &state.carried else { return };
                        if held.pointer.is_none()
                            && (event.position - held.press).magnitude() <= 2.0
                        {
                            return;
                        }
                        let id = held.id.clone();
                        let mut order = state.order.clone();
                        let from = order.index_of(&id);
                        drop(state);
                        let changed = moving.follow(&mut order, event.position, cx);
                        let to = order.index_of(&id);
                        state = moving.0.borrow_mut();
                        state.order = order;
                        let painter = state.painter;
                        drop(state);
                        if changed && let (Some(from), Some(to), Some(moved)) = (from, to, &moved) {
                            moved(&Move { from, to }, window, cx);
                        }
                        painter.notify(cx);
                        cx.stop_propagation();
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                            return;
                        }
                        if let Some(id) = reorder.release(cx) {
                            let outside = !reorder.0.borrow().bounds.contains(&event.position);
                            if outside && let Some(callback) = &self.outside {
                                callback(
                                    &OutsideDrop {
                                        id,
                                        position: event.position,
                                    },
                                    window,
                                    cx,
                                );
                            }
                            cx.stop_propagation();
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        )
    }
}
