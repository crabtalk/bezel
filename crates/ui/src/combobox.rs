//! [`Combobox`] — a select you can type into: the closed face of a select over
//! an anchored menu whose rows narrow as you search.
//!
//! An entity for the same reason [`crate::palette::CommandPalette`] is one — it
//! owns a query [`TextField`]. The two share [`popover::Filter`] and differ only
//! in frame: the palette is a modal over every command, this hangs under a
//! trigger and remembers what was chosen.
//!
//! ```ignore
//! ui::combobox::init(cx);   // once, at startup (with input::init)
//! let language = cx.new(|cx| Combobox::new(vec!["Rust".into()], "Language", cx));
//! cx.subscribe(&language, |_, _, event, _| match event {
//!     ComboboxEvent::Selected(index) => { /* item `index` */ }
//! })
//! .detach();
//! ```

use crate::{
    input::{self, TextField},
    popover,
    widgets::Controls,
};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding, Pixels, SharedString,
    Window, actions, canvas, div, prelude::*, px,
};
use motion::{Fade, Painter};
use theme::Theme;

actions!(
    bezel_combobox,
    [SelectNext, SelectPrevious, Confirm, Dismiss]
);

/// The key context the combobox claims. It wraps the query field's own
/// context, so typing goes to the field while navigation keys fall through.
pub const KEY_CONTEXT: &str = "Combobox";

/// Install the combobox's navigation bindings. Call once, alongside
/// [`crate::input::init`].
pub fn init(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("up", SelectPrevious, ctx),
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
        KeyBinding::new("ctrl-n", SelectNext, ctx),
        KeyBinding::new("ctrl-p", SelectPrevious, ctx),
    ]);
}

/// What the combobox reports. The index is into the ORIGINAL item list, never
/// into the filtered view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboboxEvent {
    Selected(usize),
}

pub struct Combobox {
    query: Entity<TextField>,
    filter: popover::Filter,
    menu: popover::Popup<()>,
    chosen: Option<usize>,
    placeholder: SharedString,
    /// The trigger's laid-out width, measured last frame — the menu matches
    /// it. An anchored layer sizes to its own content, so without measuring,
    /// a combobox's menu could not line up with its face.
    trigger_width: Option<Pixels>,
    focus_handle: FocusHandle,
}

impl EventEmitter<ComboboxEvent> for Combobox {}

impl Combobox {
    pub fn new(
        items: Vec<SharedString>,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| TextField::new(cx).with_placeholder("Search…"));
        cx.observe(&query, |combobox, _, cx| {
            let query = combobox.query.read(cx).content().clone();
            combobox.filter.refilter(&query);
            cx.notify();
        })
        .detach();
        Self {
            query,
            filter: popover::Filter::new(items),
            menu: popover::Popup::default(),
            chosen: None,
            placeholder: placeholder.into(),
            trigger_width: None,
            // One stop per combobox: the query field is inside `menu_card`, so
            // it only joins the order while the menu is actually open.
            focus_handle: cx.focus_handle().tab_stop(true),
        }
    }

    /// Preselect an item — the value a form field starts with.
    pub fn with_selection(mut self, item: usize) -> Self {
        self.chosen = (item < self.filter.items().len()).then_some(item);
        self
    }

    pub fn selection(&self) -> Option<usize> {
        self.chosen
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The press note was taken on mouse-down: if the menu was mounted
        // then, this click is the dismissal, not a fresh open.
        if self.menu.take_press_was_open() {
            self.close(cx);
        } else {
            // A stale query would reopen the menu already narrowed.
            self.query.update(cx, |query, cx| query.clear(cx));
            self.filter.refilter("");
            self.menu.open(());
            window.focus(&self.query.focus_handle(cx), cx);
            cx.notify();
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.menu.begin_close() {
            popover::reap_popup(cx, |combobox: &mut Self| &mut combobox.menu);
        }
        cx.notify();
    }

    fn choose(&mut self, item: usize, cx: &mut Context<Self>) {
        self.chosen = Some(item);
        cx.emit(ComboboxEvent::Selected(item));
        self.close(cx);
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.filter.step(1);
        cx.notify();
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.filter.step(-1);
        cx.notify();
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = self.filter.active_item() {
            self.choose(item, cx);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
    }

    fn menu_card(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        // Element ids are namespaced by the view; hover-fade keys are a global
        // map, so those carry the entity id — a form can hold several of these.
        let view = Painter::of(cx);
        let rows: Vec<gpui::AnyElement> = self
            .filter
            .filtered()
            .iter()
            .enumerate()
            .map(|(position, &item)| {
                popover::menu_row_nav(
                    theme,
                    Some(item) == self.chosen,
                    Some(position) == self.filter.active(),
                    Fade::new(view, format!("combobox-row-{item}")),
                )
                .id(SharedString::from(format!("row-{item}")))
                .on_click(cx.listener(move |combobox, _, _, cx| combobox.choose(item, cx)))
                .child(self.filter.items()[item].clone())
                .into_any_element()
            })
            .collect();

        popover::popover_card(theme)
            .w(self.trigger_width.unwrap_or(px(200.0)))
            .on_mouse_down_out(cx.listener(|combobox, _, _, cx| combobox.close(cx)))
            .child(popover::search_input_frame(
                theme,
                self.query.clone().into_any_element(),
            ))
            .child(if rows.is_empty() {
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_size(px(13.0))
                    .text_color(theme.text_muted)
                    .child("No matches")
                    .into_any_element()
            } else {
                div().flex().flex_col().children(rows).into_any_element()
            })
            .into_any_element()
    }
}

impl Focusable for Combobox {
    /// The query field holds focus while open; this is the context around it.
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Combobox {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let open = self.menu.is_open() || self.menu.is_closing();
        let label = match self.chosen {
            Some(item) => self.filter.items()[item].clone(),
            None => self.placeholder.clone(),
        };
        let card = open.then(|| self.menu_card(&theme, cx));
        let combobox = cx.entity().downgrade();

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .relative()
            .w_full()
            // Records the trigger width for next frame's menu; the trigger is
            // always on screen before the menu opens, so it is never unset
            // when it matters.
            .child(
                canvas(
                    move |bounds, _, cx| {
                        combobox
                            .update(cx, |combobox, _| {
                                combobox.trigger_width = Some(bounds.size.width);
                            })
                            .ok();
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .id("combobox-trigger")
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|combobox, _, _, _| combobox.menu.note_trigger_press()),
                    )
                    .on_click(cx.listener(|combobox, _, window, cx| combobox.toggle(window, cx)))
                    .child(theme.select_trigger(label, open)),
            )
            .when_some(card, |trigger, card| {
                trigger.child(popover::anchored_menu_below(
                    "combobox-menu",
                    card,
                    self.menu.closing_since(),
                ))
            })
    }
}

/// Re-exported so a host can wire the field's context without depending on
/// [`crate::input`] directly.
pub use input::KEY_CONTEXT as FIELD_KEY_CONTEXT;
