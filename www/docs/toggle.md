---
title: Toggle
description: An 18×32 switch whose knob slides and whose track fills with the text tone when on — state stays with the row that owns it.
---

```rust
use ui::widgets::Controls;

theme.toggle(self.enabled)
```

Display-only, like the rest of `widgets`: the caller adds `.id(..)` and `.on_click(..)`, and holds the bool.

## Keyboard

```rust
focus::focusable(&theme, &self.switch, theme.toggle(self.enabled))
```

## API

```rust
pub trait Controls: ThemeExt {
    fn toggle(&self, on: bool) -> Div;

    // ...
}
```
