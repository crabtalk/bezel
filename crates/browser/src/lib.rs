//! A platform webview hosted in a gpui window.
//!
//! The page is a native view (WKWebView on macOS, WebView2 on Windows) above
//! gpui's own surface, not pixels gpui paints. gpui paints under it, and gpui's content masks do not clip it. On the web
//! build, [`Frame`] is an `<iframe>` above the canvas, under the same limits.

#[cfg(target_family = "wasm")]
mod frame;
mod host;
mod page;
mod store;
mod view;

#[cfg(target_family = "wasm")]
pub use frame::Frame;
pub use store::{DataStore, Usage};
pub use view::{ConsoleLevel, ConsoleMessage, EvalError, LoadState, WebView, WebViewEvent};
