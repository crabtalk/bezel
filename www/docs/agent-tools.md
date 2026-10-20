---
title: Tool calls
description: A run of tool calls folded by verb — where the component turned out to be one std call plus a container that already existed.
prompt: List the agent's tool calls as rows, folding consecutive calls with the same verb into one expandable "Read · 3" group.
---

```rust
use ui::widgets::Status;

for run in calls.chunk_by(|a, b| a.verb == b.verb) {
    let row = theme
        .step_row(run[0].icon, run[0].verb, Some(format!("· {}", run.len()).into()), None, failed, Some(open))
        .id(SharedString::from(format!("group-{start}")))
        .on_click(cx.listener(move |view, _, _, cx| view.toggle(start, cx)));
    // open: one row per call, each `theme.step_output(id, output)` under it
}
```

- `.id` and `.on_click` go on the row `step_row` returns, never on a wrapper.
- `expanded: None` draws no chevron; the caller owns the open flag.
- There is no tool-call type: icon, verb, detail and duration are the app's strings. See [`step_row`](/docs/step-row); the demo is `apps/gallery/src/pages/agent/activity.rs`.
