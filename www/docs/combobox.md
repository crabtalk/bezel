---
title: Combobox
description: A select you can type into — the closed face of a select over an anchored menu whose rows narrow as you search.
prompt: Add a language picker to the form that I can type into to narrow the list, and tell me which language was picked.
---

```rust
use ui::combobox::{self, Combobox, ComboboxEvent};

combobox::init(cx);   // once, at startup, alongside input::init

let language = cx.new(|cx| Combobox::new(LANGUAGES.to_vec(), "Language", cx));

cx.subscribe(&language, |_, _, event, _| match event {
    ComboboxEvent::Selected(index) => { /* LANGUAGES[index] */ }
})
.detach();
```

- The reported index is into the original item list, never the filtered view.
