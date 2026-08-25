//! [`Menubar`] — the in-window bar: a strip of titles that drop menus.
//!
//! Not the *native* one. On macOS that is `cx.set_menus` and four lines in an
//! app's `main`, which is where it belongs; this is the bar an app with a custom
//! titlebar draws for itself, and the one every other platform expects to see
//! inside the window.
//!
//! An entity, on the line [`crate::date::Calendar`] drew: it owns which menu is
//! down and where the keyboard is inside it — state the app has no opinion
//! about — and reports the one thing the app wants, through [`MenubarEvent`].
//! The menus are data the app hands over, shaped like gpui's own `Menu` and
//! `MenuItem` so an app drawing both bars writes them the same way. It does not
//! *take* those types: they carry a boxed action, and reporting an index leaves
//! dispatch with the app, the way [`crate::combobox`] and [`crate::palette`]
//! already do.
//!
//! What makes it a menubar rather than a row of dropdowns is that one menu being
//! open changes what the others do: sliding the pointer onto a sibling title
//! switches to it with no click, and `left`/`right` cross between menus without
//! leaving the keyboard.
//!
//! ```ignore
//! ui::menubar::init(cx);   // once, at startup
//! let bar = cx.new(|cx| Menubar::new(vec![
//!     Menu::new("File", vec![
//!         Item::action("New Window").with_keystroke("⌘N"),
//!         Item::Separator,
//!         Item::action("Close").with_keystroke("⌘W").disabled(),
//!     ]),
//! ], cx));
//! cx.subscribe(&bar, |_, _, event, _| match event {
//!     MenubarEvent::Selected { menu, item } => { /* dispatch */ }
//! })
//! .detach();
//! ```

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyBinding, SharedString, Window, actions,
    div, prelude::*, px,
};

use theme::{Theme, ink};

use crate::popover;

/// One menu on the bar.
#[derive(Clone, Debug)]
pub struct Menu {
    pub title: SharedString,
    pub items: Vec<Item>,
}

impl Menu {
    pub fn new(title: impl Into<SharedString>, items: Vec<Item>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }
}

/// A row in a menu.
///
/// Deliberately not a struct with an `is_separator` flag: a separator has no
/// label, no accelerator and nothing to enable, and every one of those fields
/// would have to be answered anyway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Action {
        label: SharedString,
        /// The accelerator to *print* — the binding itself is the app's, and
        /// bezel never dispatches it. A menu that showed a keystroke it did not
        /// own would be documenting a lie.
        keystroke: Option<SharedString>,
        enabled: bool,
    },
    Separator,
}

impl Item {
    pub fn action(label: impl Into<SharedString>) -> Self {
        Item::Action {
            label: label.into(),
            keystroke: None,
            enabled: true,
        }
    }

    /// No-ops on a separator, which has nothing to hang a keystroke on.
    pub fn with_keystroke(self, keystroke: impl Into<SharedString>) -> Self {
        match self {
            Item::Action { label, enabled, .. } => Item::Action {
                label,
                keystroke: Some(keystroke.into()),
                enabled,
            },
            Item::Separator => Item::Separator,
        }
    }

    pub fn disabled(self) -> Self {
        match self {
            Item::Action {
                label, keystroke, ..
            } => Item::Action {
                label,
                keystroke,
                enabled: false,
            },
            Item::Separator => Item::Separator,
        }
    }

    /// Whether the keyboard and the pointer can land here at all.
    pub fn selectable(&self) -> bool {
        matches!(self, Item::Action { enabled: true, .. })
    }
}

/// The next row the keyboard can land on, `delta` deciding the direction:
/// separators and disabled rows are stepped straight over, and both ends wrap.
/// `from` of `None` enters the menu at the edge the direction comes from.
///
/// [`popover::menu_step`] cannot do this — it counts rows and knows nothing
/// about which of them can be landed on. `None` back means *nothing* in the menu
/// is selectable, which is the one shape that would otherwise spin forever.
pub fn next_selectable(items: &[Item], from: Option<usize>, delta: isize) -> Option<usize> {
    let count = items.len();
    if count == 0 {
        return None;
    }
    let step = if delta >= 0 { 1 } else { -1 };
    let wrap = |at: usize| (at as isize + step).rem_euclid(count as isize) as usize;
    // Entering, the first candidate is the edge itself; moving, it is the row
    // after the one you are on.
    let mut at = match from {
        None if step > 0 => 0,
        None => count - 1,
        Some(at) => wrap(at.min(count - 1)),
    };
    for _ in 0..count {
        if items[at].selectable() {
            return Some(at);
        }
        at = wrap(at);
    }
    None
}

