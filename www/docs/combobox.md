---
title: Combobox
description: A select you can type into — the closed face of a select over an anchored menu whose rows narrow as you search.
---

```rust
use ui::combobox::{self, Combobox, ComboboxEvent};

combobox::init(cx);   // once, at startup, alongside input::init

let language = cx.new(|cx| Combobox::new(LANGUAGES.to_vec(), "Language", cx));

cx.subscribe(&language, |_, _, event, _| match event {
    ComboboxEvent::Selected(index) => { /* item `index` */ }
})
.detach();
```

An entity for the same reason the command palette is one — it owns a query `TextField`. The two share `popover::Filter` and differ only in frame: the palette is a modal over every command, this hangs under a trigger and remembers what was chosen.

The reported index is into the **original** item list, never into the filtered view.

The menu matches the trigger's width, measured from the last frame's layout. An anchored layer sizes to its own content, so without measuring, a combobox's menu could not line up with its own face.

Keys are the palette's set — `up`/`down`, `ctrl-p`/`ctrl-n`, `enter`, `escape` — scoped to a context that wraps the query field's, so typing reaches the field and navigation falls through.
