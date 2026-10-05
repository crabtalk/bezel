//! A menu opened at the pointer — a right press on a text surface, say.
//!
//! It holds the keyboard while it is open: the arrows walk its rows, `enter`
//! runs the live one and `escape` closes it, under [`KEY_CONTEXT`], whose
//! bindings are in [`crate::menu::bindings`]. Closing by key or by a chosen row
//! hands the focus back to whatever held it before the menu opened. A press
//! elsewhere closes it and leaves the focus to that press.

use crate::{
    menu::{self, Confirm, Cursor, Dismiss, Hit, Item, SelectNext, SelectPrevious},
    popover::{self, Popup},
};
use gpui::{AnyElement, Context, FocusHandle, Pixels, Point, SharedString, Window, prelude::*};
use std::rc::Rc;
use theme::Theme;

/// The key context an open menu claims.
pub const KEY_CONTEXT: &str = "ContextMenu";

/// What choosing a row runs, given the row's path into the items.
type OnChoose<V> = Rc<dyn Fn(&mut V, &[usize], &mut Window, &mut Context<V>)>;

/// Where an owning view keeps its menu.
pub type Slot<V> = fn(&mut V) -> &mut ContextMenu;

struct Open {
    at: Point<Pixels>,
    cursor: Cursor,
    items: Vec<Item>,
    focus: FocusHandle,
    back: Option<FocusHandle>,
}

/// A view's context menu: closed, or open with its rows.
#[derive(Default)]
pub struct ContextMenu(Popup<Open>);

impl ContextMenu {
    /// Open `items` at `at`, taking the keyboard.
    pub fn open<V: 'static>(
        &mut self,
        at: Point<Pixels>,
        items: Vec<Item>,
        window: &mut Window,
        cx: &mut Context<V>,
    ) {
        let back = window.focused(cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.0.open(Open {
            at,
            cursor: Cursor::default(),
            items,
            focus,
            back,
        });
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.0.is_open()
    }

    /// The menu, while it is open. `id` prefixes its rows' element ids and
    /// debug selectors — see [`menu::card`].
    pub fn render<V: 'static>(
        &self,
        id: impl Into<SharedString>,
        slot: Slot<V>,
        on_choose: impl Fn(&mut V, &[usize], &mut Window, &mut Context<V>) + 'static,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Option<AnyElement> {
        let open = self.0.get()?;
        let id = id.into();
        let on_choose: OnChoose<V> = Rc::new(on_choose);
        let theme = Theme::of(cx).clone();
        let choose = on_choose.clone();
        let card = menu::card(
            &theme,
            id.clone(),
            &open.items,
            &open.cursor,
            window,
            cx,
            move |view: &mut V, hit, window, cx| match hit {
                Hit::Point(path) => {
                    if let Some(open) = slot(view).0.open_mut()
                        && open.cursor.point_at(&open.items, &path)
                    {
                        cx.notify();
                    }
                }
                Hit::Choose(path) => {
                    close(view, slot, true, window, cx);
                    choose(view, &path, window, cx);
                }
                Hit::Dismiss => close(view, slot, false, window, cx),
            },
        )
        .track_focus(&open.focus)
        .key_context(KEY_CONTEXT)
        .on_action(cx.listener(move |view, _: &SelectNext, _, cx| step(view, slot, 1, cx)))
        .on_action(cx.listener(move |view, _: &SelectPrevious, _, cx| step(view, slot, -1, cx)))
        .on_action(cx.listener(move |view, _: &Confirm, window, cx| {
            let Some(path) = slot(view).0.get().and_then(|open| open.cursor.path()) else {
                return;
            };
            close(view, slot, true, window, cx);
            on_choose(view, &path, window, cx);
        }))
        .on_action(
            cx.listener(move |view, _: &Dismiss, window, cx| close(view, slot, true, window, cx)),
        );
        Some(popover::menu_at(
            id,
            open.at,
            card.into_any_element(),
            self.0.closing_since(),
        ))
    }
}

fn step<V: 'static>(view: &mut V, slot: Slot<V>, delta: isize, cx: &mut Context<V>) {
    if let Some(open) = slot(view).0.open_mut() {
        open.cursor.step(&open.items, delta);
        cx.notify();
    }
}

/// Close the menu, and when `refocus`, give the keyboard back to what held it
/// before.
fn close<V: 'static>(
    view: &mut V,
    slot: Slot<V>,
    refocus: bool,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    let back = slot(view).0.get().and_then(|open| open.back.clone());
    popover::close_popup(view, cx, move |view| &mut slot(view).0);
    if refocus && let Some(back) = back {
        window.focus(&back, cx);
    }
}
