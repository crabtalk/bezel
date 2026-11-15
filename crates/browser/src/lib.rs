//! Platform web pages hosted in GPUI windows.
//!
//! macOS participates in native/GPU scene composition, including content masks
//! and overlay stacking. Windows uses a native child-view overlay. On the web,
//! [`Frame`] hosts an iframe above the canvas.

#[cfg(target_family = "wasm")]
mod frame;
mod host;
mod page;
mod store;
mod view;

#[cfg(target_family = "wasm")]
pub use frame::Frame;
pub use store::{DataStore, Usage};
pub use view::{
    ConsoleLevel, ConsoleMessage, EvalError, LoadState, Presentation, WebView, WebViewEvent,
};
