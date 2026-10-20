---
title: Diff
description: A file review — and the page whose finding is that it produced no library code at all.
prompt: Show the agent's edit to a file as a unified diff with line numbers, green added lines and red removed lines, under a header with the path.
---

```rust
let (wash, mark, sign) = match change {
    Change::Added => (theme.success.opacity(0.1), theme.success, "+"),
    Change::Removed => (theme.danger.opacity(0.1), theme.danger, "-"),
    Change::Context => (gpui::transparent_black(), theme.text_faint, " "),
};
div().flex().flex_row().gap(px(8.0)).bg(wash).font_family(theme.font_mono.clone())
    .child(number(old)).child(number(new))
    .child(div().text_color(mark).child(sign))
    .child(text)
```

- There is no diff component, and bezel never computes a diff: the rows arrive decided. No hunk folding, word-level marks, syntax highlighting or side-by-side panes.
- The palette also carries `diff_add`, `diff_del` and `diff_hunk_bg`.
- Copy `apps/gallery/src/pages/agent/diff.rs`.
