//! [`CommandPalette`] — a filtered command list over a [`TextField`].
//!
//! Stateful for the same reason the text field is: it owns a query, a filtered
//! view of the items and an active row. It reports outcomes as gpui events
//! rather than taking a callback, so the host decides what a selection *means*
//! and the palette never knows about the app's actions.
//!
//! The state underneath is [`popover::Filter`], shared with
//! [`crate::combobox::Combobox`] and tested there.
//!
//! ```ignore
//! bezel_ui::palette::init(cx);   // once, at startup (with input::init)
//! let palette = cx.new(|cx| CommandPalette::new(vec!["Open File".into()], cx));
//! cx.subscribe(&palette, |_, _, event, _| match event {
//!     PaletteEvent::Selected(index) => { /* run command `index` */ }
//!     PaletteEvent::Dismissed => { /* unmount */ }
//! })
//! .detach();
//! ```

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding, SharedString, Window,
    actions, div, prelude::*, px,
};

use bezel_theme::Theme;

use crate::input::{self, TextField};
use crate::popover;

actions!(
    bezel_command_palette,
    [SelectNext, SelectPrevious, Confirm, Dismiss]
);

/// The key context the palette claims. It wraps the field's own context, so
/// typing goes to the field while navigation keys fall through to here.
pub const KEY_CONTEXT: &str = "CommandPalette";

/// Install the palette's navigation bindings. Call once, alongside
/// [`crate::input::init`].
pub fn init(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("up", SelectPrevious, ctx),
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
        // The emacs pair, for the same reason the field honours ctrl-b/f.
        KeyBinding::new("ctrl-n", SelectNext, ctx),
        KeyBinding::new("ctrl-p", SelectPrevious, ctx),
    ]);
}

/// What the palette reports. Indices are into the ORIGINAL item list, never
/// into the filtered view — a caller matching on a filtered index would run
/// the wrong command the moment a query is typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteEvent {
    Selected(usize),
    Dismissed,
}

pub struct CommandPalette {
    query: Entity<TextField>,
    filter: popover::Filter,
    focus_handle: FocusHandle,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl CommandPalette {
    pub fn new(items: Vec<SharedString>, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| TextField::new(cx).with_placeholder("Type a command…"));
        // Re-filter whenever the field's content changes.
        cx.observe(&query, |palette, _, cx| {
            palette.refilter(cx);
        })
        .detach();
        Self {
            query,
            filter: popover::Filter::new(items),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Focus the query field — call after mounting, or the palette swallows
    /// keys without showing a caret.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.query.focus_handle(cx), cx);
    }

    pub fn query_text(&self, cx: &App) -> SharedString {
        self.query.read(cx).content().clone()
    }

    /// The item the user would get by confirming right now.
    pub fn active_item(&self) -> Option<usize> {
        self.filter.active_item()
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let query = self.query.read(cx).content().clone();
        self.filter.refilter(&query);
        cx.notify();
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
        if let Some(item) = self.active_item() {
            cx.emit(PaletteEvent::Selected(item));
        }
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PaletteEvent::Dismissed);
    }
}

impl Focusable for CommandPalette {
    /// The field holds focus; the palette is the context around it.
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let rows: Vec<gpui::AnyElement> = self
            .filter
            .filtered()
            .iter()
            .enumerate()
            .map(|(position, &item)| {
                popover::menu_row(
                    &theme,
                    Some(position) == self.filter.active(),
                    SharedString::from(format!("palette-row-{item}")),
                )
                .id(SharedString::from(format!("palette-{item}")))
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(PaletteEvent::Selected(item));
                }))
                .child(self.filter.items()[item].clone())
                .into_any_element()
            })
            .collect();

        let card = popover::popover_card(&theme)
            .w(px(420.0))
            .child(popover::search_input_frame(
                &theme,
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
            });

        // The actions live on a wrapper, not the card, because the card is
        // handed to `material` — which frosts the backdrop so the content
        // behind the palette blurs instead of reading through it.
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .child(crate::material::material(
                12.0,
                crate::material::MENU_BLUR,
                card,
            ))
    }
}

/// Re-exported so a host can bind its own "open palette" chord without
/// depending on gpui's action macros directly.
pub use input::KEY_CONTEXT as FIELD_KEY_CONTEXT;
