---
title: Transcript
description: A conversation as a scrollback — everything built for the agent port appearing at once, and three reducers that are all std.
---

`scroll::follow` pins it to the newest line, `widgets::Takeover` runs each turn's work zone, `widgets::step_row` draws the tool calls, and `markdown::markdown` renders the answers — which is why this page could not be honest until that crate existed.

The screen it was ported from is 943 lines and produced no library code. That was the prediction, and the measurement is that its three reducers are all standard library:

- **Turns** — a question and the answer it drew — are `chunk_by`: start a chunk at every question.
- **The zone split** is `rposition`. The answer is the prose after the last tool call; everything before it is interim. That one sentence is the entire rule, and it is what stops a model's thinking-out-loud from being presented as its reply.
- **A run of adjacent tool calls** is `chunk_by` again, and the `Verb · N` fold inside it is the same `chunk_by` the tool calls page uses.

What is left of the 943 lines is IPC, project lookups, sticky-scroll measurement and error parsing — an app's job, all of it.

The beat type is page-local and deliberately not a library type: `step_row` takes strings, so bezel never learns what a tool call is.

The source is at `apps/gallery/src/patterns/transcript.rs`. Copy the file.
