---
title: Mindmap
description: A tree of ideas around a root, its shape set by indentation.
prompt: Draw a mindmap of the project's goals, with each goal's tasks under it.
---

```mermaid
mindmap
  root((mindmap))
    Origins
      Long history
      Popularisation
        British popular psychology author Tony Buzan
    Research
      On effectiveness<br/>and features
      On Automatic creation
        Uses
            Creative techniques
            Strategic planning
            Argument mapping
    Tools
      Pen and paper
      Mermaid
```

## Syntax

- Indentation is the tree; the first node is the root.
- `id((text))`, `id(text)`, `id[text]`, `id))text((` and `id{{text}}` pick a node's shape.
- `::icon(…)` and `:::class` on the next line decorate the node above.
