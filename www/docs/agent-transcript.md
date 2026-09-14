---
title: Transcript
description: A conversation as a scrollback — everything built for the agent port appearing at once, and three reducers that are all std.
---

[`scroll::follow`](/docs/follow) pins it to the newest line, [`Takeover`](/docs/collapsible) runs each turn's work zone, [`step_row`](/docs/step-row) draws the tool calls, and `markdown::markdown` renders the answers.

The screen it was ported from is 943 lines and produced no library code. Its three reducers are all standard library:

| | |
| --- | --- |
| turns | `chunk_by` — start a chunk at every question. |
| the zone split | `rposition`. The answer is the prose after the last tool call; everything before it is interim, which is what stops thinking-out-loud being presented as a reply. |
| a run of tool calls | `chunk_by` again, and the `Verb · N` fold inside it is the same call. |

What is left of the 943 lines is IPC, project lookups, sticky-scroll measurement and error parsing. The beat type is page-local and deliberately not a library type.

The source is at `apps/gallery/src/patterns/transcript.rs`. Copy the file.
