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
//! Everything that paints is behind the `gpui` feature, on by default. Without
//! it the crate is the document model alone and builds for
//! `wasm32-unknown-unknown`.
//!
//! An image at an `http` URL — a picture, a favicon, a bookmark's cover —
//! needs an http client on the app, which `gpui_platform::application` installs
//! and a hand-built [`gpui::Application`] does not. gpui's own default is a
//! `NullHttpClient`, and the failure is silent: the element paints the same
//! fallback it would show while a fetch was still in flight.

#[cfg(feature = "gpui")]
mod app;
#[cfg(feature = "gpui")]
pub use app::AppExt;

#[cfg(feature = "gpui")]
pub mod block;
#[cfg(feature = "mermaid")]
pub mod charts;
#[cfg(feature = "gpui")]
pub mod comment;
pub mod doc;
pub mod edit;
#[cfg(feature = "gpui")]
pub mod find;
#[cfg(feature = "gpui")]
pub mod highlight;
#[cfg(feature = "gpui")]
pub mod layout;
#[cfg(feature = "gpui")]
pub mod link;
pub mod marks;
#[cfg(feature = "mermaid")]
pub mod mermaid;
pub mod parse;
#[cfg(feature = "gpui")]
pub mod picture;
#[cfg(feature = "gpui")]
pub mod preview;
pub mod quote;
#[cfg(feature = "gpui")]
pub mod render;
pub mod select;
#[cfg(feature = "gpui")]
pub mod selectable;
pub mod serialize;
#[cfg(feature = "gpui")]
pub mod source;
#[cfg(feature = "gpui")]
pub mod source_style;
pub mod synced;
#[cfg(feature = "gpui")]
pub mod typography;

#[cfg(feature = "gpui")]
pub use block::{BlockRenderer, Fence, Rewrite};
#[cfg(feature = "gpui")]
pub use comment::{CommentPaint, CommentWashes, default_comment};
pub use doc::{Align, Block, BlockKind, Doc, Form, Mark, MarkSpan, Part, QuoteKind, Quoted, Text};
pub use edit::{Shortcut, Splice, shortcut};
#[cfg(feature = "gpui")]
pub use find::{FindPaint, default_find, find_washes};
#[cfg(feature = "gpui")]
pub use highlight::Highlighter;
#[cfg(feature = "gpui")]
pub use layout::Layout;
#[cfg(feature = "gpui")]
pub use link::{Link, Links};
pub use marks::Marks;
#[cfg(feature = "gpui")]
pub use marks::{
    HighlightColor, HighlightPaint, MarkPaint, default_highlight, highlight_solid, highlight_wash,
};
pub use parse::{
    ParsedDoc, is_image, is_link, is_url, parse, parse_at, parse_at_many, parse_ranges,
    parse_ranges_with, parse_with,
};
#[cfg(feature = "gpui")]
pub use picture::PictureMenu;
#[cfg(feature = "gpui")]
pub use preview::Preview;
pub use quote::Quote;
#[cfg(feature = "gpui")]
pub use render::{
    Annotation, BlockLayouts, Caption, CopyButton, Editing, FenceHost, ImageOverlay,
    ImageOverlayCorner, LeaveBlock, OnImage, OnImageMenu, OnJump, OnLeave, OnResize, OnRewrite,
    OnToggle, PAINTED_CONTEXT, RevealMode, SourceText, SpanJob, Spans, Toggle, image_source,
    markdown, render, render_source, render_with,
};
pub use select::{Affinity, Cursor, Selection};
pub use serialize::{serialize, serialize_at, serialize_at_many, serialize_with};
#[cfg(feature = "gpui")]
pub use source::spans as source_spans;
#[cfg(feature = "gpui")]
pub use source_style::SourceStyle;
pub use synced::Synced;
#[cfg(feature = "gpui")]
pub use typography::Typography;
