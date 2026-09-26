//! A platform webview hosted in a gpui window.
//!
//! The page is a native view (WKWebView on macOS, WebView2 on Windows,
//! webkit2gtk on Linux) above gpui's own surface, not pixels gpui paints.
//! Nothing gpui paints can cover it, and gpui's content masks do not clip it.
//! On the web build, [`Frame`] is an `<iframe>` above the canvas, under the
//! same limits.

#[cfg(target_family = "wasm")]
mod frame;
mod host;
mod page;
mod view;

#[cfg(target_family = "wasm")]
pub use frame::Frame;
pub use view::{EvalError, LoadState, WebView, WebViewEvent};
