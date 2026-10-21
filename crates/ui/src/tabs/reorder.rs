//! The horizontal, single-region adapter for [`crate::drag`].

use std::{cell::RefCell, rc::Rc};
use web_time::Instant;

use gpui::{
    App, Axis, Bounds, Corners, Div, ElementId, IntoElement, Pixels, Point, RenderOnce,
    ScrollHandle, Stateful, Window, canvas, div, fill, prelude::*, px, relative, size,
    transparent_black,
};
use motion::TAB_SLIDE;
use theme::Theme;

use super::{GAP, Strip, TRACK_HEIGHT, TRACK_MIN_WIDTH, TRACK_PAD, Track};
use crate::drag::{self, Domain};
use crate::widgets;

/// Persistent gesture and animation state, one per strip.
///
/// Its tabs take no `on_drag` of their own. A control inside a tab stops
/// mouse-down propagation, as [`super::close`] does, or a press on it picks up
/// the tab.
pub struct Reorder<Id> {
    domain: Domain<(), Id>,
    scroll: ScrollHandle,
    pill: Rc<RefCell<Pill<Id>>>,
}

impl<Id> Clone for Reorder<Id> {
    fn clone(&self) -> Self {
        Self {
            domain: self.domain.clone(),
            scroll: self.scroll.clone(),
            pill: self.pill.clone(),
        }
    }
}

/// The front tab's pill on a [`Track`], drawn under the tabs so it can slide
/// from the tab that was in front to the one that is.
struct Pill<Id> {
    /// Each tab's box as laid out this frame, in the window's coordinates.
    places: Vec<(Id, Bounds<Pixels>)>,
    /// The tab the pill stands under, and the box it was last drawn in.
    active: Option<Id>,
    drawn: Option<Bounds<Pixels>>,
    /// Where a slide set out from, and when.
    from: Option<(Bounds<Pixels>, Instant)>,
}

impl<Id> Default for Pill<Id> {
    fn default() -> Self {
        Self {
            places: Vec::new(),
            active: None,
            drawn: None,
            from: None,
        }
    }
}

impl<Id: Clone + PartialEq> Pill<Id> {
    /// The box the pill is drawn in this frame, in the coordinates of the row
    /// whose top left is `origin`, and whether it is still sliding. The row and
    /// its tabs scroll together, so a tab's place in it holds while it scrolls.
    fn place(
        &mut self,
        active: Option<&Id>,
        origin: Point<Pixels>,
        cx: &App,
    ) -> (Option<Bounds<Pixels>>, bool) {
        if self.active.as_ref() != active {
            self.from = self
                .drawn
                .map(|drawn| (drawn, cx.background_executor().now()));
            self.active = active.cloned();
        }
        let to = self
            .places
            .iter()
            .find(|(id, _)| Some(id) == active)
            .map(|(_, tab)| Bounds::new(tab.origin - origin, tab.size));
        let Some(to) = to else {
            self.drawn = None;
            return (None, false);
        };
        let t = match self.from {
            Some((_, since)) if !cx.reduce_motion() => {
                let elapsed = cx
                    .background_executor()
                    .now()
                    .saturating_duration_since(since);
                let total = TAB_SLIDE.total().mul_f32(motion::speed_scale());
                TAB_SLIDE.progress(elapsed.as_secs_f32() / total.as_secs_f32().max(f32::EPSILON))
            }
            _ => 1.,
        };
        let at = match self.from {
            Some((from, _)) if t < 1. => Bounds::new(
                from.origin + (to.origin - from.origin) * t,
                size(
                    from.size.width + (to.size.width - from.size.width) * t,
                    from.size.height + (to.size.height - from.size.height) * t,
                ),
            ),
            _ => {
                self.from = None;
                to
            }
        };
        self.drawn = Some(at);
        (Some(at), self.from.is_some())
    }
}

