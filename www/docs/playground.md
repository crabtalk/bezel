---
title: Playground
description: Type Mermaid source and see it drawn as you go — a text field beside a one-fence markdown document, parsed again on every edit.
prompt: Add a Mermaid editor pane with the source on the left and the live diagram on the right, redrawn as I type.
---

```rust
let source = cx.new(|cx| TextField::new(cx).with_shape(Shape::Fill));
cx.subscribe(&source, |this, source, event, cx| {
    if let FieldEvent::Changed(_) = event {
        this.preview = markdown::parse(&format!("```mermaid\n{}\n```", source.read(cx).content()));
        cx.notify();
    }
}).detach();

// render
markdown::render(&self.preview, markdown::Caption::Shown, window, cx)
```

- The fence keeps the diagram it last drew on screen while the next one draws. Source that does not draw shows as code with the error under it.
- `Shape::Fill` makes the field as tall as its parent, which must give it a definite height.
- The source is at `apps/gallery/src/pages/diagrams/playground.rs`. Copy the file.
