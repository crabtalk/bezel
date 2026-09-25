# browse

A Safari-shaped browser built on [`bezel-browser`](../../crates/browser)'s `WebView`: a unified toolbar with the address field in the titlebar, a tab bar, a favorites start page and a sidebar. It is a demo app and is not published.

```sh
cargo run -p browse                    # opens on the start page
cargo run -p browse -- example.com     # opens on a page
cargo build -p browse --release        # the app's own code at opt-level "z"
```

The page is a WKWebView on macOS and WebView2 on Windows. On Linux the window opens but the page area stays empty.

## Keys

`secondary` is cmd on macOS and ctrl elsewhere.

| Keys | Action |
| --- | --- |
| `secondary-t` | New tab |
| `secondary-w` | Close tab |
| `ctrl-tab`, `secondary-shift-]` | Next tab |
| `ctrl-shift-tab`, `secondary-shift-[` | Previous tab |
| `secondary-l` | Edit the address |
| `enter` / `escape` in the address field | Go / cancel |
| `secondary-r` | Reload |
| `secondary-[` / `secondary-]` | Back / forward |
| `secondary-shift-l` | Toggle the sidebar |
| `secondary-q` | Quit |

Text in the address field that has no scheme, contains a dot and no spaces loads over `https://`. Anything else is a DuckDuckGo search.

## Gaps

- Tabs show a globe, not the site's favicon.
- No stop button or load progress: the address field shows only whether a page is loading.
- Back and forward are always enabled.
- On Windows, the page takes every key while it holds focus, so the shortcuts above do not fire from inside a page (BROW-8).
