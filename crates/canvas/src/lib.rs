//! An infinite canvas for gpui, over [JSON Canvas](https://jsoncanvas.org).
//!
//! ```ignore
//! canvas::init(cx);                                   // once, at startup
//! canvas::set_node_renderer(cx, my_nodes);            // an app's own kinds
//! let view = cx.new(|cx| canvas::CanvasView::new(Canvas::parse(json)?, cx));
//! ```
//!
//! [`model`], [`mindmap`] and [`drag`] are pure — no gpui. [`CanvasView`] is
//! the surface: pan, zoom, selection, and the mindmap keys over the same
//! document a save writes. What a dropped node does is the app's
//! ([`drag::DragHandler`]).

pub mod drag;
pub mod mindmap;
pub mod model;
mod node;
mod view;

pub use model::Canvas;
pub use node::{NodeRenderer, set_node_renderer};
pub use view::{Arrange, CONTEXT, CanvasEvent, CanvasView, init, keys};
