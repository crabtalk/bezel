//! Variable-height virtual lists with stable keys and viewport notifications.
use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, Empty, FocusHandle, IntoElement,
    ListAlignment, ListOffset, ListState, MouseButton, Pixels, RenderOnce, SharedString, Window,
    prelude::*, px,
};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A tail-following list that builds visible rows plus 500px of overscan.
/// Keys must be unique. Call `sync` before rendering and `invalidate` when an
/// offscreen item's height changes. Use `state.remeasure()` after font changes.
#[derive(Clone)]
pub struct VariableList<K> {
    id: usize,
    activity: Rc<Cell<ScrollbarActivity>>,
    hover: Rc<Cell<crate::scroll::Hover>>,
    pub state: ListState,
    keys: Rc<RefCell<Vec<K>>>,
    visible: Rc<RefCell<Range<usize>>>,
    grab: Rc<Cell<Option<(Pixels, Pixels)>>>,
    end_inset: Rc<Cell<Pixels>>,
}
impl<K: Clone + Eq + 'static> Default for VariableList<K> {
    fn default() -> Self {
        let state = ListState::new(0, ListAlignment::Top, px(500.));
        state.set_follow_mode(gpui::FollowMode::Tail);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            state,
            activity: Default::default(),
            hover: Default::default(),
            keys: Default::default(),
            visible: Default::default(),
            grab: Default::default(),
            end_inset: Default::default(),
        }
    }
}
impl<K: Clone + Eq + 'static> VariableList<K> {
    /// Reconcile insertions/removals while retaining the visible item's pixel anchor.
    pub fn sync(&self, next: Vec<K>) {
        let mut keys = self.keys.borrow_mut();
        if *keys == next {
            return;
        }
        let top = self.state.logical_scroll_top();
        let anchor = keys.get(top.item_ix).cloned();
        let following = self.state.is_following_tail();
        let prefix = keys.iter().zip(&next).take_while(|(a, b)| a == b).count();
        let suffix = keys[prefix..]
            .iter()
            .rev()
            .zip(next[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        self.state
            .splice(prefix..keys.len() - suffix, next.len() - prefix - suffix);
        if !following
            && let Some(ix) = anchor.and_then(|key| next.iter().position(|item| *item == key))
        {
            self.state.scroll_to(ListOffset {
                item_ix: ix,
                offset_in_item: top.offset_in_item,
            });
        }
        *keys = next;
    }
    pub fn invalidate(&self, key: &K) {
        if let Some(ix) = self.keys.borrow().iter().position(|item| item == key) {
            self.state.remeasure_items(ix..ix + 1);
        }
    }
    /// Keep the scrollbar clear of floating controls at the bottom.
    pub fn set_end_inset(&self, inset: Pixels) {
        self.end_inset.set(inset);
    }

    pub fn visible_range(&self) -> Range<usize> {
        self.visible.borrow().clone()
    }
    pub fn scroll_to(&self, index: usize) {
        self.state.pause_following_tail();
        self.state.scroll_to(ListOffset {
            item_ix: index,
            offset_in_item: px(0.),
        });
    }
    /// Keep keyboard interaction alive for a selected item outside the viewport.
    pub fn focus_item(&self, index: usize, focus: Option<FocusHandle>) {
        let top = self.state.logical_scroll_top();
        let following = self.state.is_following_tail();
        self.state.splice_focusable(index..index + 1, [focus]);
        if !following {
            self.state.scroll_to(top);
        }
    }
    pub fn render(
        &self,
        render: impl FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static,
        on_visible: impl Fn(Range<usize>, &mut Window, &mut App) + 'static,
    ) -> gpui::Div {
        let bar = self.scrollbar();
        let state = self.state.clone();
        let visible = self.visible.clone();
        gpui::div()
            .size_full()
            .relative()
            .child(gpui::list(self.state.clone(), render).size_full())
            .child(
                gpui::canvas(
                    move |_, window, cx| {
                        let start = state.logical_scroll_top().item_ix.min(state.item_count());
                        let mut end = start;
                        let viewport = state.viewport_bounds();
                        while end < state.item_count() {
                            if let Some(bounds) = state.bounds_for_item(end) {
                                if bounds.top() >= viewport.bottom() {
                                    break;
                                }
                                end += 1;
                            } else {
                                break;
                            }
                        }
                        let range = start..end;
                        if *visible.borrow() != range {
                            *visible.borrow_mut() = range.clone();
                            on_visible(range, window, cx);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(bar)
    }
    fn scrollbar(&self) -> Scrollbar {
        Scrollbar {
            id: self.id,
            state: self.state.clone(),
            grab: self.grab.clone(),
            activity: self.activity.clone(),
            hover: self.hover.clone(),
            end_inset: self.end_inset.get(),
        }
    }
}

/// Element ids must not collide between lists sharing a window.
static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Default)]
struct ScrollbarActivity {
    offset: Pixels,
    max: Pixels,
    generation: u64,
}

/// The list's thumb, shaped like [`crate::scroll::transient`]: the fade *is*
/// the idle window, and a fresh animation id restarts it, because
/// `AnimationElement` pins its clock to the id it first laid out with.
///
/// Geometry comes from the previous frame — layout hands the list its viewport
/// after this element is built — so the strip asks for another frame whenever
/// the two disagree.
#[derive(IntoElement)]
struct Scrollbar {
    id: usize,
    state: ListState,
    grab: Rc<Cell<Option<(Pixels, Pixels)>>>,
    activity: Rc<Cell<ScrollbarActivity>>,
    hover: Rc<Cell<crate::scroll::Hover>>,
    end_inset: Pixels,
}

impl RenderOnce for Scrollbar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mode = crate::scroll::visibility(cx);
        if mode == crate::scroll::Visibility::Never {
            return Empty.into_any_element();
        }
        let always = mode == crate::scroll::Visibility::Always || cx.reduce_motion();
        let viewport = self.state.viewport_bounds().size.height;
        let max = self.state.max_offset_for_scrollbar().y;
        let offset = self.state.scroll_px_offset_for_scrollbar().y;
        let track = (viewport - self.end_inset).max(px(0.));

        // Any change in the scroll state is activity. The half-pixel slack
        // keeps a sub-pixel layout jitter from re-showing a settled bar.
        let mut held = self.activity.get();
        if (offset - held.offset).abs() > px(0.5) || (max - held.max).abs() > px(0.5) {
            held.offset = offset;
            held.max = max;
            held.generation += 1;
            self.activity.set(held);
        }
        let generation = held.generation;

        let range = crate::scroll::thumb_in_track(viewport, max, offset, track);
        let travel = range
            .as_ref()
            .map(|range| track - (range.end - range.start))
            .unwrap_or(px(0.));
        let thumb = range.map(|range| {
            let down_state = self.state.clone();
            let down_grab = self.grab.clone();
            let thumb = gpui::div()
                .id(SharedString::from(format!("list-{}-thumb", self.id)))
                .absolute()
                .top(range.start)
                .h(range.end - range.start)
                .w(px(crate::scroll::THUMB))
                .rounded_full()
                .bg(theme::Theme::of(cx).text_muted.opacity(0.4))
                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    down_state.scrollbar_drag_started();
                    down_grab.set(Some((
                        event.position.y,
                        down_state.scroll_px_offset_for_scrollbar().y,
                    )));
                    cx.stop_propagation();
                    window.refresh();
                });
            if always {
                return thumb.into_any_element();
            }
            // The thumb's presence is the animation: progress 0 is fully up,
            // and progress 1 — a full idle window later — is hidden, so no
            // frame is requested once the fade completes. A hidden thumb is
            // also out of the hit test, which is what hovering the strip
            // raises it for.
            let anim_hover = self.hover.clone();
            let anim_grab = self.grab.clone();
            thumb
                .with_animation(
                    ElementId::from(SharedString::from(format!(
                        "list-{}-fade-{generation}",
                        self.id
                    ))),
                    Animation::new(crate::scroll::TRANSIENT_IDLE),
                    move |el, progress| {
                        if anim_hover.get().held() || anim_grab.get().is_some() {
                            el
                        } else if progress < 1.0 {
                            el.opacity(1.0 - progress)
                        } else {
                            el.hidden()
                        }
                    },
                )
                .into_any_element()
        });

        let activity = self.activity.clone();
        let state = self.state.clone();
        let grab = self.grab.clone();
        let strip = gpui::div()
            .id(SharedString::from(format!("list-{}-track", self.id)))
            .absolute()
            .right_0()
            .top_0()
            .bottom(self.end_inset)
            .w(px(10.))
            .flex()
            .justify_center();
        crate::scroll::hover_track(strip, self.hover, move |window, _| {
            let mut held = activity.get();
            held.generation += 1;
            activity.set(held);
            window.refresh();
        })
        .children(thumb)
        .child(
            gpui::canvas(
                move |bounds, window, _| {
                    // Laid out against a viewport the geometry above has
                    // not seen. Self-limiting: once they agree, nothing is
                    // requested.
                    if (bounds.size.height - track).abs() > px(0.5) {
                        window.request_animation_frame();
                    }
                },
                move |_, _, window, _| {
                    // Window-wide, because a drag leaves the strip and a
                    // release can land anywhere: a grab left set would make
                    // the next press continue the last gesture.
                    let moved_state = state.clone();
                    let moved_grab = grab.clone();
                    window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, window, _| {
                        if phase == gpui::DispatchPhase::Bubble
                            && let Some((start, offset)) = moved_grab.get()
                            && travel > px(0.)
                        {
                            moved_state.set_offset_from_scrollbar(gpui::point(
                                px(0.),
                                (offset - (event.position.y - start) / travel * max)
                                    .clamp(-max, px(0.)),
                            ));
                            window.refresh();
                        }
                    });
                    let up_state = state.clone();
                    let up_grab = grab.clone();
                    window.on_mouse_event(move |event: &gpui::MouseUpEvent, _, window, _| {
                        if event.button == MouseButton::Left && up_grab.take().is_some() {
                            up_state.scrollbar_drag_ended();
                            window.refresh();
                        }
                    });
                },
            )
            .absolute()
            .size_full(),
        )
        .into_any_element()
    }
}
