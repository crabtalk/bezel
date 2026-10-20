---
title: Transcript
description: A conversation as a scrollback — everything built for the agent port appearing at once, and three reducers that are all std.
prompt: Render an agent conversation as a scrollback pinned to the newest line, each turn's tool calls folded into a work section above the final answer in markdown.
---

```rust
for turn in beats.chunk_by(|_, b| !b.is_ask()) {
    // the answer is the prose after the last tool call; the rest is work
    let answer_from = turn.iter().rposition(Beat::is_tool).map_or(0, |i| i + 1);
    let (work, answer) = turn.split_at(answer_from);
    // work: Takeover-driven section of step_rows, folded with chunk_by by verb
    // answer: markdown::markdown
}
```

- There is no transcript component or beat type; the beat type is the app's.
- The pieces are [`scroll::follow`](/docs/follow), [`Takeover`](/docs/collapsible) per turn, [`step_row`](/docs/step-row) and `markdown::markdown`.
- Copy `apps/gallery/src/pages/agent/transcript.rs`.
