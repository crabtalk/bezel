# Linux browser runtime

The Linux backend uses a separate WPEPlatform helper. Platform compilation
and runtime behavior still need verification on Linux.

## Runtime contract

Require system-installed WPE WebKit 2.54 or newer with WPEPlatform enabled,
plus its WebKit subprocesses and sandbox support. Target x86_64 and aarch64;
release support requires the backend compatibility results for each target.
Do not bundle Chromium or provide a WebKitGTK fallback.

The application must start without WPE installed. Only the browser helper
links WPE; a missing, incompatible or unstartable runtime is a browser error,
not an application startup failure. Report the detected version and the
required version. Never silently substitute an ephemeral profile.

The distribution package manager installs and updates WebKit and its
transitive dependencies. The application does not install packages, enable
third-party repositories or disable the WebKit sandbox automatically. Runtime
setup must use the versioned package names below and check WPEPlatform
availability, not merely the presence of a WebKit shared library.

## Implementation

The helper is compiled on Linux and extracted to a content-addressed executable
under `$XDG_CACHE_HOME/bezel/browser` (or `~/.cache/bezel/browser`).
`BEZEL_WPE_HELPER` overrides that executable for development. Only the helper
links WPE libraries.

Persistent pages share a helper per canonical profile directory. The default
profile is `$XDG_DATA_HOME/bezel/browser/default`, falling back to
`~/.local/share/bezel/browser/default`. Private pages use independent ephemeral
sessions. Clearing a persistent store requires closing its pages first.

The socket protocol carries bounded JSON packets and file descriptors. Commands
are queued away from the UI thread. Startup, storage operations, shutdown and
process reaping run in background tasks. Failure reaches `WebViewEvent::Error`;
loading or reloading starts a new session after failure.

WPE negotiates single-plane AR24 DMA-BUF modifiers with GPUI's Vulkan device.
Unsupported imports use shared-memory frames. Producer buffers remain held until
GPU reads finish. Presentation acknowledgments follow GPUI display callbacks.
Viewport generations reject obsolete frames after resize.

Pointer coordinates and IME caret bounds use the fixed page viewport and painted
bounds. Text clipboard, edit commands, context menus and option menus cross the
helper boundary; rich clipboard formats and an inspector UI are unavailable.

The helper build requires a C compiler and pkg-config modules `wpe-webkit-2.0`
(2.54+), `wpe-platform-2.0`, `json-glib-1.0`, `gbm`, `egl`, `glesv2` and
`xkbcommon`. Runtime users do not need development headers or pkg-config.

## Package availability

Checked 14 November 2026. Package availability is not hardware verification.

| Distribution | Available package | Browser eligibility |
| --- | --- | --- |
| Arch Linux x86_64 | `wpewebkit` 2.54.2-1 in Extra | Meets version floor |
| Debian testing and unstable | `libwpewebkit-2.0-1` 2.54.2-1, including amd64 and arm64 | Meets version floor |
| Debian 13 stable | `libwpewebkit-2.0-1` 2.48.3-1 | Below version floor |
| Other releases and architectures | Not established by this review | Require package and backend verification |

Arch lists approximately 40.4 MB compressed and 148.9 MB installed for its
WPE package. These are package-only figures, excluding dependencies; the
incremental download depends on the user's installed packages. Do not use
these numbers as a bundled runtime budget.

Developers additionally need the matching development package supplying
`wpe-webkit-2.0` and `wpe-platform-2.0` pkg-config modules, a C compiler, and
any graphics libraries the custom platform implementation uses. Runtime
users do not need development headers or pkg-config.

## Sources

- [Stable WPEPlatform API](https://wpewebkit.org/release/wpewebkit-2.54.0.html)
- [WPE integration architecture](https://wpewebkit.org/about/architecture.html)
- [Arch package](https://archlinux.org/packages/extra/x86_64/wpewebkit/)
- [Debian package versions](https://packages.debian.org/search?keywords=libwpewebkit-2.0-1&searchon=names&suite=all&section=all)
