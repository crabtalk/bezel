# bezel-mermaid

Mermaid source drawn as SVG, a port of [Pele](https://github.com/obsidianmd/pele) 1.6.0. No browser, JavaScript engine or GUI toolkit.

```rust
let drawing = mermaid::render("flowchart LR\n  A --> B", &Default::default())?;
assert!(drawing.svg.starts_with("<svg"));
```

## Diagram types

`info`, `pie`, `xychart`, `quadrantChart`, `radar`, `sankey`, `treemap`, `gitGraph`, `packet`, `treeView`, `mindmap`, `railroad`, `venn`, `ishikawa`, `gantt`, `journey`, `kanban`, `timeline`, `classDiagram`, `flowchart`, `stateDiagram`, `erDiagram`, `requirementDiagram`, `sequenceDiagram`, `cynefin`, `wardley`, `eventmodeling`, `C4`, `architecture`, `block`, `agentflow`, `usecase` and `swimlane`.

## API

| Item | What it does |
| --- | --- |
| `render(source, &Options)` | A `Drawing`: `svg`, `width`, `height`, `kind` and `links` |
| `parse` | A `Parsed`, whose `model` serializes as Pele's `parse` does |
| `render_parsed` | Draws a `Parsed` |
| `detect_type`, `supports` | The type a source names, and whether this build draws it |

`Options` takes Pele's render options. The host supplies:

| Field | What it is |
| --- | --- |
| `measure` | Text widths, as a `Measure`. `Metrics` (Arial's advances) when unset. |
| `icons` | An `IconResolver` |
| `colors` | `Colors`, written in place of every CSS variable, for renderers without custom properties |
| `now` | The time gantt charts treat as now |

Errors carry Pele's messages, a `Code`, and the line, column and snippet.

## Output

The SVG matches Pele's, with class names, CSS variables and ids prefixed `bezel` instead of `pele`. It does not match mermaid.js's output.

## Not supported

- Incremental layout: a changed source is drawn again from the start.

## Tests

```sh
cargo nextest run -p bezel-mermaid
```

`tests/mermaid/compat/` holds Pele's `parse` output for each type, and `tests/mermaid/corpus/` Mermaid's documentation examples. `MERMAID_PELE_SVG=<dir>` compares every drawing against Pele's SVG, written by `node tests/mermaid/pele/render.mjs svg <dir>`.

## Benchmark against Pele

```sh
npm --prefix crates/mermaid/tests/mermaid/pele run bench
```

Times Pele in Node and this crate at `opt-level = 3` (`cargo bench`) and with `--profile release`, on Pele's benchmark cases and the corpus. It prints each case side by side and the geometric mean.
