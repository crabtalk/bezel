//! [`CommandPalette`] — a filtered command list over a [`input::TextField`].
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
//! ui::palette::init(cx);   // once, at startup (with input::init)
//! let palette = cx.new(|cx| CommandPalette::new(vec!["Open File".into()], cx));
//! cx.subscribe(&palette, |_, _, event, _| match event {
//!     PaletteEvent::Selected(index) => { /* run command `index` */ }
//!     PaletteEvent::Dismissed => { /* unmount */ }
//! })
//! .detach();
//! ```

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyBinding, SharedString, Window, actions,
    div, prelude::*, px,
};

use theme::Theme;

use crate::{input, popover, search::SearchList, surface::Surfaced as _};

actions!(
    bezel_command_palette,
    [SelectNext, SelectPrevious, Confirm, Dismiss]
);

/// The key context the palette claims. It wraps the field's own context, so
/// typing goes to the field while navigation keys fall through to here.
pub const KEY_CONTEXT: &str = "CommandPalette";

/// Install the bindings — [`bindings`], bound. Call once, alongside
/// [`crate::input::init`].
pub fn init(cx: &mut App) {
    cx.bind_keys(bindings());
}

/// The palette's navigation keymap, as data, so an app can have it without having to
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
        // The emacs pair, for the same reason the field honours ctrl-b/f.
        KeyBinding::new("ctrl-n", SelectNext, ctx),
        KeyBinding::new("ctrl-p", SelectPrevious, ctx),
    ]);

    bindings
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
    search: SearchList,
    focus_handle: FocusHandle,
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl CommandPalette {
    pub fn new(items: Vec<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            search: SearchList::new(
                items,
                "Type a command…",
                |view: &mut Self| &mut view.search,
                cx,
            ),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Focus the query field — call after mounting, or the palette swallows
    /// keys without showing a caret.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.search.query.focus_handle(cx), cx);
    }

    pub fn query_text(&self, cx: &App) -> SharedString {
        self.search.query.read(cx).content().clone()
    }

    /// The item the user would get by confirming right now.
    pub fn active_item(&self) -> Option<usize> {
        self.search.filter.active_item()
    }

    fn choose(&mut self, item: usize, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PaletteEvent::Selected(item));
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.search.filter.step(1);
        cx.notify();
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.search.filter.step(-1);
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
        let card = popover::popover_card(&theme)
            .w(px(420.0))
            .child(
                self.search
                    .body(&theme, None, |view| &mut view.search, Self::choose, cx),
            );

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
            .child(card.surface(&theme, theme.popover_surface))
    }
}

/// Re-exported so a host can bind its own "open palette" chord without
/// depending on gpui's action macros directly.
pub use input::KEY_CONTEXT as FIELD_KEY_CONTEXT;
