//! An infinite canvas for gpui, over [JSON Canvas](https://jsoncanvas.org).
//!
//! ```ignore
//! canvas::init(cx);                                               // once, at startup
//! canvas::set_kinds(cx, Kinds::new().with("session", SESSION));  // an app's own types
//! let view = cx.new(|cx| canvas::CanvasView::new(Canvas::parse(json)?, cx));
//! ```
//!
//! [`model`], [`mindmap`], [`layout`], [`change`] and [`drag`] are pure — no
//! gpui. What a node is comes from [`kind`]; where it sits from [`layout`];
//! every edit is a [`Change`] an app can refuse or rewrite
//! ([`CanvasView::with_changes`]). [`CanvasView`] is the surface: pan, zoom,
//! selection and the keys.

pub mod change;
pub mod drag;
pub mod kind;
pub mod layout;
pub mod mindmap;
pub mod model;
mod view;

pub use change::Change;
pub use kind::{Kind, Kinds, set_kinds, text_style};
pub use layout::Layout;
pub use model::Canvas;
pub use view::{CONTEXT, CanvasEvent, CanvasView, init, keys};
