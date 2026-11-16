# Browser surfaces in a design canvas

Implementation contract, 14 November 2026. Pages must remain live and
interactive inside the canvas on supported native platforms. Native presentation is implemented for macOS, Linux and Windows.
Windows platform compilation and runtime verification remain user-owned. Linux software rendering is covered by automated WPE/GPUI checks; physical GPU compatibility remains user-owned.

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
| Linux | WPEPlatform DMA-BUF or shared-memory frames | Public WPE platform integration and GPUI composition; software path verified, hardware review pending |
| macOS | Live WKWebView hosted between GPU layers | Fixed viewport, native transforms, scene clips and overlay hit regions; GPU-only exports exclude native content |
| Windows | WebView2 composition controller and Windows Graphics Capture | GPUI shared D3D texture import implemented locally; composition hosting, bounded capture and transformed pointer input implemented; Windows verification pending |
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
`WebView::snapshot` copies the current viewport for export independently of
live presentation.

Headless scene tests and compilation do not establish real WebKit behavior for
IME, pointer capture, animations, native appearance or many-page GPU costs.
Those properties have not been exercised through desktop UI checks.

Windows uses a WebView2 composition controller with continuous Windows
Graphics Capture. The browser supplies bounded BGRA textures and producer
fences on the atlas adapter. GPUI's D3D11 and Wgpu DX12 atlases wait and copy on
the GPU, retaining resources through completion. WebView2 owns keyboard/IME;
caret and native popup placement under transforms remain unverified. Protected
video is excluded by Graphics Capture.

The Windows backend now creates composition controllers directly through
WebView2 COM. A three-texture producer pool bounds capture resources. Restart
epochs discard old events. The active D3D11 atlas and Wgpu's DX12 atlas both
implement the shared-texture contract. Windows compilation and runtime checks
are user-owned and have not run.

The canvas release requires these platform prerequisites. Opening another
browser panel to interact with a still does not meet the contract.

## Image export

`WebView::snapshot` returns owned BGRA pixels. macOS uses WKWebView snapshots;
Windows copies the retained capture texture through a staging resource; Linux
uses the public WebKit snapshot API and exports a separate sealed pixel descriptor.
Snapshot requests do not change live presentation to a CPU frame path. Loading,
failed, unrendered and timed-out pages return errors.

Canvas kinds can prepare an owned export renderer asynchronously. The existing
canvas export pass composes those renderers with text and images at the chosen
canvas geometry, clip and scale. An unavailable frame fails the export with
its node id; it is never silently omitted.

## Sources

- [WPEPlatform](https://wpewebkit.org/about/architecture.html)
- [WKWebView snapshots](https://developer.apple.com/documentation/webkit/wksnapshotconfiguration)
- [WebView2 rendering APIs](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-apis)

- [Microsoft WebView2 composition control](https://github.com/MicrosoftEdge/WebView2Feedback/blob/main/specs/WPF_WebView2CompositionControl.md)
