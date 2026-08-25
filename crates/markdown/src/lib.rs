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
//! gpui, no painting — and [`render`] is the gpui layer over them, caret and
//! selection included for a caller that owns them. The editing *surface* is the
//! `editor` crate.
//!
//! An image at an `http` URL — a picture, a favicon, a bookmark's cover —
//! needs an http client on the app, which `gpui_platform::application` installs
//! and a hand-built [`gpui::Application`] does not. gpui's own default is a
//! `NullHttpClient`, and the failure is silent: the element paints the same
//! fallback it would show while a fetch was still in flight.

pub mod doc;
pub mod edit;
pub mod highlight;
pub mod parse;
pub mod preview;
pub mod render;
pub mod select;
pub mod serialize;

pub use doc::{Align, Block, BlockKind, Doc, Form, Mark, MarkSpan, Part, Text};
pub use edit::{Shortcut, shortcut};
pub use highlight::{Highlighter, languages, set_highlighter};
pub use parse::{is_image, is_url, parse};
pub use preview::{LinkPreview, Preview, set_link_preview};
pub use render::{BlockLayouts, Caption, markdown, render, render_with_selection};
pub use select::{Cursor, Selection};
pub use serialize::serialize;
