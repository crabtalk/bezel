---
title: Diff
description: A file review — and the pattern whose finding is that it produced no library code at all.
---

A diff row is two numbers, a sign and a line of text: three `div`s and a color, with no reducer, no state and nothing to measure. Next to `tree` (flattening plus an arrow-key walk) or `table` (a sort reducer and a cell-count guard) it would be a paint helper wearing a component's badge — so it lives in the file you would copy rather than in `crates/ui`.

The attempt is the finding. What is actually hard about a diff view — folding hunks, word-level marks inside a changed line, syntax highlighting, two panes scrolling together — is either the app's or waits on a syntax crate. None of it is here, and calling this a component would have promised all four.

The rows arrive already decided. bezel never computes a diff; whatever produced it is the app's business, the same line `tree` holds about your file system.

The header over it is `widgets` chrome, and the tones are the palette's `diff_add`, `diff_del` and `diff_hunk_bg`.

The source is at `apps/gallery/src/patterns/diff.rs`.
