//! JSON Canvas data and interactive GPUI views.
//!
//! The model, change operations, layouts and JSON Canvas import/export are
//! available without default features. Interactive views require `gpui`.

#[cfg(feature = "gpui")]
mod app;
#[cfg(feature = "gpui")]
pub use app::AppExt;

#[cfg(feature = "gpui")]
mod arrange;
#[cfg(feature = "gpui")]
pub use arrange::Arrange;
pub mod change;
pub mod clip;
pub mod contain;
pub mod drag;
#[cfg(feature = "gpui")]
pub mod export;
#[cfg(feature = "gpui")]
pub mod handle;
pub mod json_canvas;
pub mod layout;
pub mod mindmap;
pub mod model;
#[cfg(feature = "gpui")]
pub mod paint;
#[cfg(feature = "gpui")]
pub mod path;
pub mod snap;

pub use change::Change;
#[cfg(feature = "gpui")]
pub use handle::Handle;
pub use layout::Layout;
pub use model::Canvas;
pub use snap::Snap;

#[cfg(feature = "gpui")]
mod commands;
#[cfg(feature = "gpui")]
mod document;
#[cfg(feature = "gpui")]
pub mod edge;
#[cfg(feature = "gpui")]
pub use document::{Document, DocumentChange, DocumentEvent};
#[cfg(feature = "gpui")]
mod edit;
#[cfg(feature = "gpui")]
pub mod kind;
#[cfg(feature = "gpui")]
mod minimap;
#[cfg(feature = "gpui")]
pub mod options;
#[cfg(feature = "gpui")]
pub mod presentation;
#[cfg(feature = "gpui")]
mod preview;
#[cfg(feature = "gpui")]
pub mod tool;
#[cfg(feature = "gpui")]
mod view;

#[cfg(feature = "gpui")]
pub use edge::{EdgeKind, EdgeKinds};
#[cfg(feature = "gpui")]
pub use edit::{CanvasEditor, CanvasEvent, Item};
#[cfg(feature = "gpui")]
pub use kind::{Kind, Kinds};
#[cfg(feature = "gpui")]
pub use minimap::minimap;
#[cfg(feature = "gpui")]
pub use options::{Options, Overlays, Style};
#[cfg(feature = "gpui")]
pub use tool::Tool;
#[cfg(feature = "gpui")]
pub use view::{CONTEXT, CanvasView, init, keys};
