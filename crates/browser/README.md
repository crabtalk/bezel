# bezel-browser

A platform webview hosted in a GPUI window. On macOS, native/GPU composition
preserves content masks, scene stacking and live WebKit content. Windows uses
a native child-view overlay.

```rust
let page = cx.new(|cx| browser::WebView::new("https://example.com", window, cx));
```

## Platforms

| Platform | Page |
| --- | --- |
| macOS | WKWebView with native/GPU scene composition |
| Windows | WebView2 |
| Web (wasm) | `Frame`, an `<iframe>` above the canvas |
| Linux and elsewhere | None: the crate compiles and `WebView` paints nothing |

## Fixed viewport

On macOS, `WebView::set_presentation` accepts a `Presentation` with a validated
`gpui::native_view::Viewport` and an `interactive` flag. The viewport determines
page layout; the element's displayed bounds determine canvas scaling. Disabled
input passes pointer and wheel events through to the canvas and releases page
keyboard focus. `None` restores a responsive viewport and page input.

Pages remain live while composited. Hiding retains document state without
promising to suspend background JavaScript or network activity. Native pages
and materials are not included in GPUI's GPU-only image export.

## Not supported

- Linux browsing.
- Live canvas composition on Windows or web.
- Downloads: the crate has no download API.
