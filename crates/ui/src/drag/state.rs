use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{App, Axis, Bounds, FocusHandle, Pixels, Point, Size, Window, point, px, size};
use motion::{Painter, TAB_SLIDE};
use web_time::Instant;

use super::{Drop, Feedback, Ghost, Outside};
use crate::scroll::Scroller;

/// Shared through the payload with every target of the drag.
#[derive(Default)]
pub(crate) struct Gesture {
    pub grab: Cell<Point<Pixels>>,
    pub size: Cell<Size<Pixels>>,
    pub claimed: Cell<bool>,
    pub hosted: Cell<bool>,
    /// Where the ghost standing in for the item was last drawn.
    pub shown: Cell<Option<Point<Pixels>>>,
}

impl Gesture {
    /// Hides the in-place item while unclaimed, for a target drawing its own
    /// ghost at `origin` this frame.
    pub fn host(&self, origin: Point<Pixels>) {
        self.hosted.set(true);
        self.shown.set(Some(origin));
    }

    pub fn ghost_bounds(&self, pointer: Point<Pixels>) -> Bounds<Pixels> {
        let full = self.size.get();
        let grab = self.grab.get();
        let frame = size(full.width.min(px(180.)), full.height.min(px(32.)));
        let grab = point(grab.x.min(frame.width), grab.y.min(frame.height));
        Bounds::new(pointer - grab, frame)
    }
}

pub(super) type OnDrop<R, I> = dyn Fn(&Drop<R, I>, &mut Window, &mut App);
pub(super) type OnOutside<I> = dyn Fn(&Outside<I>, &mut Window, &mut App);
pub(super) type Accepts<I> = dyn Fn(&I) -> bool;
pub(super) type Lands<I> = dyn Fn(&I, Option<&I>, Option<&I>) -> bool;
pub(super) type Carries<I> = dyn Fn(&I) -> Vec<I>;

pub(super) struct Config<R, I> {
    pub accepts: Option<Box<Accepts<I>>>,
    pub lands: Option<Box<Lands<I>>>,
    pub carries: Option<Box<Carries<I>>>,
    pub feedback: Feedback,
    pub axis_locked: bool,
    pub scroll: Option<Scroller>,
    pub dropped: Option<Rc<OnDrop<R, I>>>,
    pub outside: Option<Rc<OnOutside<I>>>,
    pub focus: FocusHandle,
}

impl<R, I> Config<R, I> {
    fn accepts(&self, item: &I) -> bool {
        self.accepts.as_ref().is_none_or(|accepts| accepts(item))
    }

    /// The allowed gap nearest `proposed` among `others`, the painted items
    /// the carried one is not.
    fn nearest(&self, item: &I, others: &[&Mark<I>], proposed: usize) -> Option<usize> {
        let Some(lands) = &self.lands else {
            return Some(proposed);
        };
        let allowed = |at: usize| {
            let after = at.checked_sub(1).map(|at| &others[at].item);
            let before = others.get(at).map(|mark| &mark.item);
            lands(item, after, before)
        };
        (0..=others.len()).find_map(|distance| {
            [proposed.checked_sub(distance), Some(proposed + distance)]
                .into_iter()
                .flatten()
                .filter(|at| *at <= others.len())
                .find(|at| allowed(*at))
        })
    }
}

pub(super) struct Mark<I> {
    pub item: I,
    pub bounds: Bounds<Pixels>,
}

/// A region as painted in the latest frame.
pub(super) struct Frame<R, I> {
    pub id: R,
    pub axis: Axis,
    pub origin: Point<Pixels>,
    /// The region's scroll offset as it entered the frame. Read once there:
    /// a list holds its state for the whole of its items' prepaint.
    pub scrolled: Point<Pixels>,
    pub visible: Bounds<Pixels>,
    pub handles: Vec<Mark<I>>,
    pub config: Rc<Config<R, I>>,
}

