# bezel-browser

A platform webview hosted in a GPUI window. On macOS, native/GPU composition
preserves content masks, scene stacking and live WebKit content. Linux uses WPEPlatform buffers composed by GPUI. Windows uses WebView2 composition hosting and Windows Graphics Capture.

```rust
let page = cx.new(|cx| browser::WebView::new("https://example.com", window, cx));
```

## Platforms

| Platform | Page |
| --- | --- |
| macOS | WKWebView with native/GPU scene composition |
| Windows | WebView2 composition controller and shared D3D textures |
| Web (wasm) | `Frame`, an `<iframe>` above the canvas |
| Linux | System WPE WebKit 2.54+ with WPEPlatform; DMA-BUF or shared-memory frames |
| Elsewhere | No native backend |

## Fixed viewport

On macOS, Linux and Windows, `WebView::set_presentation` accepts a `Presentation` with a validated
`gpui::native_view::Viewport` and an `interactive` flag. The viewport determines
page layout; the element's displayed bounds determine canvas scaling. Disabled
input passes pointer and wheel events through to the canvas and releases page
keyboard focus. `None` restores a responsive viewport and page input.

Pages remain live while composited. Hiding retains document state without
promising to suspend background JavaScript or network activity. Native pages
and materials are not included automatically in GPUI's GPU-only image export.
Use `WebView::snapshot` to acquire page pixels for a separate export pass.
Loading, failed, unrendered and timed-out pages return errors.

Linux runtime setup and build requirements are in
[linux-runtime.md](design/linux-runtime.md). Linux platform behavior has not
been verified on hardware.

## Not supported

- Live canvas composition on web.
- Downloads: the crate has no download API.
- Linux developer inspector UI and rich clipboard formats.
