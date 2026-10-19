---
title: Entity relationship
description: Entities, their attributes, and the cardinality of each relationship.
---

```mermaid
erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
    CUSTOMER }|..|{ DELIVERY-ADDRESS : uses
    CUSTOMER {
        string name
        string email
    }
    ORDER {
        int id
        date placed
    }
```

## Syntax

- `A ||--o{ B : label` relates two entities; each end's pair of marks is its cardinality — `|o` zero or one, `||` exactly one, `}o` zero or more, `}|` one or more.
- `--` is an identifying relationship, `..` a non-identifying one.
- `ENTITY { type name }` lists attributes; `PK`, `FK` and `UK` after one mark keys, and a quoted string comments it.
