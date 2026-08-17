---
title: Tool calls
description: A run of tool calls folded by verb — where the component turned out to be one std call plus a container that already existed.
---

What looked like a component — a "tool group" — is `slice::chunk_by` over consecutive calls of the same verb:

```rust
for run in CALLS.chunk_by(|a, b| a.verb == b.verb) {
    // one row, or a folded group of them
}
```

bezel wrote nothing for that. The rows are `widgets::step_row`, the output under an open row is `widgets::step_output`, and the grouping is std.

The two shapes are here as well, and they are not a parameter anywhere in the library: a lone call is a bordered card, a grouped one is a bare row, and the group's box owns the border and the hairlines between its rows.

`step_row` takes strings, so bezel never learns what a tool call *is*. The icon, the verb, the detail and the duration are the app's vocabulary; the row is the shape they share with a CI step and a migration.

The source is at `apps/gallery/src/patterns/agent.rs`.
