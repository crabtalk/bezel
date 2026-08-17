//! A Notion-style block document model, with markdown as the wire form.
//!
//! ```
//! let doc = bezel_markdown::parse("# Title\n\n- a\n- b");
//! assert_eq!(doc.blocks.len(), 3);
//! assert_eq!(bezel_markdown::serialize(&doc), "# Title\n\n- a\n- b");
//! ```
//!
//! The model is a flat list of blocks with an indent level ([`Doc`]), not a
//! nested tree — Notion's shape rather than CommonMark's, chosen because
//! editing a flat list is list operations while editing a tree is restructuring.
//! [`parse`] and [`serialize`] are inverses up to a fixed point: parsing,
//! serializing and parsing again always lands on the same document, so an
//! edit/save cycle cannot drift.
//!
//! [`doc`], [`parse`], [`serialize`] and [`edit`] are pure — no gpui, no painting — and
//! [`render`] is the gpui layer over them. The editing *surface* — a focus
//! handle, key bindings and the platform input handler — is [`editor`], behind
//! the `editor` feature so a read-only consumer compiles none of it.

pub mod doc;
pub mod edit;
#[cfg(feature = "editor")]
pub mod editor;
pub mod parse;
pub mod render;
pub mod serialize;

pub use doc::{Align, Block, BlockKind, Doc, Mark, MarkSpan, Text};
pub use edit::{Cursor, Shortcut, shortcut};
#[cfg(feature = "editor")]
pub use editor::Editor;
pub use parse::parse;
pub use render::{BlockLayouts, markdown, render, render_with_caret};
pub use serialize::serialize;
