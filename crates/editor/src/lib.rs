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

mod editor;
mod history;
mod link;
mod slash;

pub use editor::image::{ImageStore, Source, set_image_store};
#[doc(hidden)]
pub use editor::menu::SLASH_MENU;
pub use editor::{Editor, init};
pub use history::{DEFAULT_UNDO_LIMIT, EditKind, History};