impl<Id: Clone + PartialEq + 'static> Default for Reorder<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq + 'static> Reorder<Id> {
    pub fn new() -> Self {
        Self {
            domain: Domain::new(),
            scroll: ScrollHandle::new(),
            pill: Rc::default(),
        }
    }

    /// Supply tabs in model order, each with a key stable across renders. The
    /// preview moves live; host order changes on release.
    pub fn bar(
        &self,
        id: impl Into<ElementId>,
        strip: &Strip<Id>,
        tabs: impl IntoIterator<Item = (Id, Stateful<Div>)>,
    ) -> Bar<Id> {
        let tabs: Vec<_> = tabs.into_iter().collect();
        debug_assert!(tabs.iter().map(|(id, _)| id).eq(strip.tabs()));
        if self
            .domain
            .carried()
            .is_some_and(|carried| !strip.contains(&carried))
        {
            self.domain.invalidate();
        }
        Bar {
            reorder: self.clone(),
            id: id.into(),
            tabs,
            active: strip.active().cloned(),
            track: None,
            moved: None,
            outside: None,
        }
    }
}

/// A committed move. The active tab remains the host's choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: usize,
    pub to: usize,
}

/// A carried tab released outside its strip; local reordering is cancelled.
#[derive(Clone, Debug)]
pub struct OutsideDrop<Id> {
    pub id: Id,
    pub position: Point<Pixels>,
}

type OnMove = dyn Fn(&Move, &mut Window, &mut App);
type OnOutside<Id> = dyn Fn(&OutsideDrop<Id>, &mut Window, &mut App);

#[derive(IntoElement)]
pub struct Bar<Id: Clone + PartialEq + 'static> {
    reorder: Reorder<Id>,
    id: ElementId,
    tabs: Vec<(Id, Stateful<Div>)>,
    active: Option<Id>,
    track: Option<Track>,
    moved: Option<Rc<OnMove>>,
    outside: Option<Rc<OnOutside<Id>>>,
}

