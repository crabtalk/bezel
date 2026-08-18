---
title: Step row
description: One operation as a row — icon, what it was, how it went — with an optional disclosure onto its verbatim output.
---

A tool call in a transcript, a step in a CI run, a file in a migration: the shape is the same everywhere, which is why this takes strings rather than a type that knows what any of them mean.

```rust
use ui::widgets;

widgets::step_row(
    &theme,
    icons::TERMINAL,
    "Bash",
    Some("cargo test -p ui".into()),  // truncating middle
    Some("1.4s".into()),                     // right-aligned, never truncates
    false,                                   // failed
    Some(open),                              // has an output to disclose
)
.id("step-3")
.on_click(cx.listener(..))
```

`detail` is the middle that truncates — a query, a path, a `· 3` count. `meta` is the right-aligned figure that does not: a duration, a size, a row count.

`expanded` is `None` when there is nothing under the row, and then the chevron is simply absent. A disclosure that opens onto nothing is worse than no disclosure.

Add `.id(..)` and `.on_click(..)` **to the row itself**, never to a wrapper around it, or the hitbox ends up narrower than what it paints.

What it opens onto is `step_output`:

```rust
widgets::step_output(&theme, "step-3-out", stdout)
```

Monospaced and capped in height, because the thing being shown is a program's stdout and the row it hangs off is one line tall — a 900-line stack trace pushing the next step off screen is the failure the cap exists for. Past the cap it scrolls, which is why it takes an id. No scrollbar: the wheel reaches it anyway, and a bar would demand a `ScrollHandle` and a `ScrollbarState` from every caller for a box that is usually four lines long.

A row that opens itself while work streams in wants `Takeover`:

```rust
let open = self.details.get(self.running); // follows `running` until touched
self.details.toggle(self.running);         // …and the click wins from here
```

It is an `Option<bool>` rather than the two flags it reads as, because "untouched, and here is the manual value" is a state that cannot mean anything — this way it cannot be written.