impl<R, I: PartialEq> Frame<R, I> {
    /// The painted items that are not carried.
    fn others<'a>(&'a self, members: &'a [I]) -> impl Iterator<Item = &'a Mark<I>> + 'a {
        self.handles
            .iter()
            .filter(move |mark| !members.contains(&mark.item))
    }

    fn position(&self, item: &I) -> Option<usize> {
        self.handles.iter().position(|mark| &mark.item == item)
    }

    fn reference(&self) -> Point<Pixels> {
        self.origin + self.scrolled
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct Landing<R> {
    pub region: R,
    /// Insertion index among the region's painted items, the carried one
    /// left out.
    pub index: usize,
}

pub(super) struct Live<R, I> {
    pub item: I,
    /// The item and the ones it carries along, in order.
    pub members: Vec<I>,
    pub from: R,
    pub gesture: Rc<Gesture>,
    pub origin: Point<Pixels>,
    pub extent: Pixels,
    pub landing: Option<Landing<R>>,
    pub detached: bool,
    pub pointer: Point<Pixels>,
    pub outside: Option<Rc<OnOutside<I>>>,
    pub previous_focus: Option<FocusHandle>,
}

struct Slide {
    from: Point<Pixels>,
    since: Instant,
    /// Drawn above its region, as the floating item or ghost it settles from.
    lifted: bool,
}

impl Slide {
    fn offset(&self, now: Instant) -> Option<Point<Pixels>> {
        let total = TAB_SLIDE.total().mul_f32(motion::speed_scale());
        let elapsed = now.saturating_duration_since(self.since);
        if elapsed >= total {
            return None;
        }
        Some(self.from * (1. - TAB_SLIDE.progress(elapsed.as_secs_f32() / total.as_secs_f32())))
    }
}

/// What represented an item on screen in the last frame it was placed.
enum Seen {
    /// Drawn in its slot, heading for `target`.
    Slot {
        target: Point<Pixels>,
        painted: Point<Pixels>,
    },
    /// Drawn in place, following the pointer.
    Floating(Point<Pixels>),
    /// Hidden while a ghost stood in for it, wherever the ghost reported.
    Ghost(Rc<Gesture>),
    /// Not drawn.
    Hidden,
}

struct Slot<I> {
    item: I,
    seen_at: Seen,
    /// The region's reference when `seen_at` was recorded.
    reference: Point<Pixels>,
    slide: Option<Slide>,
    seen: bool,
}

/// How a handle paints this frame.
pub(super) struct Placement {
    pub offset: Point<Pixels>,
    pub floating: bool,
    pub hidden: bool,
}

pub(super) struct State<R, I> {
    pub painter: Painter,
    pub ghost: Option<Rc<Ghost<I>>>,
    pub live: Option<Live<R, I>>,
    pub press: Option<(I, Point<Pixels>)>,
    pub stale: bool,
    /// The last complete frame: what landings are computed against.
    frames: Vec<Frame<R, I>>,
    /// The frame being prepainted.
    building: Vec<Frame<R, I>>,
    current: Option<usize>,
    prepainted: bool,
    slots: Vec<Slot<I>>,
    drift_since: Option<Instant>,
    /// Whether this frame has drifted yet: once, before the first region lays
    /// out, so every region in the frame reads the same offsets.
    drifted: bool,
}

pub(super) fn along(axis: Axis, point: Point<Pixels>) -> Pixels {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

pub(super) fn across(axis: Axis, point: Point<Pixels>) -> Pixels {
    along(axis.invert(), point)
}

pub(super) fn length(axis: Axis, size: Size<Pixels>) -> Pixels {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn on(axis: Axis, value: Pixels) -> Point<Pixels> {
    match axis {
        Axis::Horizontal => point(value, px(0.)),
        Axis::Vertical => point(px(0.), value),
    }
}

/// How far a tab may leave its strip across its axis and still be on it.
const DETACH: Pixels = px(12.);

impl<R: Clone + PartialEq, I: Clone + PartialEq> State<R, I> {
    pub fn new(painter: Painter, ghost: Option<Rc<Ghost<I>>>) -> Self {
        Self {
            painter,
            ghost,
            live: None,
            press: None,
            stale: false,
            frames: Vec::new(),
            building: Vec::new(),
            current: None,
            prepainted: false,
            slots: Vec::new(),
            drift_since: None,
            drifted: false,
        }
    }

    /// The first region built after a paint starts a new frame of geometry,
    /// and aims against the one just completed before anything renders.
    pub fn begin_frame(&mut self) {
        if !self.prepainted {
            return;
        }
        self.prepainted = false;
        self.drifted = false;
        self.frames = std::mem::take(&mut self.building);
        self.slots.retain(|slot| slot.seen);
        for slot in &mut self.slots {
            slot.seen = false;
        }
        if let Some(pointer) = self.live.as_ref().map(|live| live.pointer) {
            self.aim(pointer);
        }
    }

    pub fn enter(
        &mut self,
        id: R,
        axis: Axis,
        bounds: Bounds<Pixels>,
        visible: Bounds<Pixels>,
        config: Rc<Config<R, I>>,
    ) -> Option<usize> {
        let frame = Frame {
            id,
            axis,
            origin: bounds.origin,
            scrolled: config
                .scroll
                .as_ref()
                .map(|scroll| scroll.offset())
                .unwrap_or_default(),
            visible,
            handles: Vec::new(),
            config,
        };
        let at = match self.building.iter().position(|old| old.id == frame.id) {
            Some(at) => {
                self.building[at] = frame;
                at
            }
            None => {
                self.building.push(frame);
                self.building.len() - 1
            }
        };
        self.current.replace(at)
    }

    pub fn leave(&mut self, previous: Option<usize>) {
        self.current = previous;
        self.prepainted = true;
    }

    pub fn place(
        &mut self,
        item: &I,
        bounds: Bounds<Pixels>,
        pointer: Point<Pixels>,
        cx: &mut App,
    ) -> Placement {
        let now = cx.background_executor().now();
        let Some(ix) = self.current else {
            return Placement {
                offset: Point::default(),
                floating: false,
                hidden: false,
            };
        };
        let frame = &mut self.building[ix];
        let (carried_before, m) = match &self.live {
            Some(live) => (
                frame
                    .handles
                    .iter()
                    .any(|mark| live.members.contains(&mark.item)),
                frame.others(&live.members).count(),
            ),
            None => (false, 0),
        };
        frame.handles.push(Mark {
            item: item.clone(),
            bounds,
        });
        let frame = &self.building[ix];
        let reference = frame.reference();
        let axis = frame.axis;
        let mut seen = None;
        let target = match &self.live {
            Some(live) if &live.item == item => {
                let gesture = &live.gesture;
                if self.ghost.is_some() || (gesture.hosted.get() && !gesture.claimed.get()) {
                    seen = Some(Seen::Ghost(gesture.clone()));
                    bounds.origin
                } else {
                    let mut at = pointer - gesture.grab.get();
                    if frame.config.axis_locked && !live.detached {
                        at += on(axis.invert(), across(axis, live.origin - at));
                    }
                    seen = Some(Seen::Floating(at));
                    at
                }
            }
            Some(live) if live.members.contains(item) => {
                seen = Some(Seen::Hidden);
                bounds.origin
            }
            Some(live) if frame.config.feedback == Feedback::Displace => {
                let removed = frame.id == live.from && carried_before;
                let inserted = live
                    .landing
                    .as_ref()
                    .is_some_and(|landing| landing.region == frame.id && m >= landing.index);
                let shift = match (inserted, removed) {
                    (true, false) => live.extent,
                    (false, true) => -live.extent,
                    _ => px(0.),
                };
                bounds.origin + on(axis, shift)
            }
            _ => bounds.origin,
        };
        let painter = self.painter;
        let slot = match self.slots.iter().position(|slot| &slot.item == item) {
            Some(at) => &mut self.slots[at],
            None => {
                self.slots.push(Slot {
                    item: item.clone(),
                    seen_at: Seen::Hidden,
                    reference,
                    slide: None,
                    seen: false,
                });
                self.slots.last_mut().unwrap()
            }
        };
        let hidden = matches!(seen, Some(Seen::Ghost(_) | Seen::Hidden));
        if seen.is_some() {
            slot.slide = None;
        } else {
            let moved = reference - slot.reference;
            let from = match &slot.seen_at {
                Seen::Slot {
                    target: last,
                    painted,
                } => (*last + moved != target).then(|| (*painted + moved, false)),
                Seen::Floating(at) => Some((*at, true)),
                Seen::Ghost(gesture) => gesture.shown.get().map(|at| (at, true)),
                Seen::Hidden => None,
            };
            if let Some((from, lifted)) = from {
                let lifted = lifted || slot.slide.as_ref().is_some_and(|slide| slide.lifted);
                slot.slide = (from != target).then(|| Slide {
                    from: from - target,
                    since: now,
                    lifted,
                });
            }
        }
        if cx.reduce_motion() {
            slot.slide = None;
        }
        let sliding = match slot.slide.as_ref().and_then(|slide| slide.offset(now)) {
            Some(offset) => offset,
            None => {
                slot.slide = None;
                Point::default()
            }
        };
        let floating = matches!(seen, Some(Seen::Floating(_)))
            || slot.slide.as_ref().is_some_and(|slide| slide.lifted);
        slot.seen_at = seen.unwrap_or(Seen::Slot {
            target,
            painted: target + sliding,
        });
        slot.reference = reference;
        slot.seen = true;
        if slot.slide.is_some() {
            painter.lease(120., TAB_SLIDE.total(), cx);
        }
        Placement {
            offset: target + sliding - bounds.origin,
            floating,
            hidden,
        }
    }

    pub fn painted(&self, item: &I) -> Option<Point<Pixels>> {
        self.slots
            .iter()
            .find(|slot| &slot.item == item)
            .and_then(|slot| match slot.seen_at {
                Seen::Slot { painted, .. } => Some(painted),
                Seen::Floating(at) => Some(at),
                Seen::Ghost(_) | Seen::Hidden => None,
            })
    }

    pub fn start(
        &mut self,
        item: &I,
        gesture: Rc<Gesture>,
        cursor: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Point<Pixels> {
        let Some((frame, at)) = self
            .frames
            .iter()
            .find_map(|frame| frame.position(item).map(|at| (frame, at)))
        else {
            debug_assert!(false, "a drag handle is painted inside a region");
            return cursor;
        };
        let axis = frame.axis;
        let bounds = frame.handles[at].bounds;
        let grab = match &self.press {
            Some((pressed, grab)) if pressed == item => *grab,
            _ => cursor,
        };
        let start = along(axis, bounds.origin);
        let members: Vec<I> = std::iter::once(item.clone())
            .chain(
                frame
                    .config
                    .carries
                    .as_ref()
                    .map(|carries| carries(item))
                    .unwrap_or_default(),
            )
            .collect();
        let gap = match (frame.handles.get(at + 1), at.checked_sub(1)) {
            (Some(next), _) => along(axis, next.bounds.origin) - start - length(axis, bounds.size),
            (None, Some(previous)) => {
                let previous = frame.handles[previous].bounds;
                start - along(axis, previous.origin) - length(axis, previous.size)
            }
            (None, None) => px(0.),
        }
        .max(px(0.));
        let own = length(axis, bounds.size) + gap;
        let painted: Vec<usize> = frame
            .handles
            .iter()
            .enumerate()
            .filter(|(_, mark)| members.contains(&mark.item))
            .map(|(at, _)| at)
            .collect();
        let last = painted.iter().copied().max().unwrap_or(at);
        let block = match frame.handles.get(last + 1) {
            Some(next) => along(axis, next.bounds.origin) - start,
            None => {
                let last = frame.handles[last].bounds;
                along(axis, last.origin) + length(axis, last.size) + gap - start
            }
        };
        // Members out of the painted range count at the item's own pitch.
        let extent = block + own * (members.len() - painted.len()) as f32;
        gesture.grab.set(grab);
        gesture.size.set(bounds.size);
        gesture.claimed.set(true);
        let origin = self.painted(item).unwrap_or(bounds.origin);
        let previous_focus = window.focused(cx);
        window.focus(&frame.config.focus, cx);
        self.live = Some(Live {
            item: item.clone(),
            members,
            from: frame.id.clone(),
            gesture,
            origin,
            extent,
            landing: Some(Landing {
                region: frame.id.clone(),
                index: at,
            }),
            detached: false,
            pointer: origin + grab,
            outside: frame.config.outside.clone(),
            previous_focus,
        });
        self.press = None;
        self.aim(window.mouse_position());
        self.painter.notify(cx);
        grab
    }

    /// Recompute the landing for `pointer`. True when it changed.
    pub fn aim(&mut self, pointer: Point<Pixels>) -> bool {
        let Some(live) = self.live.as_mut() else {
            return false;
        };
        live.pointer = pointer;
        let item = &live.item;
        let members = &live.members;
        let grab = live.gesture.grab.get();
        let size = live.gesture.size.get();
        let mut claimed = false;
        let mut landing = None;
        for frame in self.frames.iter().rev() {
            if !frame.config.accepts(item) {
                continue;
            }
            let mut at = pointer;
            let visible = frame.visible;
            if visible.size.width <= px(0.) || visible.size.height <= px(0.) {
                continue;
            }
            if frame.config.axis_locked && frame.id == live.from {
                let axis = frame.axis;
                let across_at = across(axis, pointer);
                let (start, end) = (
                    across(axis, visible.origin),
                    across(axis, visible.origin) + length(axis.invert(), visible.size),
                );
                live.detached = (live.detached && !visible.contains(&pointer))
                    || across_at < start - DETACH
                    || across_at > end + DETACH;
                if live.detached {
                    continue;
                }
                claimed = true;
                at += on(axis.invert(), across_at.clamp(start, end) - across_at);
            }
            if !visible.contains(&at) {
                continue;
            }
            claimed = true;
            let axis = frame.axis;
            let centre = along(axis, pointer - grab) + length(axis, size) / 2.;
            let mut index = 0;
            for mark in frame.others(members) {
                let middle = along(axis, mark.bounds.origin) + length(axis, mark.bounds.size) / 2.;
                if centre <= middle {
                    break;
                }
                index += 1;
            }
            landing = frame
                .config
                .nearest(item, &frame.others(members).collect::<Vec<_>>(), index)
                .map(|index| Landing {
                    region: frame.id.clone(),
                    index,
                });
            break;
        }
        live.gesture.claimed.set(claimed);
        if landing == live.landing {
            return false;
        }
        live.landing = landing;
        true
    }

    /// The gap a region shows as an indicator: its position along the axis.
    pub fn indicator(&self, id: &R) -> Option<(Axis, Pixels, Bounds<Pixels>)> {
        let live = self.live.as_ref()?;
        let landing = live
            .landing
            .as_ref()
            .filter(|landing| &landing.region == id)?;
        let aimed = self.frames.iter().find(|frame| &frame.id == id)?;
        let others: Vec<_> = aimed.others(&live.members).collect();
        let before = others.get(landing.index).map(|mark| &mark.item);
        let after = landing
            .index
            .checked_sub(1)
            .and_then(|at| others.get(at))
            .map(|mark| &mark.item);
        // Drawn where the neighbours are painted in the frame being drawn.
        let frame = self
            .building
            .iter()
            .find(|frame| &frame.id == id)
            .unwrap_or(aimed);
        let axis = frame.axis;
        let bounds = |item: &I| {
            frame
                .handles
                .iter()
                .find(|mark| &mark.item == item)
                .map(|mark| mark.bounds)
        };
        let at = match (before.and_then(bounds), after.and_then(bounds)) {
            (Some(before), _) => along(axis, before.origin),
            (None, Some(after)) => along(axis, after.origin) + length(axis, after.size),
            (None, None) => along(axis, frame.origin) + px(1.),
        };
        Some((axis, at, frame.visible))
    }

    /// Ends the gesture. The caller commits whatever the landing names.
    pub fn end(&mut self, window: &mut Window, cx: &mut App) -> Option<Live<R, I>> {
        let live = self.live.take()?;
        self.stale = false;
        self.drift_since = None;
        let focus = self
            .frames
            .iter()
            .find(|frame| frame.id == live.from)
            .map(|frame| frame.config.focus.clone());
        if focus.is_some_and(|focus| focus.is_focused(window)) {
            match &live.previous_focus {
                Some(previous) => window.focus(previous, cx),
                None => window.blur(cx),
            }
        }
        self.painter.notify(cx);
        Some(live)
    }

    pub fn release(
        &mut self,
        pointer: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Commit<R, I>> {
        self.aim(pointer);
        let live = self.end(window, cx)?;
        let Some(landing) = live.landing.clone() else {
            if live.gesture.claimed.get() {
                return None;
            }
            return live.outside.map(|outside| {
                Commit::Outside(
                    outside,
                    Outside {
                        item: live.item,
                        position: pointer,
                    },
                )
            });
        };
        let frame = self
            .frames
            .iter()
            .find(|frame| frame.id == landing.region)?;
        if frame.id == live.from && frame.position(&live.item) == Some(landing.index) {
            return None;
        }
        let dropped = frame.config.dropped.clone()?;
        let others: Vec<_> = frame.others(&live.members).collect();
        let after = landing
            .index
            .checked_sub(1)
            .and_then(|at| others.get(at))
            .map(|mark| mark.item.clone());
        let before = others.get(landing.index).map(|mark| mark.item.clone());
        Some(Commit::Drop(
            dropped,
            Drop {
                item: live.item,
                from: live.from,
                region: landing.region,
                after,
                before,
            },
        ))
    }

    /// Scroll every region under the pointer while it rests near that
    /// region's visible edges, whether or not the region accepts the item.
    pub fn drift(&mut self, cx: &mut App) {
        if std::mem::replace(&mut self.drifted, true) {
            return;
        }
        let Some(pointer) = self.live.as_ref().map(|live| live.pointer) else {
            self.drift_since = None;
            return;
        };
        let now = cx.background_executor().now();
        let dt = self
            .drift_since
            .map(|since| now.saturating_duration_since(since).as_secs_f32().min(0.05))
            .unwrap_or(0.);
        let mut drifting = false;
        for frame in &self.frames {
            let Some(scroll) = frame.config.scroll.as_ref() else {
                continue;
            };
            if !frame.visible.contains(&pointer) {
                continue;
            }
            let axis = frame.axis;
            let start = along(axis, frame.visible.origin);
            let end = start + length(axis, frame.visible.size);
            let edge = px(24.).min((end - start) / 4.);
            let at = along(axis, pointer);
            let speed = if at < start + edge {
                ((start + edge - at) / edge) * 600.
            } else if at > end - edge {
                -((at - end + edge) / edge) * 600.
            } else {
                0.
            };
            let old = scroll.offset();
            let max = along(axis, scroll.max_offset());
            let can_move = (speed > 0. && along(axis, old) < px(0.))
                || (speed < 0. && along(axis, old) > -max);
            if !can_move {
                continue;
            }
            drifting = true;
            let value = (along(axis, old) + px(speed * dt)).clamp(-max, px(0.));
            scroll.set_offset(match axis {
                Axis::Horizontal => point(value, old.y),
                Axis::Vertical => point(old.x, value),
            });
        }
        match drifting {
            true => {
                self.drift_since = Some(now);
                self.painter.lease(120., Duration::from_millis(100), cx);
            }
            false => self.drift_since = None,
        }
    }
}

pub(super) enum Commit<R, I> {
    Drop(Rc<OnDrop<R, I>>, Drop<R, I>),
    Outside(Rc<OnOutside<I>>, Outside<I>),
}
