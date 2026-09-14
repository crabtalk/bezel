---
title: Alert strips
description: Two inline notices — red for what failed, amber for what wants attention — each a leading icon and a line of copy.
---

```rust
use ui::widgets::Status;

theme.error_strip("Something went wrong.")
theme.warning_strip("Heads up, check this.")
```

Two tones, not a level enum: a strip is either the thing that failed or the thing to watch. Both return a `Div` — a dismiss control is a child you add.

## API

| | |
| --- | --- |
| `error_strip(message)` | The `danger` family. |
| `warning_strip(message)` | The `warning` family. |
