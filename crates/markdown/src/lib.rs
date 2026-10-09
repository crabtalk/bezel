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

mod app;
pub use app::AppExt;

pub mod block;
pub mod doc;
pub mod edit;
pub mod find;
pub mod highlight;
pub mod layout;
pub mod link;
pub mod marks;
#[cfg(feature = "mermaid")]
pub mod mermaid;
pub mod parse;
pub mod preview;
pub mod quote;
pub mod render;
pub mod select;
pub mod selectable;
pub mod serialize;
pub mod source;
pub mod source_style;
pub mod typography;

pub use block::{BlockRenderer, Fence, Rewrite};
pub use doc::{Align, Block, BlockKind, Doc, Form, Mark, MarkSpan, Part, QuoteKind, Quoted, Text};
pub use edit::{Shortcut, Splice, shortcut};
pub use find::{FindPaint, default_find};
pub use highlight::Highlighter;
pub use layout::Layout;
pub use link::LinkHandler;
pub use marks::{
    HighlightColor, HighlightPaint, MarkPaint, Marks, default_highlight, highlight_solid,
};
pub use parse::{
    ParsedDoc, is_image, is_link, is_url, parse, parse_at, parse_at_many, parse_ranges, parse_with,
};
pub use preview::{LinkCard, LinkPreview, Preview};
pub use quote::Quote;
pub use render::{
    Annotation, BlockLayouts, Caption, CopyButton, Editing, FenceHost, ImageOverlay,
    ImageOverlayCorner, LeaveBlock, OnImage, OnJump, OnLeave, OnResize, OnRewrite, OnToggle,
    PAINTED_CONTEXT, RevealMode, Toggle, image_source, markdown, render, render_source,
    render_with,
};
pub use select::{Affinity, Cursor, Selection};
pub use serialize::{serialize, serialize_at, serialize_at_many, serialize_with};
pub use source::spans as source_spans;
pub use source_style::SourceStyle;
pub use typography::Typography;