// ---------------------------------------------------------------------------
// The bar
// ---------------------------------------------------------------------------

actions!(
    bezel_menubar,
    [PrevMenu, NextMenu, PrevItem, NextItem, Confirm, Dismiss]
);

/// The key context the bar claims, closed as well as open — `enter` on a
/// focused-but-closed bar drops its first menu.
pub const KEY_CONTEXT: &str = "Menubar";

/// Install the bar's bindings. Call once, alongside [`crate::input::init`].
///
/// `left`/`right` cross between menus and `up`/`down` walk the rows, which is
/// the one arrangement every platform's menubar agrees on. Nothing claims `alt`
/// to focus the bar: that is a Windows convention, and a component library that
/// binds a chord it is unsure of takes it away from every app downstream.
pub fn init(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("left", PrevMenu, ctx),
        KeyBinding::new("right", NextMenu, ctx),
        KeyBinding::new("up", PrevItem, ctx),
        KeyBinding::new("down", NextItem, ctx),
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
    ]);
}

/// What the bar reports: an item chosen, by its place in the menus it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenubarEvent {
    Selected { menu: usize, item: usize },
}

pub struct Menubar {
    menus: Vec<Menu>,
    /// Which title is down. One popup for the whole bar rather than one each:
    /// exactly one menu can be open, and saying so in the type is what makes
    /// switching between them a single assignment.
    open: popover::Popup<usize>,
    /// Where the keyboard is inside the open menu. Cleared whenever the menu
    /// changes, so a fresh menu opens with nothing highlighted rather than with
    /// the last one's row number pointing at whatever now sits there.
    highlighted: Option<usize>,
    focus_handle: FocusHandle,
}

impl EventEmitter<MenubarEvent> for Menubar {}

impl Menubar {
    pub fn new(menus: Vec<Menu>, cx: &mut Context<Self>) -> Self {
        Self {
            menus,
            open: popover::Popup::default(),
            highlighted: None,
            // One stop for the whole bar: the menus are keyboard-driven from
            // here, so no row takes focus of its own.
            focus_handle: cx.focus_handle().tab_stop(true),
        }
    }

    /// Which menu is down, `None` while none is (or while one is closing).
    pub fn open_menu(&self) -> Option<usize> {
        self.open.as_open().copied()
    }

    /// The menus as given. [`MenubarEvent`] reports a place in this list, so
    /// this is how a host turns one back into the item it named — without
    /// keeping a second copy that could drift from the bar's.
    pub fn menus(&self) -> &[Menu] {
        &self.menus
    }

