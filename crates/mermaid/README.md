# bezel-mermaid

Mermaid source drawn as SVG, a port of [Pele](https://github.com/obsidianmd/pele) 1.6.0. No browser, JavaScript engine or GUI toolkit.

```rust
let drawing = mermaid::render("flowchart LR\n  A --> B", &Default::default(), &mermaid::Metrics)?;
assert!(drawing.svg.starts_with("<svg"));
```

## Diagram types

`info`, `pie`, `xychart`, `quadrantChart`, `radar`, `sankey`, `treemap`, `gitGraph`, `packet`, `treeView`, `mindmap`, `railroad`, `venn`, `ishikawa`, `gantt`, `journey`, `kanban`, `timeline`, `classDiagram`, `flowchart`, `stateDiagram`, `erDiagram`, `requirementDiagram`, `sequenceDiagram`, `cynefin`, `wardley`, `eventmodeling`, `C4`, `architecture`, `block`, `agentflow`, `usecase` and `swimlane`.

## API

| Item | What it does |
| --- | --- |
| `render(source, &Options, &host)` | A `Drawing`: `svg`, `width`, `height`, `kind` and `links` |
| `parse(source, &Options)` | A `Parsed`, whose `model` serializes as Pele's `parse` does |
| `render_parsed(&parsed, &Options, &host)` | Draws a `Parsed` |
| `detect_type`, `supports` | The type a source names, and whether this build draws it |

`Options` takes Pele's render options; `Default` is Pele's defaults. `colors` takes `Colors`, written in place of every CSS variable, for renderers without custom properties, and `now` the time gantt charts treat as now.

The host is a `Host`: `width` answers how wide a run of text is in the fonts the drawing is shown in, and `icon` the SVG markup for an icon name (none by default). `Metrics` is a `Host` with Arial's advances and no icons.

Errors carry Pele's messages, a `Code`, and the line, column and snippet.

## Output

The SVG matches Pele's, with class names, CSS variables and ids prefixed `bezel` instead of `pele`. It does not match mermaid.js's output.

## Not supported

- Incremental layout: a changed source is drawn again from the start.

## Tests

```sh
cargo nextest run -p bezel-mermaid
```

`tests/mermaid/compat/` holds Pele's `parse` output for each type, written by `build.rs` with Node in a checkout, and `tests/mermaid/corpus/` Mermaid's documentation examples. `MERMAID_PELE_SVG=<dir>` compares every drawing against Pele's SVG, written by `node tests/mermaid/pele/render.mjs svg <dir>`.

## Benchmark against Pele

```sh
npm --prefix crates/mermaid/tests/mermaid/pele run bench
```

Times Pele in Node and this crate at `opt-level = 3` (`cargo bench`) and with `--profile release`, on Pele's benchmark cases and the corpus. It prints each case side by side and the geometric mean.
