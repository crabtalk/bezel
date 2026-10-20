---
title: Icons
description: The Lucide set ported into typed SVG constants, one cargo feature per Lucide category.
prompt: Put a search icon in the toolbar, a filled star on favourited rows, and our own logo from assets/brand/mark.svg in the sidebar.
---

```rust
use theme::TextStyle;
use ui::icons::{self, Icon, glyph};
use ui::widgets::Icons as _;

theme.icon(glyph::Search)                            // body size, text tone
theme.icon_at(TextStyle::Caption, glyph::Search)
theme.icon(Icon::glyph(glyph::Star).solid())
theme.icon(Icon::asset("brand/mark.svg"))            // the app's AssetSource
theme.icon(Icon::file(picked_path))                  // off disk
icons::icon(glyph::Search).size(px(16.0)).text_color(theme.text_muted)
```

- `icons::icon` with no `text_color` anywhere paints nothing. `theme.icon` supplies size and tone.
- One cargo feature per Lucide category (`bezel-icons = { version = "0.2", features = ["arrows", "navigation"] }`), `full` for all 42, none by default. `ui` turns on `arrows`, `layout`, `notifications`, `shapes` and `text`; the `bezel` facade turns on `full`.
- `Icon::file` loads off the paint thread; the first frame or two paint nothing.
- Naming `icons::CATEGORIES` links every enabled glyph into the binary.
