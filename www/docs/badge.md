---
title: Badge
description: A small right-anchored pill for a count or a state, in a quiet variant and an active one.
prompt: Show a "3 unread" badge on the Inbox row and a green "Active" badge on connected accounts.
---

```rust
use ui::widgets::Content;

theme.badge("3 unread")
theme.badge_active("Active")
```

- Import the `Content` trait to get these on `Theme`.
