---
title: Thinking orbs
description: Twelve hand-tuned dotted states over a sphere of particles, four sizes, monochrome ink on a transparent canvas.
prompt: Show an animated thinking orb beside the agent's status line while it works, switching to the searching state during a web search.
---

```rust
use agent::orbs::{Orb, OrbSize, OrbState};

let orb = cx.new(|_| Orb::new().state(OrbState::Searching).size(OrbSize::Inline));
let status = orb.read(cx).state_value().label();       // the line beside it

orb.update(cx, |orb, cx| orb.set_state(OrbState::Reasoning, cx));
```

- An orb kept mounted while scrolled off screen keeps ticking; call `set_visible(false)` or unmount it.
- `OrbState` is `#[non_exhaustive]`; match with a wildcard arm, or use `label` / `as_str`.
- `orb_element(state, size, t, &frame)` paints for a host that ticks its own clock. Keep one `frame` per orb; `t = 0.6` is the still frame.
- Ink is monochrome on a transparent background. `OrbTheme::Dark` / `Light` pin it for a surface that is not the window's.
