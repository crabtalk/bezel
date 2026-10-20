---
title: Status dot
description: The small state bead on a row — the tone comes from the caller, because the meaning does.
prompt: Put a green, amber or red dot beside each job in the list for running, queued and failed.
---

```rust
use ui::widgets;

widgets::status_dot(match job.state {
    State::Running => theme.success,
    State::Queued => theme.warning,
    State::Failed => theme.danger,
})
```

- The caller maps its own states to a theme tone.
