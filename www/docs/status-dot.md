---
title: Status dot
description: The small state bead on a row — the tone comes from the caller, because the meaning does.
---

```rust
use ui::widgets;

widgets::status_dot(theme.success)
widgets::status_dot(theme.busy)
widgets::status_dot(theme.danger)
```

The only parameter is the color. "Working", "idle", "failed" are the caller's domain, so the mapping from a state to a tone stays there rather than becoming an enum in the library that every app has to translate into.

The palette carries the tones worth using: `success`, `busy`, `warning`, `danger`, and `text_faint` for a bead that means nothing in particular yet.
