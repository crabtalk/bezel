---
title: Alert strips
description: Two inline notices — red for what failed, amber for what wants attention — each a leading icon and a line of copy.
prompt: Show a red error strip above the form when saving fails, and an amber warning when the API key is about to expire.
---

```rust
use ui::widgets::Status;

theme.error_strip("Couldn't save: the server refused the request.")
theme.warning_strip("Your API key expires in 3 days.")
```

- There are two tones and no level enum.
- A dismiss control is a child you add.
