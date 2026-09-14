---
title: Tool calls
description: A run of tool calls folded by verb — where the component turned out to be one std call plus a container that already existed.
---

```rust
for run in CALLS.chunk_by(|a, b| a.verb == b.verb) {
    // one row, or a folded group of them
}
```

What looked like a "tool group" component is `slice::chunk_by` over consecutive calls of the same verb. The rows are [`step_row`](/docs/step-row), the output under an open row is `step_output`, and the grouping is std.

A lone call is a bordered card and a grouped one is a bare row, with the group's box owning the border and the hairlines — not a parameter anywhere in the library.

`step_row` takes strings, so bezel never learns what a tool call *is*: the icon, verb, detail and duration are the app's vocabulary.

The source is at `apps/gallery/src/patterns/agent.rs`.
