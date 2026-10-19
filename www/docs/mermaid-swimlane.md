---
title: Swimlane
description: A flowchart whose subgraphs are lanes, one per party.
---

```mermaid
swimlane-beta LR
  subgraph Customer
    Browse[Browse catalogue]
    Pay[Pay]
  end
  subgraph Warehouse
    Pick[Pick items]
    Ship[Ship order]
  end
  subgraph Finance
    Invoice[Raise invoice]
  end
  Browse --> Pay
  Pay --> Pick
  Pick --> Ship
  Pay --> Invoice
```

## Syntax

- A flowchart's syntax, after `swimlane-beta` and a direction.
- Each top-level `subgraph … end` is a lane; its nodes stay in it.
- Edges cross lanes freely.
