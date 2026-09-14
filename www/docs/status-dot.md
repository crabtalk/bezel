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

"Working", "idle", "failed" are the caller's domain, so the mapping from a state to a tone stays there rather than becoming an enum every app has to translate.

## API

| | |
| --- | --- |
| `status_dot(tone)` | A 6px bead. |
| tones | `success`, `busy`, `warning`, `danger`, and `text_faint` for one that means nothing yet. |
