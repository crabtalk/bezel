//! A tab carried along its strip: it follows the pointer, and the tabs it
//! passes slide aside into the place it left.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, Div, Pixels, Point, Stateful, canvas, deferred, prelude::*, px,
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
        let home = self.slot(id).and_then(|slot| slot.home);
        Some(match (carried.pointer, home) {
            (Some(pointer), Some(home)) => pointer - carried.grab - home,
            _ => px(0.0),
        })
    }
}

/// A strip's tabs in motion: the one being dragged, and the ones sliding
/// aside for it. Kept by the view beside its [`Strip`], one per strip.
///
/// The order changes while the drag is held, so a drop needs no handler: the
/// tab is already where it was let go. Four places to wire:
///
/// ```ignore
/// tabs::bar("panel-tabs")
///     .on_drag_move(cx.listener(|view, event: &DragMoveEvent<TabDrag>, _, cx| {
///         view.reorder.follow(&mut view.strip, event.event.position, cx);
///         cx.notify();
///     }))
///     .children(self.strip.tabs().iter().map(|id| {
///         let tab = tabs::tab(&theme, key, label, state).on_drag(TabDrag(*id), {
///             let (reorder, id) = (self.reorder.clone(), *id);
///             move |_, at, _, cx| {
///                 reorder.grab(id, at);
///                 cx.new(|_| gpui::Empty)
///             }
///         });
///         self.reorder.tab(id, tab, &theme, cx)
///     }))
/// ```
///
/// The ghost `on_drag` returns is still painted at the pointer; an empty one
/// leaves the carried tab as the only thing that moves.
///
/// `follow` runs for every move of the drag, wherever the pointer is. Pointer
/// travel off the strip's axis is ignored.
///
/// The carried tab paints deferred, over its neighbours and outside the
/// strip's clip.
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
        })))
    }

    /// Pick `id` up. Call it from the tab's `on_drag` constructor, with the
    /// press offset that constructor is handed.
    pub fn grab(&self, id: Id, at: Point<Pixels>) {
        self.0.borrow_mut().carried = Some(Carried {
            id,
            grab: at.x,
            pointer: None,
        });
    }

    /// Carry the held tab to `pointer`, reordering `strip` as it passes its
    /// neighbours. `true` when the order changed.
    pub fn follow(&self, strip: &mut Strip<Id>, pointer: Point<Pixels>, cx: &App) -> bool {
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
    pub fn tab(&self, id: &Id, tab: Stateful<Div>, theme: &Theme, cx: &mut App) -> AnyElement {
        let now = cx.background_executor().now();
        let mut state = self.0.borrow_mut();

        // The drag going away is the end of the gesture, however it ended.
        if !cx.has_active_drag()
            && let Some(carried) = state.carried.take()
        {
            let home = state.slot(&carried.id).and_then(|slot| slot.home);
            let from = match (carried.pointer, home) {
                (Some(pointer), Some(home)) => pointer - carried.grab - home,
                _ => px(0.0),
            };
            if let Some(slot) = state.slot_mut(&carried.id)
                && !cx.reduce_motion()
                && from != px(0.0)
            {
                slot.slide = Some(Slide { from, since: now });
            }
        }

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
