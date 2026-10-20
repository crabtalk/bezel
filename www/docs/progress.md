---
title: Progress
description: A determinate bar — a clamped fraction over a track that keeps its full width.
prompt: Show a progress bar under the upload row that fills as the file uploads.
---

```rust
use ui::widgets::Controls;

theme.progress_bar(self.sent as f32 / self.total as f32)
```

- Determinate only. For work of unknown length, use a [loader](/docs/loaders).
