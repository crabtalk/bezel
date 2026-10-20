---
title: Kanban
description: Columns of task cards.
prompt: Show this sprint's tasks as a kanban diagram with To do, In progress and Done columns.
---

```mermaid
kanban
  todo[To do]
    spec[Write the spec]
    tests[Add tests]
  doing[In progress]
    parser[Port the parser]
  done[Done]
    layout[Layered layout]
```

## Syntax

- An unindented `id[Title]` is a column; each indented one under it is a card.
- `@{ assigned: …, ticket: …, priority: … }` after a card adds its metadata.
