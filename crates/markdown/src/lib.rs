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
pub mod app;
pub mod doc;
pub mod edit;
#[cfg(feature = "gpui")]
pub mod fence;
pub mod html;
pub mod marks;
#[cfg(feature = "gpui")]
pub mod paint;
pub mod parse;
pub mod quote;
#[cfg(feature = "gpui")]
pub mod render;
pub mod select;
pub mod serialize;
#[cfg(feature = "gpui")]
pub mod source;
pub mod synced;

#[cfg(feature = "gpui")]
pub use app::{
    AppExt,
    layout::Layout,
    link::{Link, Links},
    picture::PictureMenu,
    preview::Preview,
    typography::Typography,
};
pub use doc::{Align, Block, BlockKind, Doc, Form, Mark, MarkSpan, Part, QuoteKind, Quoted, Text};
pub use edit::{Shortcut, Splice, shortcut};
#[cfg(feature = "gpui")]
pub use fence::{BlockRenderer, Fence, Rewrite, highlight::Highlighter};
pub use html::{Html, html};
pub use marks::Marks;
#[cfg(feature = "gpui")]
pub use paint::{
    comment::{CommentPaint, CommentWashes, default_comment},
    find::{FindPaint, default_find, find_washes},
    marks::{
        HighlightColor, HighlightPaint, MarkPaint, default_highlight, highlight_solid,
        highlight_wash,
    },
};
pub use parse::{
    ParsedDoc, is_image, is_link, is_url, parse, parse_at, parse_at_many, parse_ranges,
    parse_ranges_with, parse_with,
};
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
pub use source::{spans as source_spans, style::SourceStyle};
pub use synced::Synced;
