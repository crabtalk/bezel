//! A view's menu: a right press on a text surface, a `···` on a row, a `+` in
//! a header.
//!
//! It holds the keyboard while it is open: the arrows walk its rows and its
//! submenus, `enter` runs the live row or opens its submenu, and `escape` backs
//! out of a submenu and then closes it, under [`KEY_CONTEXT`], whose bindings
//! are in [`crate::menu::bindings`] and installed by [`crate::menu::init`].
//! Closing by key or by a chosen row
//! hands the focus back to whatever held it before the menu opened. A press
//! elsewhere closes it and leaves the focus to that press.
//!
//! One `ContextMenu` serves every menu a view has: `K` names which one is open,
//! so opening one closes the rest.

use crate::{
    menu::{
        self, Confirm, Cursor, Dismiss, Hit, Item, SelectChild, SelectNext, SelectParent,
        SelectPrevious,
    },
    popover::{self, Popup},
};
use gpui::{
    AnyElement, Context, Div, FocusHandle, Pixels, Point, SharedString, StatefulInteractiveElement,
    Window, prelude::*,
};
use std::rc::Rc;
use theme::Theme;

/// The key context an open menu claims.
pub const KEY_CONTEXT: &str = "ContextMenu";

/// What choosing a row runs, given the row's path into the items.
type OnChoose<V> = Rc<dyn Fn(&mut V, &[usize], &mut Window, &mut Context<V>)>;

/// Where an owning view keeps its menu.
pub type Slot<V, K = ()> = fn(&mut V) -> &mut ContextMenu<V, K>;

/// Where an open menu stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// At a window point: where a right press landed.
    At(Point<Pixels>),
    /// Down from the trigger's bottom edge, left edges aligned.
    Below,
    /// Down from the trigger's bottom edge, right edges aligned.
    BelowEnd,
    /// Up from the trigger's top edge, left edges aligned.
    Above,
    /// Up from the trigger's top edge, right edges aligned.
    AboveEnd,
}

struct Open<V, K> {
    key: K,
    place: Place,
    cursor: Cursor,
    items: Vec<Item>,
    on_choose: OnChoose<V>,
    focus: FocusHandle,
    back: Option<FocusHandle>,
}

/// A view's menu: closed, or open with its rows and what choosing one does.
pub struct ContextMenu<V: 'static, K: 'static = ()>(Popup<Open<V, K>>);

impl<V: 'static, K: 'static> Default for ContextMenu<V, K> {
    fn default() -> Self {
        Self(Popup::default())
    }
}

impl<V: 'static, K: PartialEq + 'static> ContextMenu<V, K> {
    /// Open `items` as the menu `key`, standing at `place` and taking the
    /// keyboard. A menu already open is replaced.
    pub fn open(
        &mut self,
        key: K,
        place: Place,
        items: Vec<Item>,
        on_choose: impl Fn(&mut V, &[usize], &mut Window, &mut Context<V>) + 'static,
        window: &mut Window,
        cx: &mut Context<V>,
    ) {
        // A menu replacing another hands the focus back to what held it before
        // the first.
        let back = match self.0.as_open() {
            Some(open) => open.back.clone(),
            None => window.focused(cx),
        };
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.0.open(Open {
            key,
            place,
            cursor: Cursor::default(),
            items,
            on_choose: Rc::new(on_choose),
            focus,
            back,
        });
        cx.notify();
    }

    /// Open the menu `key`, or close it when it is the one up or the press
    /// that led here found it up — see [`Self::note_press`]. `build` makes the
    /// rows and what choosing one does, and runs only when the menu opens.
    pub fn toggle<F>(
        view: &mut V,
        slot: Slot<V, K>,
        key: K,
        place: Place,
        build: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> (Vec<Item>, F),
        window: &mut Window,
        cx: &mut Context<V>,
    ) where
        F: Fn(&mut V, &[usize], &mut Window, &mut Context<V>) + 'static,
    {
        let menu = slot(view);
        if menu.0.take_press_was_open() || menu.key() == Some(&key) {
            Self::close_back(view, slot, window, cx);
            return;
        }
        let (items, on_choose) = build(view, window, cx);
        slot(view).open(key, place, items, on_choose, window, cx);
    }

    /// Note, from a trigger's press, whether the menu `key` is on screen. The
    /// open card dismisses on that same press, so by click time the menu reads
    /// as closed, and [`Self::toggle`] would open it straight back without
    /// this.
    pub fn note_press(&mut self, key: &K) {
        self.0.note_trigger_press_matching(|open| open.key == *key);
    }

    /// `el`, noting its presses for the menu `key` — see [`Self::note_press`].
    /// Every trigger that calls [`Self::toggle`] needs it.
    pub fn trigger<E: StatefulInteractiveElement>(
        el: E,
        slot: Slot<V, K>,
        key: K,
        cx: &Context<V>,
    ) -> E {
        // Capture, for the reason `popover::trigger_press_matching` gives.
        el.capture_any_mouse_down(cx.listener(move |view, _: &gpui::MouseDownEvent, _, _| {
            slot(view).note_press(&key);
        }))
    }

    /// Which menu is open. `None` while none is, or the last one is on its way
    /// out.
    pub fn key(&self) -> Option<&K> {
        self.0.as_open().map(|open| &open.key)
    }

    pub fn is_open(&self) -> bool {
        self.0.is_open()
    }

    /// Whether the menu `key` is on screen, open or on its way out.
    pub fn showing(&self, key: &K) -> bool {
        self.0.get().is_some_and(|open| open.key == *key)
    }

