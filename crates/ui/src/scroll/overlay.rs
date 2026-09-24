//! Stateful overlays for either scroll axis.

use super::{self as scroll, ScrollbarState, TransientState};
use gpui::{
    self, Animation, AnimationExt, AnyElement, App, Axis, Div, DragMoveEvent, Empty, Global,
    IntoElement, MouseButton, Pixels, RenderOnce, ScrollHandle, SharedString, Stateful, Window,
    canvas, div, point, prelude::*, px,
};
use motion::Painter;
use std::{cell::Cell, rc::Rc};
use theme::ink;

/// Visibility for overflowing panes; content that fits never draws a bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Scrolling,
    Always,
    Never,
}
impl Global for Visibility {}

pub fn visibility(cx: &App) -> Visibility {
    cx.try_global::<Visibility>().copied().unwrap_or_default()
}

/// Set the default for overlays, including those inside Markdown blocks.
pub fn set_visibility(value: Visibility, cx: &mut App) {
    cx.set_global(value);
    cx.refresh_windows();
}

#[derive(IntoElement)]
pub struct Overlay {
    id: SharedString,
    handle: ScrollHandle,
    axis: Axis,
    visibility: Option<Visibility>,
    place: scroll::Place,
}

impl Overlay {
    /// Mount beside the scroller in a relative wrapper of the same size.
    pub fn new(id: impl Into<SharedString>, handle: &ScrollHandle, axis: Axis) -> Self {
        Self {
            id: id.into(),
            handle: handle.clone(),
            axis,
            visibility: None,
            place: scroll::Place::default(),
        }
    }

    /// Shorten the track to clear an overlaid footer without resizing content.
    pub fn end_inset(mut self, inset: Pixels) -> Self {
        self.place.end = inset.max(px(0.));
        self
    }

    /// Centre the bar in `room` reserved across its axis rather than in the
    /// default strip at the edge. Pass the padding the pane holds beside its
    /// content and the thumb runs down the middle of it.
    pub fn channel(mut self, room: Pixels) -> Self {
        self.place.channel = room.max(px(0.));
        self
    }

    /// Override the default for an individual pane, such as a sidebar.
    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = Some(visibility);
        self
    }
}

/// An intrinsically sized scroll container with its own handle and overlay.
#[derive(IntoElement)]
pub struct Viewport {
    id: SharedString,
    content: Stateful<Div>,
    axis: Axis,
    fill: bool,
    handle: Option<ScrollHandle>,
}

impl Viewport {
    pub fn new(id: impl Into<SharedString>, content: Stateful<Div>, axis: Axis) -> Self {
        Self {
            id: id.into(),
            content,
            axis,
            fill: false,
            handle: None,
        }
    }

    /// Fill the remaining space in a flex container instead of sizing to content.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Scroll it from outside — what a caller needs to ask
    /// [`gpui::ScrollHandle::scroll_to_item`] for one of its children. Left
    /// unset, the viewport keeps a handle of its own that nothing else can
    /// reach.
    pub fn track_scroll(mut self, handle: &ScrollHandle) -> Self {
        self.handle = Some(handle.clone());
        self
    }
}

impl RenderOnce for Viewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = match self.handle {
            Some(handle) => handle,
            None => window
                .use_keyed_state(
                    SharedString::from(format!("{}-handle", self.id)),
                    cx,
                    |_, _| ScrollHandle::new(),
                )
                .read(cx)
                .clone(),
        };
        let axes = match self.axis {
            Axis::Vertical => scroll::Axes::Vertical,
            Axis::Horizontal => scroll::Axes::Horizontal,
        };
        div()
            .relative()
            .w_full()
            .min_w_0()
            .when(self.fill, |el| el.flex_1().min_h_0().flex().flex_col())
            .child(scroll::scrolls(self.content, axes).track_scroll(&handle))
            .child(Overlay::new(self.id, &handle, self.axis))
    }
}

struct State {
    steady: ScrollbarState,
    transient: TransientState,
    horizontal: Rc<Cell<Horizontal>>,
    horizontal_hover: Rc<Cell<scroll::Hover>>,
}

#[derive(Clone, Copy, Default)]
struct Horizontal {
    offset: Pixels,
    max: Pixels,
    generation: usize,
    grab: Option<Pixels>,
}

