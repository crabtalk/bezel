# Linux browser runtime

Accepted runtime policy, 14 November 2026. No Linux browser backend is
implemented yet.

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
