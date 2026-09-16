//! An infinite canvas for gpui, over [JSON Canvas](https://jsoncanvas.org).
//!
//! ```ignore
//! canvas::init(cx);                                                // once, at startup
//! let view = cx.new(|cx| {
//!     CanvasView::new(Canvas::parse(json)?, cx)
//!         .with_kinds(Kinds::new().with("session", SESSION))       // an app's own types
//! });
//! ```
//!
//! [`model`], [`mindmap`], [`layout`], [`change`], [`clip`] and [`drag`] are
//! pure — no gpui. What a node is comes from [`kind`]; where it sits from [`layout`];
//! every edit is a [`Change`] an app can refuse or rewrite
//! ([`CanvasEditor::with_changes`]). [`CanvasEditor`] is the canvas without a
//! window — document, selection, history, the part in view — and
//! [`CanvasView`] paints it and turns keys and the pointer into its commands.

pub mod change;
pub mod clip;
pub mod contain;
pub mod drag;
mod edit;
pub mod kind;
pub mod layout;
pub mod mindmap;
mod minimap;
pub mod model;
pub mod snap;
pub mod tool;
mod view;

pub use change::Change;
pub use edit::{CanvasEditor, CanvasEvent, Item};
pub use kind::{Kind, Kinds, set_kinds, text_style};
pub use layout::Layout;
pub use minimap::minimap;
pub use model::Canvas;
pub use snap::Snap;
pub use tool::Tool;
pub use view::{CONTEXT, CanvasView, init, keys};