impl RenderOnce for Overlay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mode = self.visibility.unwrap_or_else(|| visibility(cx));
        if mode == Visibility::Never {
            return Empty.into_any_element();
        }
        let state = window.use_keyed_state(
            SharedString::from(format!("{}-state", self.id)),
            cx,
            |_, cx| State {
                steady: ScrollbarState::new(Painter::of(cx)),
                transient: TransientState::new(Painter::of(cx)),
                horizontal: Rc::default(),
                horizontal_hover: Rc::default(),
            },
        );
        let held = state.read(cx);
        let always = mode == Visibility::Always || cx.reduce_motion();
        let inner = match self.axis {
            Axis::Vertical if always => {
                scroll::scrollbar_placed(self.id, &self.handle, &held.steady, self.place)
            }
            Axis::Vertical => {
                scroll::transient_placed(self.id, &self.handle, &held.transient, false, self.place)
            }
            Axis::Horizontal => horizontal(
                self.id,
                &self.handle,
                held.horizontal.clone(),
                held.horizontal_hover.clone(),
                always,
                self.place,
            ),
        };
        let handle = self.handle;
        let before = (handle.bounds(), handle.max_offset(), handle.offset());
        // Handles receive new geometry during layout, after this render pass.
        div()
            .absolute()
            .inset_0()
            .child(inner)
            .child(
                canvas(
                    move |_, window, _| {
                        if before != (handle.bounds(), handle.max_offset(), handle.offset()) {
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }
}

#[derive(Clone)]
struct HorizontalDrag(SharedString);

fn horizontal(
    id: SharedString,
    handle: &ScrollHandle,
    state: Rc<Cell<Horizontal>>,
    hover: Rc<Cell<scroll::Hover>>,
    always: bool,
    place: scroll::Place,
) -> AnyElement {
    let end_inset = place.end;
    let viewport = handle.bounds().size.width;
    let max = handle.max_offset().x;
    let Some(range) = scroll::thumb_in_track(
        viewport,
        max,
        handle.offset().x,
        viewport - 2. * scroll::BAR_INSET - end_inset,
    ) else {
        return Empty.into_any_element();
    };
    let size = range.end - range.start;
    let mut held = state.get();
    if (held.offset - handle.offset().x).abs() > px(0.5) || (held.max - max).abs() > px(0.5) {
        held.offset = handle.offset().x;
        held.max = max;
        held.generation += 1;
        state.set(held);
    }
    let drag_id = id.clone();
    let drag_handle = handle.clone();
    let drag_state = state.clone();
    let release_state = state.clone();
    let release = move |_: &gpui::MouseUpEvent, window: &mut Window, _: &mut App| {
        let mut held = release_state.get();
        held.grab = None;
        held.generation += 1;
        release_state.set(held);
        window.refresh();
    };
    let hover_state = state.clone();
    let track = scroll::track(&id, place, Axis::Horizontal);
    let track = scroll::hover_track(track, hover.clone(), move |window, _| {
        let mut held = hover_state.get();
        held.generation += 1;
        hover_state.set(held);
        window.refresh();
    })
    .on_drag_move(move |event: &DragMoveEvent<HorizontalDrag>, window, cx| {
        if event.drag(cx).0 != drag_id {
            return;
        }
        let viewport = drag_handle.bounds().size.width;
        let max = drag_handle.max_offset().x;
        let Some(range) = scroll::thumb_in_track(
            viewport,
            max,
            drag_handle.offset().x,
            viewport - 2. * scroll::BAR_INSET - end_inset,
        ) else {
            return;
        };
        let pointer = event.event.position.x - event.bounds.left();
        let mut held = drag_state.get();
        let grab = *held
            .grab
            .get_or_insert((pointer - range.start).clamp(px(0.), range.end - range.start));
        drag_state.set(held);
        let x = scroll::offset_for_thumb(
            pointer - grab,
            viewport - 2. * scroll::BAR_INSET - end_inset,
            max,
            range.end - range.start,
        );
        drag_handle.set_offset(point(x, drag_handle.offset().y));
        window.refresh();
    })
    .on_mouse_up(MouseButton::Left, release.clone())
    .on_mouse_up_out(MouseButton::Left, release);
    let thumb_debug_id = id.clone();
    let press_state = state.clone();
    let press_handle = handle.clone();
    let thumb = div()
        .debug_selector(move || format!("{thumb_debug_id}-thumb"))
        .id(SharedString::from(format!("{id}-thumb")))
        .absolute()
        .left(range.start)
        .w(size)
        .h(px(scroll::THUMB))
        .rounded_full()
        .bg(ink(0.2))
        .hover(|s| s.bg(ink(0.32)))
        .on_mouse_down(MouseButton::Left, move |event, window, _| {
            let mut held = press_state.get();
            held.grab = Some(
                (event.position.x - press_handle.bounds().left() - scroll::BAR_INSET - range.start)
                    .clamp(px(0.), size),
            );
            press_state.set(held);
            window.refresh();
        })
        .on_drag(HorizontalDrag(id.clone()), |_, _, _, cx| cx.new(|_| Empty));
    let thumb = if always {
        thumb.into_any_element()
    } else {
        thumb
            .with_animation(
                SharedString::from(format!("{id}-fade-{}", held.generation)),
                Animation::new(scroll::TRANSIENT_IDLE),
                move |el, p| {
                    if hover.get().held() || state.get().grab.is_some() {
                        el
                    } else if p < 1. {
                        el.opacity(1. - p)
                    } else {
                        el.hidden()
                    }
                },
            )
            .into_any_element()
    };
    track.child(thumb).into_any_element()
}
