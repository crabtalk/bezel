# Browser surfaces in a design canvas

Implementation contract, 14 November 2026. Pages must remain live and
interactive inside the canvas on supported native platforms. macOS has a
native/GPU presentation path; shared canvas integration remains separate.

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
forwarding them on buffer-based backends. macOS uses AppKit view transforms
and native event delivery. Pointer capture must finish a gesture even if it leaves the
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
| Linux | WPEPlatform DMA-BUF or shared-memory frames | Backend and GPUI import/scheduling implemented locally; Linux verification pending |
| macOS | Live WKWebView hosted between GPU layers | Fixed viewport, native transforms, scene clips and overlay hit regions; GPU-only exports exclude native content |
| Windows | WebView2 composition controller and Windows Graphics Capture | GPUI shared D3D texture import implemented locally; browser capture and transformed input unfinished |
| Web | Existing iframe overlay | Does not provide same-origin-independent composited pixels; excluded from this native surface contract |

Linux's WPE implementation is not a macOS or Windows backend. Do not claim
live cross-platform canvas rendering from Linux results. A snapshot can be
exported or compared, but is not an interactive live page.

## Platform prerequisites

macOS presents WKWebView in GPUI's native/GPU compositor through public AppKit
APIs. `WebView::set_presentation` takes a fixed logical viewport and an explicit
input flag. The compositor owns the native view's hierarchy, frame and visibility;
the browser retains navigation and document state. AppKit handles transformed
hit testing, scrolling, pointer gestures and IME. Scene hit regions prevent page
input through overlaid GPUI controls. Cached scenes share live input state and
consume keyboard-focus requests once. Successful presentation rebinds browser
ownership when a cached page moves between windows.

Native Liquid Glass hosts its foreground in `contentView`; related surfaces use
native glass containers. The same material path applies over pages and other
application content. No page snapshots are used for macOS display composition.
Native views require a separate pixel-acquisition path for export.

Headless scene tests and compilation do not establish real WebKit behavior for
IME, pointer capture, animations, native appearance or many-page GPU costs.
Those properties have not been exercised through desktop UI checks.

Windows still needs a WebView2 capture/hosting and input path with this contract.
Microsoft's WebView2CompositionControl uses continuous Windows Graphics Capture.
GPUI now accepts shared BGRA D3D textures and producer fences on the DX12 queue.
The browser must supply bounded capture buffers, same-adapter resources and
completion ownership. WebView2's composition controller owns keyboard/IME;
the host must establish caret and popup placement under canvas transforms.
Repeated CapturePreview is not the selected live path. Protected video is
excluded by Graphics Capture.

The existing Wry backend creates a windowed controller; it cannot be cast into
composition-controller hosting. The live backend requires direct WebView2 COM
creation and lifecycle ownership. Windows compilation and runtime checks are
user-owned; the capture/hosting backend has not been implemented.

The canvas release requires these platform prerequisites. Opening another
browser panel to interact with a still does not meet the contract.

## Sources

- [WPEPlatform](https://wpewebkit.org/about/architecture.html)
- [WKWebView snapshots](https://developer.apple.com/documentation/webkit/wksnapshotconfiguration)
- [WebView2 rendering APIs](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-apis)

- [Microsoft WebView2 composition control](https://github.com/MicrosoftEdge/WebView2Feedback/blob/main/specs/WPF_WebView2CompositionControl.md)
