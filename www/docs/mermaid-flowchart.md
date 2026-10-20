---
title: Flowchart
description: Nodes and edges, laid out in layers. Subgraphs group nodes; the header names the direction.
prompt: Draw a left-to-right flowchart of our client, services and storage, grouped into subgraphs.
---

```mermaid
flowchart LR
  subgraph Client
    UI[Web app]
    Cache[(Local cache)]
  end
  subgraph Services
    API[API gateway]
    Auth[Auth service]
    Orders[Order service]
  end
  subgraph Storage
    DB[(Orders DB)]
  end
  UI --> API
  UI --> Cache
  API --> Auth
  API --> Orders
  Orders --> DB
  Auth -. token .-> UI
```

## Syntax

- `flowchart` (or `graph`) and a direction: `TB`, `TD`, `BT`, `LR` or `RL`.
- `A[text]` is a node; the brackets pick the shape — `(round)`, `([stadium])`, `[(database)]`, `((circle))`, `{decision}`, `{{hexagon}}`.
- `-->` is an arrow, `---` a line, `-.->` dotted, `==>` thick; `-- text -->` or `-->|text|` labels it.
- `subgraph name … end` groups nodes, and `direction` inside one sets its own.
- `classDef`, `class` and `style` restyle nodes.
