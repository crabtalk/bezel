# bezel-browser

A platform webview hosted in a gpui window. The page is a native view above gpui's own surface, not pixels gpui paints: gpui paints under it, and gpui's content masks do not clip it.

```rust
let page = cx.new(|cx| browser::WebView::new("https://example.com", window, cx));
```

## Platforms

| Platform | Page |
| --- | --- |
| macOS | WKWebView |
| Windows | WebView2 |
| Web (wasm) | `Frame`, an `<iframe>` above the canvas |
| Linux and elsewhere | None: the crate compiles and `WebView` paints nothing |

## Not supported

- Linux.
- Downloads: the crate has no download API.
