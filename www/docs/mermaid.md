---
title: Mermaid
description: Mermaid source drawn as SVG in Rust, with no browser or JavaScript engine — as a fence in a markdown document, or on its own.
prompt: Render the ```mermaid blocks in my markdown notes as diagrams, and export a flowchart to an SVG file from a command.
---

```rust
// In a document: markdown = { version = "0.2.20", features = ["mermaid"] }

// On its own:
let drawing = mermaid::render("flowchart LR\n  A --> B", &mermaid::Options::default(), &Fonts)?;
std::fs::write("flow.svg", &drawing.svg)?;

struct Fonts;
impl mermaid::Host for Fonts {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 { /* … */ }
    fn icon(&self, name: &str) -> Option<String> { None }  // optional
}
```

- In a document, the fence draws off the main thread and keeps the diagram it last drew until the next is ready, or stands empty. Source that does not draw shows as code with the error under it.
- `Host::width` must answer as the fonts the SVG is shown in. `mermaid::Metrics` answers with Arial's advances and has no icons.
- The SVG is Pele 1.6.0's, with class names, CSS variables and ids prefixed `bezel`; it does not match mermaid.js's. For a renderer without CSS custom properties, set `Options::colors`.
- `bezel-mermaid` builds without gpui, for `wasm32-unknown-unknown` too.
