//! The canvas document and everything about it that needs no editor: the
//! format, where nodes go, what an edit changes, and where an edge runs.
//!
//! `bezel-canvas` builds the interactive canvas on this and re-exports it.

pub mod change;
pub mod clip;
pub mod contain;
pub mod drag;
pub mod handle;
pub mod layout;
pub mod mindmap;
pub mod model;
pub mod path;
pub mod snap;

pub use change::Change;
pub use handle::Handle;
pub use layout::Layout;
pub use model::Canvas;
pub use snap::Snap;
