# Browser surfaces in a design canvas

Implementation contract, 14 November 2026. Pages must remain live and
interactive inside the canvas on supported native platforms. No composited
canvas surface is implemented yet.

## Geometry and frame ownership

The page viewport is measured in logical page pixels. It is independent of
canvas zoom, device scale and the rectangle in which the frame is painted.
Changing canvas zoom must not change responsive layout or page scroll.

The browser owns navigation, document state, rendering and buffers. GPUI
owns composition, clipping and stacking. A canvas host owns node geometry,
selection and the current interaction mode. There is one page instance per
hosted frame; copying a node creates an independent page instance.

A completed frame includes its pixel size, page scale and generation. Retain
it until replaced or evicted. A completion from an old viewport generation
must not overwrite a newer frame. A new or recreated atlas tile needs a full
frame before damage-only updates. Release producer buffers only after the
GPU's final read, including error and device-loss paths.

## Input and lifecycle

The host explicitly enables page input. In canvas mode, pointer and wheel
events belong to the canvas. In page mode, the host maps screen coordinates
through the inverse canvas transform into viewport coordinates before
forwarding them. Pointer capture must finish a gesture even if it leaves the
frame. Focus, IME cursor placement and native menus use the same transform.

The host can return to canvas mode without destroying the page. Keyboard
focus and text input must leave the page when that mode ends. Entry and exit
gestures belong to the canvas host; the browser receives explicit interaction
and focus state.

An offscreen or hidden frame retains page state but requests no presentation
callbacks. Showing it again restarts presentation. This does not promise to
suspend arbitrary background JavaScript or network activity. Helper shutdown
and restart must not wait on the UI thread.

## Platform feasibility

| Platform | Existing or available path | Boundary |
| --- | --- | --- |
| Linux | WPEPlatform DMA-BUF or shared-memory frames | Requires the WPE backend and GPUI import and scheduling APIs |
| macOS | WKWebView snapshots; native browser interaction | Public snapshots do not establish continuous texture output or interactive canvas embedding |
| Windows | WebView2 CapturePreview; composition hosting APIs | DirectComposition hosting alone is not a GPUI texture; continuous capture and input require their own implementation |
| Web | Existing iframe overlay | Does not provide same-origin-independent composited pixels; excluded from this native surface contract |

Linux's WPE implementation is not a macOS or Windows backend. Do not claim
live cross-platform canvas rendering from Linux results. A snapshot can be
exported or compared, but is not an interactive live page.

## Platform prerequisites

macOS needs a live presentation and input path using public APIs. Evaluate
WKWebView output and native composition together with GPUI's renderer;
periodically capturing stills does not establish a suitable live path. The
result must preserve fixed layout, canvas scaling, clipping, stacking, page
input and animation. Record an API or performance limitation explicitly if
those requirements cannot be met. No private WebKit API or Chromium dependency.

Windows needs a WebView2 presentation and input path with the same contract.
CapturePreview and composition hosting are candidate APIs, not an implemented
surface. Keep platform work separate from the shared canvas integration.

The canvas release requires these platform prerequisites. Opening another
browser panel to interact with a still does not meet the contract.

## Sources

- [WPEPlatform](https://wpewebkit.org/about/architecture.html)
- [WKWebView snapshots](https://developer.apple.com/documentation/webkit/wksnapshotconfiguration)
- [WebView2 rendering APIs](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-apis)
