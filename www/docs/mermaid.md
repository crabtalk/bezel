---
title: Mermaid
description: Mermaid source drawn as SVG in Rust, with no browser or JavaScript engine — as a fence in a markdown document, or on its own.
---

## In a document

```toml
markdown = { version = "0.2.18", features = ["mermaid"] }
```

With the feature on, a fence tagged `mermaid` is drawn as its diagram, in the theme's colours and fonts. Drawing runs on gpui's background executor; until it finishes the fence stands empty. Source that does not draw shows as code with the error under it. A press on the diagram pans it.

## On its own

```rust
let drawing = mermaid::render("flowchart LR\n  A --> B", &mermaid::Options::default(), &mermaid::Metrics)?;
// drawing.svg, drawing.width, drawing.height, drawing.kind, drawing.links
```

`bezel-mermaid` is a port of [Pele](https://github.com/obsidianmd/pele) 1.6.0 and builds without gpui, for `wasm32-unknown-unknown` too. Its SVG matches Pele's, with class names, CSS variables and ids prefixed `bezel`; it does not match mermaid.js's.

The host answers how wide a run of text is in the fonts the drawing is shown in, and the markup for an icon name. `Metrics` is a host with Arial's advances and no icons.

```rust
impl mermaid::Host for Fonts {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 { /* … */ }
    fn icon(&self, name: &str) -> Option<String> { None }
}
```

`Options::default()` draws as Pele does with no options. `colors` takes a `Colors` for a renderer without CSS custom properties, and `now` the time a gantt chart treats as now.

## API

```rust
pub fn render(source: &str, options: &Options, host: &impl Host) -> Result<Drawing, Error>;
pub fn parse(source: &str, options: &Options) -> Result<Parsed, Error>;
pub fn render_parsed(parsed: &Parsed, options: &Options, host: &impl Host) -> Result<Drawing, Error>;

/// The type a source names, and whether this build draws it.
pub fn detect_type(source: &str) -> Option<Kind>;
pub fn supports(source: &str) -> bool;
```

Errors carry Pele's message, a `Code`, and the line, column and snippet. Each page in this tab is one diagram type and the source that draws it.
