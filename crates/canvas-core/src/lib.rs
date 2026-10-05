//! The canvas document and everything about it that needs no editor: the
//! format, where nodes go, what an edit changes, and where an edge runs.
//!
//! `bezel-canvas` builds the interactive canvas on this and re-exports it.

pub mod change;
pub mod clip;
pub mod contain;
pub mod diagram;
pub mod drag;
pub mod handle;
pub mod json_canvas;
pub mod layout;
#[cfg(feature = "mermaid")]
pub mod mermaid;
pub mod mindmap;
pub mod model;
pub mod paint;
pub mod path;
pub mod snap;

pub use change::Change;
pub use diagram::diagram;
pub use handle::Handle;
pub use layout::Layout;
pub use model::Canvas;
pub use snap::Snap;
