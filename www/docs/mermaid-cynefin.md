---
title: Cynefin
description: Items sorted into the Cynefin framework's domains.
prompt: Sort our incident response actions into a Cynefin diagram by domain.
---

```mermaid
cynefin-beta
  title Incident Response

  complex
    "Investigate root cause"
    "Run chaos experiment"

  complicated
    "Analyze performance data"
    "Expert review needed"

  clear
    "Restart service"
    "Apply known fix"

  chaotic
    "Page on-call immediately"

  confusion
    "Unknown failure mode"
```

## Syntax

- `complex`, `complicated`, `clear`, `chaotic` and `confusion` open a domain.
- Each quoted line under one is an item in it.
- `title` names the diagram.