impl<Id: Clone + PartialEq + 'static> Bar<Id> {
    /// Stand the tabs on a track, filling the box the bar is put in.
    pub fn on_track(mut self, track: Track) -> Self {
        self.track = Some(track);
        self
    }

    /// Apply the final move synchronously. Accepts `cx.listener`.
    pub fn on_reorder(mut self, moved: impl Fn(&Move, &mut Window, &mut App) + 'static) -> Self {
        self.moved = Some(Rc::new(moved));
        self
    }

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
        let raised = theme.surface_raised;
        let Reorder {
            domain,
            scroll,
            pill,
        } = self.reorder;
        let order: Vec<Id> = self.tabs.iter().map(|(id, _)| id.clone()).collect();
        let (track, active) = (self.track, self.active);
        pill.borrow_mut()
            .places
            .retain(|(id, _)| order.contains(id));
        let flat = track == Some(Track::Flat);
        // On a track the front tab's pill is drawn under the tabs rather than
        // by the tab, so it can slide between them. It is placed when painted:
        // every tab has been laid out and measured by then, this frame.
        let under = track.map(|_| {
            let (pill, active, theme) = (pill.clone(), active.clone(), theme.clone());
            canvas(
                |_, _, _| {},
                move |row, _, window, cx| {
                    let (at, sliding) = pill.borrow_mut().place(active.as_ref(), row.origin, cx);
                    if sliding {
                        window.request_animation_frame();
                    }
                    let Some(at) = at else {
                        return;
                    };
                    let bounds = Bounds::new(row.origin + at.origin, at.size);
                    let corners = Corners::all(match flat {
                        true => px(Theme::control_radius()),
                        false => at.size.height / 2.,
                    });
                    window.paint_quad(fill(bounds, theme.card_selected_bg()).corner_radii(corners));
                    window.paint_inset_shadows(bounds, corners, &theme::glass_selected_shadows());
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        });
        // A line before every tab but the first, unless the pill is on either
        // side of it. Inside the tab and out of its flow, so no tab moves when
        // the pill does.
        let parted = |at: usize| {
            at > 0 && active.as_ref() != order.get(at) && active.as_ref() != order.get(at - 1)
        };
        let row = div()
            .id("tab-row")
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(GAP))
            .overflow_x_scroll()
            .track_scroll(&scroll)
            .children(under)
            .children(self.tabs.into_iter().enumerate().map(|(at, (id, tab))| {
                let shown = track.is_some() && active.as_ref() == Some(&id);
                let measure = {
                    let (pill, id) = (pill.clone(), id.clone());
                    canvas(
                        move |bounds, _, _| {
                            let mut pill = pill.borrow_mut();
                            match pill.places.iter_mut().find(|(at, _)| *at == id) {
                                Some((_, place)) => *place = bounds,
                                None => pill.places.push((id, bounds)),
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full()
                };
                let tab = match track {
                    None => tab.flex_none(),
                    Some(_) => tab
                        .when(shown, |tab| tab.bg(transparent_black()).shadow(Vec::new()))
                        .child(measure)
                        .relative()
                        .flex_1()
                        .min_w(px(TRACK_MIN_WIDTH))
                        .h(px(widgets::on_track_height(TRACK_HEIGHT, TRACK_PAD)))
                        .max_w(relative(1.))
                        .justify_center()
                        .map(|tab| match flat {
                            true => tab.rounded(px(Theme::control_radius())),
                            false => tab.rounded_full(),
                        })
                        .when(parted(at), |tab| {
                            tab.child(
                                div()
                                    .absolute()
                                    // Past the tab's own 1px border, to the
                                    // middle of the gap.
                                    .left(px(-(1.0 + GAP / 2.0) - 0.5))
                                    .top(px(5.0))
                                    .bottom(px(5.0))
                                    .w(px(1.0))
                                    .bg(theme.border),
                            )
                        }),
                };
                let tab = match domain.carries(&id) {
                    true => tab.bg(raised).cursor_grabbing(),
                    false => tab,
                };
                domain.handle(id, tab)
            }));
        let mut region = domain
            .region("tab-region", (), Axis::Horizontal, row)
            .axis_locked()
            .track_scroll(&scroll)
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex();
        if let Some(moved) = self.moved {
            region = region.on_drop(move |event: &drag::Drop<(), Id>, window, cx| {
                let Some(from) = order.iter().position(|id| *id == event.item) else {
                    return;
                };
                let rest: Vec<&Id> = order.iter().filter(|id| **id != event.item).collect();
                let to = match (&event.after, &event.before) {
                    (Some(after), _) => rest.iter().position(|id| *id == after).map(|at| at + 1),
                    (None, Some(before)) => rest.iter().position(|id| *id == before),
                    (None, None) => Some(0),
                };
                if let Some(to) = to {
                    moved(&Move { from, to }, window, cx);
                }
            });
        }
        if let Some(outside) = self.outside {
            region = region.on_drop_outside(move |event: &drag::Outside<Id>, window, cx| {
                outside(
                    &OutsideDrop {
                        id: event.item.clone(),
                        position: event.position,
                    },
                    window,
                    cx,
                );
            });
        }
        // Sized to its tabs: the bar grows into this box, not the row it sits in.
        // On a track it fills the box it is put in.
        let frame = match track {
            None => div(),
            Some(track) => widgets::track(&theme)
                .p(px(TRACK_PAD))
                .h(px(TRACK_HEIGHT))
                .rounded_full()
                .flex_1()
                .when(track != Track::Bordered, |frame| {
                    frame.border_color(transparent_black())
                })
                .when(track == Track::Flat, |frame| frame.bg(transparent_black())),
        };
        frame
            .id(self.id)
            .relative()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_row()
            .child(super::bar("tab-bar").viewport(region.into_any_element(), &scroll))
    }
}
