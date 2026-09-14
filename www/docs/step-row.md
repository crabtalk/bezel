---
title: Step row
description: One operation as a row — icon, what it was, how it went — with an optional disclosure onto its verbatim output.
---

```rust
use ui::{icons, widgets::Status};

theme.step_row(
    icons::glyph::Terminal,
    "Bash",
    Some("cargo test -p ui".into()),  // detail: truncates in the middle
    Some("1.4s".into()),              // meta: right-aligned, never truncates
    false,                            // failed
    Some(open),                       // expanded
)
.id("step-3")
.on_click(cx.listener(..))
```

Put `.id(..)` and `.on_click(..)` on the row itself, never on a wrapper, or the hitbox ends up narrower than what it paints.

## Output

```rust
theme.step_output("step-3-out", stdout)
```

## Following a run

```rust
let open = self.details.get(self.running); // follows `running` until touched
self.details.toggle(self.running);         // …and the click wins from here
```

## API

| | |
| --- | --- |
| `step_row(icon, title, detail, meta, failed, expanded)` | `expanded: None` drops the chevron — a disclosure onto nothing is worse than none. |
| `step_output(id, text)` | Monospaced, height-capped, scrolls past the cap; hence the id. |
| `Takeover` | `get(auto)` / `toggle(auto)` — an `Option<bool>`, so "untouched, and here is the manual value" cannot be written. |
