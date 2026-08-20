//! A Notion-style block document model, with markdown as the wire form.
//!
//! ```
//! let doc = markdown::parse("# Title\n\n- a\n- b");
//! assert_eq!(doc.blocks.len(), 3);
//! assert_eq!(markdown::serialize(&doc), "# Title\n\n- a\n- b");
//! ```
//!
//! The model is a flat list of blocks with an indent level ([`Doc`]), not a
//! nested tree — Notion's shape rather than CommonMark's, chosen because
//! editing a flat list is list operations while editing a tree is restructuring.
//! [`parse`] and [`serialize`] are inverses up to a fixed point: parsing,
//! serializing and parsing again always lands on the same document, so an
//! edit/save cycle cannot drift.
//!
//! [`doc`], [`parse`], [`serialize`], [`select`] and [`edit`] are pure — no
//! gpui, no painting — and [`render`] is the gpui layer over them. The editing
//! *surface* — a focus handle, key bindings and the platform input handler — is
//! [`editor`], behind the `editor` feature so a read-only consumer compiles
//! none of it.

pub mod doc;
pub mod edit;
#[cfg(feature = "editor")]
pub mod editor;
pub mod highlight;
#[cfg(feature = "editor")]
pub mod history;
pub mod parse;
pub mod render;
pub mod select;
pub mod serialize;
#[cfg(feature = "editor")]
pub mod slash;

pub use doc::{Align, Block, BlockKind, Doc, Mark, MarkSpan, Part, Text};
pub use edit::{Shortcut, shortcut};
#[cfg(feature = "editor")]
pub use editor::Editor;
pub use highlight::{Highlighter, set_highlighter};
pub use parse::parse;
pub use render::{BlockLayouts, markdown, render, render_with_selection};
pub use select::{Cursor, Selection};
pub use serialize::serialize;
