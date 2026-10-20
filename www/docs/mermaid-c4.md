---
title: C4
description: The C4 model's people, systems and boundaries, and the relations between them.
prompt: Draw a C4 system context diagram for our banking app — the customer, the banking system and the external mail system.
---

```mermaid
C4Context
  title System context for internet banking
  Person(customer, "Banking customer", "A customer of the bank")
  Enterprise_Boundary(bank, "Bank") {
    System(banking, "Internet banking", "Lets customers view their accounts")
    System_Ext(mail, "E-mail system", "Microsoft Exchange")
  }
  Rel(customer, banking, "Uses")
  Rel(banking, mail, "Sends e-mail", "SMTP")
```

## Syntax

- `C4Context`, `C4Container`, `C4Component`, `C4Dynamic` and `C4Deployment` are the five levels.
- `Person`, `System`, `Container`, `Component` and their `_Ext`, `Db` and `Queue` forms declare elements as `(alias, label, description)`.
- `Enterprise_Boundary`, `System_Boundary` and `Boundary` group elements in `{ … }`.
- `Rel(from, to, label, technology)` relates two; `BiRel`, `Rel_U`, `Rel_D`, `Rel_L` and `Rel_R` are its variants.
