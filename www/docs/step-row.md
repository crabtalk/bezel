---
title: Step row
description: One operation as a row — icon, what it was, how it went — with an optional disclosure onto its verbatim output.
prompt: Show each tool call in the agent transcript as a row with its command and duration, opening onto its output while it runs and on click.
---

```rust
use ui::{icons, widgets::{Status, Takeover}};

let open = self.details.get(self.running);        // follows `running` until clicked
div()
    .child(
        theme.step_row(icons::glyph::Terminal, "Bash", Some("cargo test -p ui".into()), Some("1.4s".into()), false, Some(open))
            .id("step-3")
            .on_click(cx.listener(|view, _, _, cx| view.details.toggle(view.running))),
    )
    .when(open, |step| step.child(theme.step_output("step-3-out", stdout)))
```

- Put `.id(..)` and `.on_click(..)` on the row itself, never on a wrapper.
- `expanded: None` draws no chevron; `Some` draws it and the caller owns the flag.
- `detail` truncates; `meta` is right-aligned and never truncates.
- `step_output` is capped in height and scrolls past it; a wheel over it never scrolls the page.
