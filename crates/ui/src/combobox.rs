//! [`Combobox`] — a select you can type into: the closed face of a select over
//! an anchored menu whose rows narrow as you search.
//!
//! An entity for the same reason [`crate::palette::CommandPalette`] is one — it
//! owns a query [`input::TextField`]. The two share [`popover::Filter`] and differ only
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

use crate::{input, popover, search::SearchList, widgets::Controls};
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyBinding, Pixels, SharedString, Window,
    actions, canvas, div, prelude::*, px,
};
use theme::Theme;

actions!(
    bezel_combobox,
    [SelectNext, SelectPrevious, Confirm, Dismiss]
);

/// The key context the combobox claims. It wraps the query field's own
/// context, so typing goes to the field while navigation keys fall through.
pub const KEY_CONTEXT: &str = "Combobox";

/// Install the bindings — [`bindings`], bound. Call once, alongside
/// [`crate::input::init`].
pub fn init(cx: &mut App) {
    cx.bind_keys(bindings());
}

/// The combobox's navigation keymap, as data, so an app can have it without having to
/// take it — see [`crate::keys`] for layering over it or taking a chord
/// away.
pub fn bindings() -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    let ctx = Some(KEY_CONTEXT);
    bindings.extend([
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("up", SelectPrevious, ctx),
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
        KeyBinding::new("ctrl-n", SelectNext, ctx),
        KeyBinding::new("ctrl-p", SelectPrevious, ctx),
    ]);

    bindings
}

/// What the combobox reports. The index is into the ORIGINAL item list, never
/// into the filtered view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboboxEvent {
    Selected(usize),
}

pub struct Combobox {
    search: SearchList,
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
        Self {
            search: SearchList::new(items, "Search…", |view: &mut Self| &mut view.search, cx),
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
        self.chosen = (item < self.search.filter.items().len()).then_some(item);
        self
    }

    pub fn selection(&self) -> Option<usize> {
        self.chosen
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.clear(cx);
        if let Some(chosen) = self.chosen {
            self.search.filter.set_active(chosen);
        }
        self.menu.open(());
        window.focus(&self.search.query.focus_handle(cx), cx);
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take_press_was_open() {
            self.close(window, cx);
        } else {
            self.open(window, cx);
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Restore only focus owned by the query: an outside click may already
        // have focused another control.
        if self.search.query.focus_handle(cx).is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
        popover::close_popup(self, cx, |view| &mut view.menu);
    }

    fn choose(&mut self, item: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.menu.is_open() {
            return;
        }
        self.chosen = Some(item);
        cx.emit(ComboboxEvent::Selected(item));
        self.close(window, cx);
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.is_open() {
            self.search.filter.step(delta);
            cx.notify();
        } else if !self.menu.is_closing() {
            self.open(window, cx);
        }
    }

    fn select_next(&mut self, _: &SelectNext, window: &mut Window, cx: &mut Context<Self>) {
        self.step(1, window, cx);
    }

    fn select_previous(&mut self, _: &SelectPrevious, window: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, window, cx);
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.is_open() {
            if let Some(item) = self.search.filter.active_item() {
                self.choose(item, window, cx);
            }
        } else if !self.menu.is_closing() {
            self.open(window, cx);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.is_open() {
            self.close(window, cx);
        } else {
            cx.propagate();
        }
    }

    fn menu_card(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        popover::popover_card(theme)
            .w(self.trigger_width.unwrap_or(px(200.0)))
            .on_mouse_down_out(cx.listener(|view, _, window, cx| view.close(window, cx)))
            .child(self.search.body(
                theme,
                self.chosen,
                |view| &mut view.search,
                Self::choose,
                cx,
            ))
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
            Some(item) => self.search.filter.items()[item].clone(),
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
            .child(popover::trigger_press(
                div()
                    .id("combobox-trigger")
                    .on_click(cx.listener(|combobox, _, window, cx| combobox.toggle(window, cx)))
                    .child(theme.select_trigger(label)),
                |combobox: &mut Self| &mut combobox.menu,
                cx,
            ))
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