    /// Light the row at `path`, as the pointer would. Whether that moved the
    /// cursor.
    pub fn point_at(&mut self, path: &[usize]) -> bool {
        self.0
            .open_mut()
            .is_some_and(|open| open.cursor.point_at(&open.items, path))
    }

    /// Open the submenu under the live row. Whether there was one.
    fn descend(&mut self) -> bool {
        self.0
            .open_mut()
            .is_some_and(|open| open.cursor.descend(&open.items))
    }

    /// Close the innermost open submenu. Whether one was open.
    fn ascend(&mut self) -> bool {
        self.0.open_mut().is_some_and(|open| open.cursor.ascend())
    }

    /// Light no row.
    pub fn clear_cursor(&mut self) {
        if let Some(open) = self.0.open_mut() {
            open.cursor.clear();
        }
    }

    /// Close the menu, leaving the focus where it is.
    pub fn close(view: &mut V, slot: Slot<V, K>, cx: &mut Context<V>) {
        popover::close_popup(view, cx, move |view| &mut slot(view).0);
    }

    /// Close the menu and give the keyboard back to what held it before.
    fn close_back(view: &mut V, slot: Slot<V, K>, window: &mut Window, cx: &mut Context<V>) {
        let back = slot(view).0.as_open().and_then(|open| open.back.clone());
        Self::close(view, slot, cx);
        if let Some(back) = back {
            window.focus(&back, cx);
        }
    }

    /// The menu's card while it is on screen, not yet placed: for a caller
    /// that sizes it before [`Self::place`]. `id` prefixes its rows' element
    /// ids and debug selectors — see [`menu::card`].
    pub fn card(
        &self,
        id: impl Into<SharedString>,
        slot: Slot<V, K>,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Option<Div> {
        let open = self.0.get()?;
        let theme = Theme::of(cx).clone();
        let choose = open.on_choose.clone();
        let card = menu::card(
            &theme,
            id,
            &open.items,
            &open.cursor,
            self.0.closing_since(),
            window,
            cx,
            move |view: &mut V, hit, window, cx| match hit {
                Hit::Point(path) => {
                    if slot(view).point_at(&path) {
                        cx.notify();
                    }
                }
                Hit::Choose(path) => {
                    Self::close_back(view, slot, window, cx);
                    choose(view, &path, window, cx);
                }
                Hit::Dismiss => Self::close(view, slot, cx),
            },
        )
        .track_focus(&open.focus)
        .key_context(KEY_CONTEXT)
        .on_action(cx.listener(move |view, _: &SelectNext, _, cx| step(view, slot, 1, cx)))
        .on_action(cx.listener(move |view, _: &SelectPrevious, _, cx| step(view, slot, -1, cx)))
        .on_action(cx.listener(move |view, _: &SelectChild, _, cx| {
            if slot(view).descend() {
                cx.notify();
            }
        }))
        .on_action(cx.listener(move |view, _: &SelectParent, _, cx| {
            if slot(view).ascend() {
                cx.notify();
            }
        }))
        .on_action(cx.listener(move |view, _: &Confirm, window, cx| {
            if slot(view).descend() {
                cx.notify();
                return;
            }
            let Some((path, choose)) = slot(view)
                .0
                .as_open()
                .and_then(|open| Some((open.cursor.path()?, open.on_choose.clone())))
            else {
                return;
            };
            Self::close_back(view, slot, window, cx);
            choose(view, &path, window, cx);
        }))
        .on_action(cx.listener(
            move |view, _: &Dismiss, window, cx| match slot(view).ascend() {
                true => cx.notify(),
                false => Self::close_back(view, slot, window, cx),
            },
        ));
        Some(card)
    }

    /// Stand `card` where the menu opened. For every [`Place`] but
    /// [`Place::At`], draw it as a child of the trigger.
    pub fn place(&self, id: impl Into<SharedString>, card: impl IntoElement) -> AnyElement {
        let card = card.into_any_element();
        let closing = self.0.closing_since();
        match self.0.get().map(|open| open.place) {
            Some(Place::At(at)) => popover::menu_at(id, at, card, closing),
            Some(Place::BelowEnd) => popover::anchored_menu_below_end(id, card, closing),
            Some(Place::Above) => popover::anchored_menu_above(id, card, closing),
            Some(Place::AboveEnd) => popover::anchored_menu_above_end(id, card, closing),
            Some(Place::Below) | None => popover::anchored_menu_below(id, card, closing),
        }
    }

    /// The menu, placed, while it is on screen.
    pub fn render(
        &self,
        id: impl Into<SharedString>,
        slot: Slot<V, K>,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Option<AnyElement> {
        let id = id.into();
        let card = self.card(id.clone(), slot, window, cx)?;
        Some(self.place(id, card))
    }

    /// [`Self::render`] for the menu `key` alone: what a trigger hangs beside
    /// itself.
    pub fn render_for(
        &self,
        key: &K,
        id: impl Into<SharedString>,
        slot: Slot<V, K>,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Option<AnyElement> {
        if !self.showing(key) {
            return None;
        }
        self.render(id, slot, window, cx)
    }
}

fn step<V: 'static, K: PartialEq + 'static>(
    view: &mut V,
    slot: Slot<V, K>,
    delta: isize,
    cx: &mut Context<V>,
) {
    if let Some(open) = slot(view).0.open_mut() {
        open.cursor.step(&open.items, delta);
        cx.notify();
    }
}
