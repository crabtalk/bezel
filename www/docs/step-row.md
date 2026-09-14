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

```rust
pub trait Status: ThemeExt {
    /// `expanded: None` drops the chevron — a disclosure onto nothing is worse
    /// than none.
    fn step_row(
        &self,
        icon: impl Into<Icon>,
        title: impl Into<SharedString>,
        detail: Option<SharedString>,
        meta: Option<SharedString>,
        failed: bool,
        expanded: Option<bool>,
    ) -> Div;

    /// Monospaced, height-capped, scrolling past the cap — hence the id.
    fn step_output(
        &self,
        id: impl Into<gpui::ElementId>,
        text: impl Into<SharedString>,
    ) -> gpui::Stateful<Div>;

    // ...
}

/// An `Option<bool>`, so "untouched, and here is the manual value" cannot be
/// written.
impl Takeover {
    /// `auto` until the first toggle, the user's own choice from then on.
    pub fn get(self, auto: bool) -> bool;

    /// Flip what is on screen — which while nobody has touched it is `auto`.
    pub fn toggle(&mut self, auto: bool);
}
```
