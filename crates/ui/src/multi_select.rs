//! [`MultiSelect`] — Notion's multi-select picker: names chosen from a shared
//! set, several at once, and the set managed in place.
//!
//! The picker is the popover's body, not its trigger: it renders its own
//! [`popover::popover_card`] and the host mounts it through one of
//! [`popover`]'s anchored layers, so one picker serves any trigger — a table
//! cell, a column heading, a toolbar button.
//!
//! The host owns the data. Every event is a request, and the picker paints
//! what [`MultiSelect::set_choices`] last handed it.
//!
//! ```ignore
//! ui::multi_select::init(cx);   // once, at startup (with input::init)
//! let labels = cx.new(|cx| {
//!     MultiSelect::new(choices, cx).with_create(normalize).with_manage()
//! });
//! cx.subscribe(&labels, |_, _, event, _| match event {
//!     MultiSelectEvent::Toggled { name, on } => { /* apply or remove */ }
//!     MultiSelectEvent::Dismissed => { /* close the popover */ }
//!     _ => {}
//! })
//! .detach();
//! ```

use std::{cell::Cell, rc::Rc};

use gpui::{
    App, Axis, Context, Entity, EventEmitter, FocusHandle, Focusable, Hsla, KeyBinding, Pixels,
    Point, ScrollHandle, SharedString, Window, actions, div, prelude::*, px,
};
use theme::{TextStyle, Theme, Typeset};

use crate::{
    icons,
    input::{self, FieldEvent, TextField},
    popover, scroll, search,
    widgets::{ButtonStyle, Buttons, Content},
};

actions!(
    bezel_multi_select,
    [SelectNext, SelectPrevious, Confirm, Dismiss]
);

/// The key context the picker claims. It wraps its fields' own context, so
/// typing goes to a field while navigation keys fall through.
pub const KEY_CONTEXT: &str = "MultiSelect";

/// The card's width until [`MultiSelect::with_width`] says otherwise.
const WIDTH: f32 = 280.0;

/// Install the bindings — [`bindings`], bound. Call once, alongside
/// [`crate::input::init`].
pub fn init(cx: &mut App) {
    cx.bind_keys(bindings());
}

/// The picker's navigation keymap, as data — see [`crate::keys`] for layering
/// over it or taking a chord away.
pub fn bindings() -> Vec<KeyBinding> {
    let ctx = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("up", SelectPrevious, ctx),
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
        KeyBinding::new("ctrl-n", SelectNext, ctx),
        KeyBinding::new("ctrl-p", SelectPrevious, ctx),
    ]
}

/// A colour for `name`, the same for the same name on every machine: an
/// FNV-1a hash of its bytes into [`Theme::categorical`].
pub fn tint(theme: &Theme, name: &str) -> Hsla {
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    theme.categorical(hash as usize)
}

/// Whether a choice is on, for everything the picker is choosing for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Off,
    On,
    /// On for some and off for others — a picker over several entries.
    Mixed,
}

/// One name the picker offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub name: SharedString,
    pub check: Check,
    /// Painted after the name when present — how many entries carry it.
    pub count: Option<usize>,
}

/// What the picker asks of its host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MultiSelectEvent {
    /// `name` turned on or off. A mixed choice turns on.
    Toggled { name: SharedString, on: bool },
    /// A name no choice has, already normalised by
    /// [`MultiSelect::with_create`]'s function, to be created and turned on.
    Created(SharedString),
    /// `from` renamed to `to`, normalised. `to` may already be a choice.
    Renamed {
        from: SharedString,
        to: SharedString,
    },
    /// `name` deleted. Not confirmed by the picker.
    Deleted(SharedString),
    /// Escape, or a press outside the card.
    Dismissed,
}

/// The rows the list pane shows, in the order they paint.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Row {
    Create(SharedString),
    /// An index into the choices.
    Choice(usize),
}

/// The pane `···` opens: one choice's name, editable, and its Delete.
struct Managing {
    name: SharedString,
    field: Entity<TextField>,
}

type Normalize = dyn Fn(&str) -> Option<SharedString>;
type Tint = dyn Fn(&str, &Theme) -> Hsla;