    fn show(&mut self, menu: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.open.open(menu);
        self.highlighted = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn toggle(&mut self, menu: usize, window: &mut Window, cx: &mut Context<Self>) {
        // The note was taken on mouse-down and only counts for *this* title, so
        // pressing a different one switches menus instead of being swallowed by
        // the dismissal that same press caused.
        if self.open.take_press_was_open() {
            self.close(cx);
        } else {
            self.show(menu, window, cx);
        }
    }

    /// The rule that makes a bar a bar: with one menu already down, the pointer
    /// crossing a sibling title opens it. With none down, hovering does nothing
    /// — a menubar that dropped a menu at the mere passage of the mouse would be
    /// unusable.
    fn hover_switch(&mut self, menu: usize, cx: &mut Context<Self>) {
        if self.open.is_open() && self.open_menu() != Some(menu) {
            self.open.open(menu);
            self.highlighted = None;
            cx.notify();
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.open.begin_close() {
            popover::reap_popup(cx, |bar: &mut Self| &mut bar.open);
        }
        self.highlighted = None;
        cx.notify();
    }

    fn choose(&mut self, menu: usize, item: usize, cx: &mut Context<Self>) {
        cx.emit(MenubarEvent::Selected { menu, item });
        self.close(cx);
    }

    fn step_item(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(menu) = self.open_menu() else { return };
        self.highlighted = next_selectable(&self.menus[menu].items, self.highlighted, delta);
        cx.notify();
    }

    fn step_menu(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(menu) = self.open_menu() else { return };
        let count = self.menus.len() as isize;
        if count == 0 {
            return;
        }
        self.open
            .open((menu as isize + delta).rem_euclid(count) as usize);
        self.highlighted = None;
        cx.notify();
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        match (self.open_menu(), self.highlighted) {
            (Some(menu), Some(item)) => self.choose(menu, item, cx),
            // Closed, `enter` drops the first menu — the same key means "act on
            // this control" either way, which is what makes the bar reachable
            // by keyboard at all.
            (None, _) if !self.menus.is_empty() => self.show(0, window, cx),
            _ => {}
        }
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
    }

    fn card(&self, menu: usize, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        // Fade keys are a process-wide map, so they carry the entity id — an app
        // may hold more than one bar.
        let view = cx.entity_id();
        popover::popover_card(theme)
            .min_w(px(180.0))
            .children(
                self.menus[menu]
                    .items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| match item {
                        Item::Separator => popover::divider().into_any_element(),
                        Item::Action {
                            label,
                            keystroke,
                            enabled: false,
                        } => {
                            disabled_row(theme, label.clone(), keystroke.clone()).into_any_element()
                        }
                        Item::Action {
                            label, keystroke, ..
                        } => popover::menu_row_nav(
                            theme,
                            false,
                            self.highlighted == Some(index),
                            SharedString::from(format!("menubar-{view}-{menu}-{index}")),
                        )
                        .justify_between()
                        .id(SharedString::from(format!("item-{menu}-{index}")))
                        .on_click(cx.listener(move |bar, _, _, cx| bar.choose(menu, index, cx)))
                        .child(label.clone())
                        .when_some(keystroke.clone(), |row, keystroke| {
                            row.child(popover::kbd_hint(theme, &keystroke))
                        })
                        .into_any_element(),
                    }),
            )
            .on_mouse_down_out(cx.listener(|bar, _, _, cx| bar.close(cx)))
            .into_any_element()
    }
}

/// One title on the strip. Lit while its own menu is down.
pub fn menubar_title(theme: &Theme, label: impl Into<SharedString>, open: bool) -> gpui::Div {
    let title = div()
        .px(px(8.0))
        .py(px(3.0))
        .rounded(px(Theme::control_radius()))
        .text_size(px(13.0))
        .cursor_pointer()
        .child(label.into());
    if open {
        title.bg(ink(0.08)).text_color(theme.text)
    } else {
        // A plain hover style, not a `motion::hover_blend` fade key: the fade
        // installs an `on_hover` *listener*, and gpui allows only one per
        // element — the switch below needs it.
        title
            .text_color(theme.text_muted)
            .hover(|s| s.bg(ink(0.05)).text_color(theme.text))
    }
}

/// The strip the titles sit on.
pub fn menubar() -> gpui::Div {
    div().flex().flex_row().items_center().gap(px(2.0))
}

/// A row that cannot be chosen: [`popover::menu_row`]'s metrics without its
/// hover fade or its click, because a disabled row that lit under the pointer
/// would be inviting a press that does nothing.
fn disabled_row(theme: &Theme, label: SharedString, keystroke: Option<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(10.0))
        .px(px(8.0))
        .py(px(6.0))
        // The disabled twin of `popover::menu_row`, in the same card — so it
        // takes its corners from the same rule rather than a matching literal.
        .rounded(px(Theme::inset_radius(
            Theme::surface_radius(),
            popover::MENU_PAD,
        )))
        .text_size(px(13.0))
        .text_color(theme.text_faint)
        .child(label)
        .when_some(keystroke, |row, keystroke| {
            row.child(popover::kbd_hint(theme, &keystroke))
        })
}

impl Focusable for Menubar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Menubar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        // `get`, not `as_open`: the card stays mounted through the exit phase.
        let mounted = self.open.get().copied();
        let closing = self.open.closing_since();

        menubar()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|bar, _: &PrevMenu, _, cx| bar.step_menu(-1, cx)))
            .on_action(cx.listener(|bar, _: &NextMenu, _, cx| bar.step_menu(1, cx)))
            .on_action(cx.listener(|bar, _: &PrevItem, _, cx| bar.step_item(-1, cx)))
            .on_action(cx.listener(|bar, _: &NextItem, _, cx| bar.step_item(1, cx)))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .children((0..self.menus.len()).map(|menu| {
                let down = mounted == Some(menu);
                let card = down.then(|| self.card(menu, &theme, cx));
                div()
                    .relative()
                    .id(SharedString::from(format!("menubar-title-{menu}")))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |bar, _, _, _| {
                            bar.open.note_trigger_press_matching(|open| *open == menu)
                        }),
                    )
                    .on_click(cx.listener(move |bar, _, window, cx| bar.toggle(menu, window, cx)))
                    .on_hover(cx.listener(move |bar, hovered: &bool, _, cx| {
                        if *hovered {
                            bar.hover_switch(menu, cx);
                        }
                    }))
                    .child(menubar_title(&theme, self.menus[menu].title.clone(), down))
                    .when_some(card, |title, card| {
                        title.child(popover::anchored_menu_below(
                            SharedString::from(format!("menubar-menu-{menu}")),
                            card,
                            closing,
                        ))
                    })
            }))
    }
}
