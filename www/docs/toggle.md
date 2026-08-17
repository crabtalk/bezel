---
title: Toggle
description: An 18×32 switch whose knob slides and whose track fills with the text tone when on — state stays with the row that owns it.
---

```rust
use bezel_ui::widgets;

widgets::toggle(&theme, self.enabled)
```

Display-only, like the rest of `widgets`: the caller adds `.id(..)` and `.on_click(..)`, and holds the bool.

Tab focus and `space`/`enter` come from the same wrapper every stateless control uses:

```rust
focus::focusable(&theme, &self.switch, widgets::toggle(&theme, self.enabled))
```
