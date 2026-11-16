# Independent WPEPlatform proof

Run on Linux with a C compiler, D-Bus, WPE WebKit 2.54+ development files, `wpe-platform-headless-2.0`, GPUI's Linux development dependencies, and a Vulkan implementation:

```sh
dbus-run-session -- cargo nextest run -p bezel-wpe-proof --features integration --no-capture
```

This opt-in harness contains an independent platform proof and production integration checks. `tests/platform.c` and `tests/integration.rs` exercise a minimal public WPE platform and real offscreen GPUI renderer. `tests/production.rs` includes the production transport and presentation modules directly and compiles the production C helper. Neither implementation uses Zeron code.

## Exercised boundary

- A custom `WPEDisplay` creates a custom `WPEView`. The view implements `render_buffer`; WPE's public `WPEToplevelHeadless` supplies the toplevel.
- System WebKit renders HTML into `WPEBufferSHM`. Borrowed pixels from `wpe_buffer_import_to_pixels` cross a pipe as complete BGRA8 frames.
- The helper retains each buffer. The host submits pixels through GPUI's `Window::update_image`. Only its GPU-completion callback acknowledges the buffer; the helper then calls `wpe_view_buffer_rendered` and `wpe_view_buffer_released` and drops its reference.
- Real WPE pointer events click the page; keyboard events type into a focused input. The test waits for the page's focus report before typing.
- Doubling GPUI's painted image dimensions preserves the page's 320×200 logical viewport. Screenshots verify the page's red, green and blue states.
- Only the helper links WPE. An incompatible runtime makes its response channel disconnect and its process fail; the Rust host remains running.
- Normal shutdown is requested over the pipe. Test cleanup kills the owned helper/D-Bus process group on failure.

## Verified environment

Verified on 16 November 2026 with Debian sid aarch64, WPE 2.54.2, GPUI 0.3.25+zed.e4a373, and Mesa llvmpipe Vulkan. WebKit's sandbox remained enabled. The container ran as a nonroot user with Docker's seccomp and masked system-path restrictions lifted so bubblewrap could create its own namespaces. No desktop window or physical GPU was used.

The minimal proof uses software frames copied through a pipe. Production checks use sealed pixel files and descriptor transport, GPUI GPU-completion acknowledgments, resize generations, snapshots, raw keys and IME composition, hide/show, persistent/private storage and helper shutdown. DMA-BUF import, hardware drivers and distribution compatibility require the separate hardware review.

## API references

- [WPEPlatform overview](https://wpewebkit.org/reference/2.54.0/wpe-platform-2.0/overview.html)
- [Platform implementation guide](https://wpewebkit.org/reference/2.54.0/wpe-platform-2.0/tutorial-platform.html)
- [WPEBuffer pixel ownership](https://wpewebkit.org/reference/2.54.0/wpe-platform-2.0/method.Buffer.import_to_pixels.html)

## Production ownership

The production helper, transport and presenter replace the previously attributed implementation. WPE owns engine lifecycle and platform buffers; the helper implements the public display/view, input and clipboard contracts; owned Rust descriptors and frame leases bridge those buffers to GPUI. GPUI owns atlas import, clipping, composition and GPU completion. The protocol and public Bezel API remain compatible.
