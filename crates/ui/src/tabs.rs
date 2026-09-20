//! Tabs — an ordered strip of open things, one of them in front.
//!
//! Not [`toggle_group`](crate::widgets::Controls::toggle_group), which picks a
//! value out of a fixed set, and not
//! [`tab_bar`](crate::widgets::Layout::tab_bar), which switches between
//! sections of one page. A tab here has identity: it arrives, it can be closed,
//! it can be dragged past its neighbour, and the strip outlives any particular
//! membership.
//!
//! [`Strip`] is the order and the activation, and imports no gpui — closing,
//! cycling and reordering are `Vec` arithmetic, testable without a window. The
//! paint is [`bar`], [`tab`] and [`close`].
//!
//! What a tab *holds* never enters this module. `Id` is the caller's key, and
//! the body it opens is the caller's match on that key.
//!
//! ```ignore
//! ui::tabs::bar("panel-tabs").children(self.strip.tabs().iter().map(|id| {
//!     let key = SharedString::from(format!("panel-{id}"));
//!     let state = match self.strip.active() == Some(id) {
//!         true => tabs::State::Focused,
//!         false => tabs::State::Resting,
//!     };
//!     tabs::tab(&theme, key.clone(), self.label(id), state)
//!         .on_click(cx.listener(move |view, _, _, cx| view.show(id, cx)))
//!         .child(
//!             tabs::close(&theme, key, tabs::Close::OnHover)
//!                 .on_click(cx.listener(move |view, _, _, cx| view.close(id, cx))),
//!         )
//! }))
//! ```

use gpui::{Div, ElementId, SharedString, Stateful, div, prelude::*, px};

use icons::Icon;
use theme::{TextStyle, Theme, Typeset};

use crate::widgets::{self, Buttons as _};

/// An ordered set of tabs, one of them active.
///
/// `Id` is whatever names a tab to its owner — a counter, a path, a layout
/// member. Identity is `PartialEq`, so an id that compares equal to one already
/// in the strip is the same tab.
///
/// A strip with tabs in it always has one in front: [`Self::active`] is `None`
/// only while [`Self::is_empty`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strip<Id> {
    order: Vec<Id>,
    active: Option<Id>,
}

impl<Id> Default for Strip<Id> {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            active: None,
        }
    }
}

impl<Id: Clone + PartialEq> FromIterator<Id> for Strip<Id> {
    /// The first of the run is the one in front.
    fn from_iter<T: IntoIterator<Item = Id>>(iter: T) -> Self {
        let order: Vec<Id> = iter.into_iter().collect();
        let active = order.first().cloned();
        Self { order, active }
    }
}

impl<Id: Clone + PartialEq> Strip<Id> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The tabs, left to right.
    pub fn tabs(&self) -> &[Id] {
        &self.order
    }

    pub fn active(&self) -> Option<&Id> {
        self.active.as_ref()
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn contains(&self, id: &Id) -> bool {
        self.order.contains(id)
    }

    pub fn index_of(&self, id: &Id) -> Option<usize> {
        self.order.iter().position(|held| held == id)
    }

    /// Bring `id` to the front, adding it at the end of the strip if it is not
    /// already there. An id that *is* there keeps its place.
    pub fn open(&mut self, id: Id) {
        if !self.contains(&id) {
            self.order.push(id.clone());
        }
        self.active = Some(id);
    }

    /// Bring an existing tab to the front. `false`, and nothing moves, when it
    /// is not in the strip.
    pub fn activate(&mut self, id: &Id) -> bool {
        let held = self.contains(id);
        if held {
            self.active = Some(id.clone());
        }
        held
    }

    /// Take a tab out. `false` when it was not in the strip.
    ///
    /// Closing the tab in front hands the front to its right-hand neighbour,
    /// or to the new last tab when it had none. Closing any other tab leaves
    /// the front where it is.
    pub fn close(&mut self, id: &Id) -> bool {
        let Some(at) = self.index_of(id) else {
            return false;
        };
        self.order.remove(at);
        if self.active.as_ref() == Some(id) {
            self.active = self.order.get(at).or_else(|| self.order.last()).cloned();
        }
        true
    }

    /// Step the front `step` tabs along, wrapping at both ends. An empty strip
    /// does not move.
    pub fn cycle(&mut self, step: isize) {
        if self.order.is_empty() {
            return;
        }
        let len = self.order.len() as isize;
        let at = self
            .active
            .as_ref()
            .and_then(|id| self.index_of(id))
            .unwrap_or(0) as isize;
        self.active = self
            .order
            .get((at + step).rem_euclid(len) as usize)
            .cloned();
    }

    /// Move the tab at `from` so that it sits at `to`. Out-of-range ends are
    /// ignored rather than clamped: a drag that left the strip did not mean the
    /// last slot.
    ///
    /// The front is held by identity, so reordering never changes which tab is
    /// in front.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from == to || from >= self.order.len() || to >= self.order.len() {
            return;
        }
        let moved = self.order.remove(from);
        self.order.insert(to, moved);
    }
}

