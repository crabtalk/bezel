//! An infinite canvas for gpui, over [JSON Canvas](https://jsoncanvas.org).
//!
//! ```ignore
//! canvas::init(cx);                                               // once, at startup
//! canvas::set_kinds(cx, Kinds::new().with("session", SESSION));  // an app's own types
//! let view = cx.new(|cx| canvas::CanvasView::new(Canvas::parse(json)?, cx));
//! ```
//!
//! [`model`], [`mindmap`], [`change`] and [`drag`] are pure — no gpui. What a
//! node is comes from [`kind`]; every edit is a [`Change`] an app can refuse or
//! rewrite ([`CanvasView::with_changes`]). [`CanvasView`] is the surface: pan,
//! zoom, selection and the mindmap keys.

pub mod change;
pub mod drag;
pub mod kind;
pub mod mindmap;
pub mod model;
mod view;

pub use change::Change;
pub use kind::{Kind, Kinds, set_kinds, text_style};
pub use model::Canvas;
pub use view::{Arrange, CONTEXT, CanvasEvent, CanvasView, init, keys};