pub struct MultiSelect {
    choices: Vec<Choice>,
    query: Entity<TextField>,
    text: SharedString,
    rows: Vec<Row>,
    /// Position in `rows`.
    active: Option<usize>,
    create: Option<Rc<Normalize>>,
    manage: bool,
    tint: Rc<Tint>,
    /// The card's narrowest and widest.
    width: (Pixels, Pixels),
    managing: Option<Managing>,
    /// The query takes focus at the next paint: the pane holding it closed
    /// where no window was to hand.
    refocus: bool,
    scroll: ScrollHandle,
    /// The row the list was last scrolled to — see [`search::SearchList`].
    scrolled: Cell<Option<usize>>,
    focus_handle: FocusHandle,
}

impl EventEmitter<MultiSelectEvent> for MultiSelect {}

impl MultiSelect {
    pub fn new(choices: Vec<Choice>, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| search::query_field("Search or create…", cx));
        cx.subscribe(&query, |view, query, event: &FieldEvent, cx| {
            if matches!(event, FieldEvent::Changed(_)) {
                let text = query.read(cx).content().clone();
                if view.text != text {
                    view.text = text;
                    view.refilter();
                    cx.notify();
                }
            }
        })
        .detach();
        let mut view = Self {
            choices,
            query,
            text: "".into(),
            rows: Vec::new(),
            active: None,
            create: None,
            manage: false,
            tint: Rc::new(|name, theme| tint(theme, name)),
            width: (px(WIDTH), px(WIDTH)),
            managing: None,
            refocus: false,
            scroll: ScrollHandle::new(),
            scrolled: Cell::new(None),
            focus_handle: cx.focus_handle(),
        };
        view.refilter();
        view
    }

    /// Offer a "Create" row for a query that names no choice. `normalize`
    /// turns the query into the name it would create, or `None` for one that
    /// cannot be a name; the row shows, and [`MultiSelectEvent::Created`]
    /// carries, what it answers. Choices are matched against it as given.
    pub fn with_create(
        mut self,
        normalize: impl Fn(&str) -> Option<SharedString> + 'static,
    ) -> Self {
        self.create = Some(Rc::new(normalize));
        self.refilter();
        self
    }

    /// Give each row a `···` opening a pane to rename or delete its choice.
    pub fn with_manage(mut self) -> Self {
        self.manage = true;
        self
    }

    /// Colour each name with `tint` instead of [`tint`].
    pub fn with_tint(mut self, tint: impl Fn(&str, &Theme) -> Hsla + 'static) -> Self {
        self.tint = Rc::new(tint);
        self
    }

    /// Size the card to its widest row, from `min` up to `max`. Equal bounds
    /// fix the width. A name wider than `max` is cut at the card's edge.
    pub fn with_width(mut self, min: Pixels, max: Pixels) -> Self {
        self.width = (min, max);
        self
    }

    pub fn choices(&self) -> &[Choice] {
        &self.choices
    }

    /// Replace the choices — the host's answer to an event. The query and the
    /// open pane stay; a pane whose choice is gone closes.
    pub fn set_choices(&mut self, choices: Vec<Choice>, cx: &mut Context<Self>) {
        self.choices = choices;
        if let Some(managing) = &self.managing
            && !self
                .choices
                .iter()
                .any(|choice| choice.name == managing.name)
        {
            self.managing = None;
            self.refocus = true;
        }
        self.refilter();
        cx.notify();
    }

    /// Start a fresh pick: an empty query, the list pane, and focus in the
    /// query. Call it as the popover opens.
    pub fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.managing = None;
        self.clear_query(cx);
        window.focus(&self.query.focus_handle(cx), cx);
        cx.notify();
    }

    fn clear_query(&mut self, cx: &mut Context<Self>) {
        self.query.update(cx, |query, cx| query.clear(cx));
        self.text = "".into();
        self.refilter();
    }

    fn refilter(&mut self) {
        let names: Vec<&str> = self
            .choices
            .iter()
            .map(|choice| choice.name.as_ref())
            .collect();
        let created = self
            .create
            .as_ref()
            .and_then(|normalize| normalize(&self.text))
            .filter(|name| !self.choices.iter().any(|choice| choice.name == *name));
        self.rows = created
            .map(Row::Create)
            .into_iter()
            .chain(
                popover::filter_indices(&self.text, &names)
                    .into_iter()
                    .map(Row::Choice),
            )
            .collect();
        self.active = (!self.rows.is_empty()).then_some(0);
        self.scroll.set_offset(Point::default());
        self.scrolled.set(None);
    }

    fn toggle(&mut self, choice: usize, cx: &mut Context<Self>) {
        let Some(choice) = self.choices.get(choice) else {
            return;
        };
        cx.emit(MultiSelectEvent::Toggled {
            name: choice.name.clone(),
            on: choice.check != Check::On,
        });
    }

    fn take(&mut self, row: usize, cx: &mut Context<Self>) {
        match self.rows.get(row).cloned() {
            Some(Row::Create(name)) => cx.emit(MultiSelectEvent::Created(name)),
            Some(Row::Choice(choice)) => self.toggle(choice, cx),
            None => return,
        }
        self.clear_query(cx);
        cx.notify();
    }

    fn manage_choice(&mut self, choice: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.choices.get(choice).map(|choice| choice.name.clone()) else {
            return;
        };
        let field = cx.new(|cx| {
            let mut field = TextField::new(cx);
            field.set_content(name.clone(), cx);
            field.select(0..name.len(), cx);
            field
        });
        window.focus(&field.focus_handle(cx), cx);
        self.managing = Some(Managing { name, field });
        cx.notify();
    }

    fn leave_managing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.managing = None;
        window.focus(&self.query.focus_handle(cx), cx);
        cx.notify();
    }

    fn rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(managing) = &self.managing else {
            return;
        };
        let text = managing.field.read(cx).content().clone();
        let to = match &self.create {
            Some(normalize) => normalize(&text),
            None => Some(SharedString::from(text.trim().to_owned())).filter(|to| !to.is_empty()),
        };
        if let Some(to) = to.filter(|to| *to != managing.name) {
            cx.emit(MultiSelectEvent::Renamed {
                from: managing.name.clone(),
                to,
            });
        }
        self.leave_managing(window, cx);
    }

    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(managing) = &self.managing {
            cx.emit(MultiSelectEvent::Deleted(managing.name.clone()));
        }
        self.leave_managing(window, cx);
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if self.managing.is_none() {
            self.active = popover::menu_step(self.active, self.rows.len(), 1);
            cx.notify();
        }
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        if self.managing.is_none() {
            self.active = popover::menu_step(self.active, self.rows.len(), -1);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if self.managing.is_some() {
            self.rename(window, cx);
        } else if let Some(active) = self.active {
            self.take(active, cx);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.managing.is_some() {
            self.leave_managing(window, cx);
        } else {
            cx.emit(MultiSelectEvent::Dismissed);
        }
    }

    /// Backspace in an empty query turns off the last choice shown in it.
    /// Taken in the capture phase, before the field sees it.
    fn backspace(&mut self, _: &input::Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.managing.is_some() || !self.text.is_empty() {
            return;
        }
        if let Some(last) = self
            .choices
            .iter()
            .rposition(|choice| choice.check == Check::On)
        {
            self.toggle(last, cx);
            cx.stop_propagation();
        }
    }

    /// The query line: every choice that is on as a chip with a ×, then the
    /// field.
    fn query_line(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let chips = self
            .choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.check == Check::On)
            .map(|(index, choice)| {
                theme
                    .chip(choice.name.clone(), (self.tint)(&choice.name, theme))
                    .max_w_full()
                    .child(
                        div()
                            .id(("multi-select-remove", index))
                            .flex_none()
                            .cursor_pointer()
                            .on_click(cx.listener(move |view, _, _, cx| view.toggle(index, cx)))
                            .child(
                                icons::icon(icons::glyph::X)
                                    .size(px(10.0))
                                    .text_color(theme.text_muted),
                            ),
                    )
            });
        popover::search_line(
            theme,
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
                .children(chips)
                .child(div().flex_1().min_w(px(80.0)).child(self.query.clone()))
                .into_any_element(),
        )
    }

    fn list(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.rows.is_empty() {
            return div()
                .px(px(popover::MENU_ROW_INSET))
                .py(px(8.0))
                .text_style(TextStyle::Body)
                .text_color(theme.text_muted)
                .child("No matches")
                .into_any_element();
        }
        let rows = self.rows.iter().enumerate().map(|(position, row)| {
            let active = Some(position) == self.active;
            let base = popover::menu_row(theme, active, None).on_mouse_move(cx.listener(
                move |view, _, _, cx| {
                    if view.active != Some(position) {
                        view.active = Some(position);
                        cx.notify();
                    }
                },
            ));
            match row {
                Row::Create(name) => base
                    .id("multi-select-create")
                    .on_click(cx.listener(move |view, _, _, cx| view.take(position, cx)))
                    .child(
                        icons::icon(icons::glyph::Plus)
                            .size(px(13.0))
                            .text_color(theme.text_muted),
                    )
                    .child("Create")
                    .child(
                        theme
                            .chip(name.clone(), (self.tint)(name, theme))
                            .max_w_full(),
                    )
                    .into_any_element(),
                Row::Choice(index) => {
                    let choice = &self.choices[*index];
                    let index = *index;
                    let mark = match choice.check {
                        Check::On => Some(icons::glyph::Check),
                        Check::Mixed => Some(icons::glyph::Minus),
                        Check::Off => None,
                    };
                    base.id(("multi-select-row", index))
                        .on_click(cx.listener(move |view, _, _, cx| view.take(position, cx)))
                        .child(
                            div().flex_1().min_w_0().flex().child(
                                theme
                                    .chip(choice.name.clone(), (self.tint)(&choice.name, theme))
                                    .flex_shrink_1()
                                    .min_w_0(),
                            ),
                        )
                        .when_some(choice.count, |row, count| {
                            row.child(
                                div()
                                    .flex_none()
                                    .text_style(TextStyle::Callout)
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(count.to_string())),
                            )
                        })
                        .child(
                            div()
                                .flex_none()
                                .size(px(13.0))
                                .when_some(mark, |slot, mark| {
                                    slot.child(
                                        icons::icon(mark).size(px(13.0)).text_color(theme.text),
                                    )
                                }),
                        )
                        .when(self.manage, |row| {
                            row.child(
                                theme
                                    .icon_button(icons::glyph::Ellipsis, ButtonStyle::Ghost, None)
                                    .id(("multi-select-manage", index))
                                    .when(!active, |button| button.invisible())
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        cx.stop_propagation();
                                        view.manage_choice(index, window, cx);
                                    })),
                            )
                        })
                        .into_any_element()
                }
            }
        });
        // Following the active row, as the search list does.
        if self.scrolled.get() != self.active {
            self.scrolled.set(self.active);
            if let Some(active) = self.active {
                self.scroll.scroll_to_item(active);
            }
        }
        scroll::Viewport::new(
            "multi-select-rows",
            div()
                .id("multi-select-rows-inner")
                .max_h(px(search::MAX_ROWS * popover::menu_row_height()))
                .flex()
                .flex_col()
                .children(rows),
            Axis::Vertical,
        )
        .track_scroll(&self.scroll)
        .into_any_element()
    }

    fn managing_pane(
        &self,
        managing: &Managing,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .p(px(popover::MENU_PAD))
                    .child(
                        theme
                            .icon_button(icons::glyph::ChevronLeft, ButtonStyle::Ghost, None)
                            .id("multi-select-back")
                            .on_click(
                                cx.listener(|view, _, window, cx| view.leave_managing(window, cx)),
                            ),
                    )
                    .child(div().flex_1().min_w_0().child(managing.field.clone())),
            )
            .child(popover::divider(theme))
            .child(
                popover::menu_row(theme, false, None)
                    .id("multi-select-delete")
                    .text_color(theme.danger)
                    .on_click(cx.listener(|view, _, window, cx| view.delete(window, cx)))
                    .child("Delete"),
            )
    }
}

impl Focusable for MultiSelect {
    /// The context around the fields; one of them holds focus while it is
    /// open — see [`MultiSelect::reset`].
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MultiSelect {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.refocus) {
            window.focus(&self.query.focus_handle(cx), cx);
        }
        let theme = Theme::of(cx).clone();
        let body = match &self.managing {
            Some(managing) => self.managing_pane(managing, &theme, cx).into_any_element(),
            None => div()
                .flex()
                .flex_col()
                .child(self.query_line(&theme, cx))
                .child(self.list(&theme, cx))
                .into_any_element(),
        };
        popover::popover_card(&theme)
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .min_w(self.width.0)
            .max_w(self.width.1)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .capture_action(cx.listener(Self::backspace))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| {
                cx.emit(MultiSelectEvent::Dismissed);
                cx.stop_propagation();
            }))
            .child(body)
    }
}
