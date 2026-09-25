//! A platform webview hosted in a gpui window.
//!
//! The page is a native view (WKWebView on macOS, WebView2 on Windows) above
//! gpui's own surface, not pixels gpui paints. Nothing gpui paints can cover
//! it, and gpui's content masks do not clip it.

mod view;

pub use view::{EvalError, LoadState, WebView, WebViewEvent};
