---
title: Wardley map
description: A value chain plotted against evolution, with the moves planned for it.
prompt: Draw a Wardley map of the tea shop's value chain, with the kettle evolving to a commodity.
---

```mermaid
wardley-beta
title Tea Shop Value Chain

anchor Business [0.95, 0.63]
component Cup of Tea [0.79, 0.61]
component Tea [0.63, 0.81]
component Hot Water [0.52, 0.80]
component Kettle [0.43, 0.35]
component Power [0.10, 0.70]

Business -> Cup of Tea
Cup of Tea -> Tea
Cup of Tea -> Hot Water
Hot Water -> Kettle
Kettle -> Power

evolve Kettle 0.62
evolve Power 0.89
```

## Syntax

- `component Name [visibility, evolution]` places a component, both between 0 and 1; `anchor` places a user need.
- `a -> b` is a dependency.
- `evolve Name x` draws where a component is moving to.
- `note` and `title` annotate the map.
