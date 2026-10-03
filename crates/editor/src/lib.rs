//! A Notion-style block editor for gpui, over the `markdown` document model.
//!
//! ```ignore
//! editor::init(cx);                       // once, at startup
//! let editor = cx.new(|cx| editor::Editor::new("# Title", cx));
//! ```
//!
//! `markdown` holds the document, its markdown wire form, and the painting —
//! all of it testable without a window. What lives here is the half that needs
//! one: focus, keys, the platform input handler, the mouse, undo, and the menus.

mod app;
pub use app::AppExt;

mod anchor;
mod editor;
mod history;
mod layout;
mod link;
mod mention;
mod paste;
mod slash;
mod text_size;

pub use anchor::{Anchor, AnchorId};
#[doc(hidden)]
pub use editor::menu::{BLOCK_HANDLE, BLOCK_MENU, SLASH_MENU};
pub use editor::{
    CONTEXT, Chrome, Editor, EditorEvent, Formatting, HIGHLIGHT_MARK, Mode,
    image::{ImageStore, Source},
    init, keys, turns,
};
pub use history::{DEFAULT_UNDO_LIMIT, EditKind, History, Step};
pub use layout::Layout;
pub use mention::{Mention, MentionSource};
pub use paste::{PasteContent, PasteContext, PasteHandler};
pub use slash::{SlashAction, SlashAt, SlashItem, SlashRow, SlashRun};
pub use text_size::TextSize;