/// What a tab shows.
#[derive(Clone, Debug, Default)]
pub struct Label {
    text: SharedString,
    icon: Option<Icon>,
    badge: Option<SharedString>,
    dirty: bool,
}

impl Label {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    /// A glyph before the text — what kind of thing the tab is on.
    pub fn with_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A quiet trailing note: a line number, a reference, a count. It does not
    /// truncate, so keep it to a few characters.
    pub fn with_badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Mark unsaved work with a dot. It sits outside the truncating label, so a
    /// long name cannot hide it.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }
}

/// How a tab reads.
///
/// `Front` and `Focused` are separate because a window can hold several strips:
/// a background pane's own front tab still has to say what is under it, while
/// only one tab in the window has the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Resting,
    Front,
    Focused,
}

/// When a tab's `×` is on show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Close {
    Always,
    OnHover,
}

/// How wide one tab grows before its label truncates.
pub const MAX_WIDTH: f32 = 180.0;

/// Gap between tabs.
const GAP: f32 = 2.0;
/// Inset at a tab's ends, and the gap between what it holds.
const TAB_PAD: f32 = 8.0;

/// The strip. Tabs go in it; a `+`, a `···` and anything else on the row are
/// the caller's, outside this.
///
/// It scrolls sideways once the tabs no longer fit. `min_w_0` is what allows
/// that — a flex child's `min-width: auto` refuses to shrink below its content,
/// so without it the strip grows past its row instead of scrolling.
pub fn bar(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(GAP))
        .overflow_x_scroll()
}

/// One tab, up to the `×`: pass the same `key` to [`close`] and chain the
/// result on as a child.
///
/// `key` names both the element and the hover group [`Close::OnHover`] reads,
/// so the two are derived from one string rather than written twice.
///
/// A resting tab takes its own `hover`, and gpui panics on a second one: reach
/// for [`Close::OnHover`]'s group, or a `group_hover` of your own, rather than
/// chaining `.hover(..)` onto what this returns.
pub fn tab(
    theme: &Theme,
    key: impl Into<SharedString>,
    label: Label,
    state: State,
) -> Stateful<Div> {
    let key = key.into();
    let group = group_of(&key);
    let tint = match state {
        State::Resting => theme.text_muted,
        State::Front | State::Focused => theme.text,
    };
    let wash = theme.element_hover;
    div()
        .id(ElementId::from(SharedString::from(format!("tab-{key}"))))
        .group(group)
        // Sized to its label, not to the bar: a tab stretched across the strip
        // reads as a field rather than a label.
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(Theme::BUTTON_HEIGHT))
        .max_w(px(MAX_WIDTH))
        .px(px(TAB_PAD))
        .rounded(px(Theme::control_radius()))
        // The slot `focus::focusable` fills, kept whether or not it is filled:
        // gpui sizes border-box, so a border that arrived with the ring would
        // shift the label by a pixel.
        .border_1()
        .border_color(widgets::RING_SLOT)
        .text_style(TextStyle::Callout)
        .text_color(tint)
        .cursor_pointer()
        .when(state == State::Focused, |el| el.bg(theme.element_active))
        .when(state == State::Resting, |el| {
            el.hover(move |el| el.bg(wash))
        })
        .children(label.icon.map(|icon| {
            crate::icons::icon(icon)
                .size(px(14.0))
                .flex_none()
                .text_color(theme.text_muted)
        }))
        .child(div().min_w_0().truncate().child(label.text))
        .when(label.dirty, |el| el.child(widgets::status_dot(tint)))
        .children(label.badge.map(|badge| {
            div()
                .flex_none()
                .text_style(TextStyle::Caption)
                .text_color(theme.text_faint)
                .child(badge)
        }))
}

/// A tab's `×`, for the `key` its [`tab`] was built with. The click is the
/// caller's, and has to stop propagating or the tab under it takes the press
/// as well.
pub fn close(theme: &Theme, key: impl Into<SharedString>, when: Close) -> Stateful<Div> {
    let key = key.into();
    let button = theme
        .ghost(ElementId::from(SharedString::from(format!(
            "tab-close-{key}"
        ))))
        .flex_none()
        .p(px(2.0))
        .child(
            crate::icons::icon(crate::icons::glyph::X)
                .size(px(11.0))
                .text_color(theme.text_muted),
        );
    match when {
        Close::Always => button,
        Close::OnHover => button
            .invisible()
            .group_hover(group_of(&key), |el| el.visible()),
    }
}

/// The hover group a tab and its `×` share.
fn group_of(key: &SharedString) -> SharedString {
    SharedString::from(format!("tab-{key}"))
}
