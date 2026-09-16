//! Variable-height virtual lists with stable keys and viewport notifications.
use gpui::{
    AnyElement, App, FocusHandle, ListAlignment, ListOffset, ListState, Pixels, Window, prelude::*,
    px,
};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};

/// A tail-following list that builds visible rows plus 500px of overscan.
/// Keys must be unique. Call `sync` before rendering and `invalidate` when an
/// offscreen item's height changes. Use `state.remeasure()` after font changes.
#[derive(Clone)]
pub struct VariableList<K> {
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
            state,
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
    fn scrollbar(&self) -> impl IntoElement {
        let state = self.state.clone();
        let grab = self.grab.clone();
        gpui::canvas(
            move |bounds, _, cx| {
                if crate::scroll::visibility(cx) == crate::scroll::Visibility::Never {
                    return None;
                }
                let viewport = state.viewport_bounds();
                let max = state.max_offset_for_scrollbar().y;
                let offset = state.scroll_px_offset_for_scrollbar().y;
                let range = crate::scroll::thumb_in_track(
                    viewport.size.height,
                    max,
                    offset,
                    bounds.size.height,
                )?;
                Some((
                    gpui::Bounds::new(
                        gpui::point(bounds.left() + px(3.), bounds.top() + range.start),
                        gpui::size(px(4.), range.end - range.start),
                    ),
                    max,
                    bounds.size.height - (range.end - range.start),
                ))
            },
            {
                let state = self.state.clone();
                move |bounds,
                      geometry: Option<(gpui::Bounds<Pixels>, Pixels, Pixels)>,
                      window,
                      cx| {
                    let Some((thumb, max, travel)) = geometry else {
                        return;
                    };
                    let color = theme::Theme::of(cx).text_muted.opacity(0.4);
                    window.paint_quad(gpui::fill(thumb, color).corner_radii(px(2.)));
                    let down_state = state.clone();
                    let down_grab = grab.clone();
                    window.on_mouse_event(
                        move |event: &gpui::MouseDownEvent, phase, window, cx| {
                            if phase == gpui::DispatchPhase::Bubble
                                && event.button == gpui::MouseButton::Left
                                && bounds.contains(&event.position)
                            {
                                down_state.scrollbar_drag_started();
                                down_grab.set(Some((
                                    event.position.y,
                                    down_state.scroll_px_offset_for_scrollbar().y,
                                )));
                                cx.stop_propagation();
                                window.refresh();
                            }
                        },
                    );
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
                        if event.button == gpui::MouseButton::Left && up_grab.take().is_some() {
                            up_state.scrollbar_drag_ended();
                            window.refresh();
                        }
                    });
                }
            },
        )
        .absolute()
        .right_0()
        .top_0()
        .bottom(self.end_inset.get())
        .w(px(10.))
    }
}
