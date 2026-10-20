---
title: Requirement
description: Requirements and the elements that satisfy, verify or trace them.
prompt: Draw a requirement diagram showing the login requirement and the test that verifies it.
---

```mermaid
requirementDiagram
    requirement test_req {
    id: 1
    text: the test text.
    risk: high
    verifymethod: test
    }

    element test_entity {
    type: simulation
    }

    test_entity - satisfies -> test_req
```

## Syntax

- `requirement name { … }` takes `id`, `text`, `risk` (`low`, `medium`, `high`) and `verifymethod` (`analysis`, `inspection`, `test`, `demonstration`).
- `functionalRequirement`, `performanceRequirement`, `interfaceRequirement`, `physicalRequirement` and `designConstraint` are the other kinds.
- `element name { type: …, docref: … }` is what meets one.
- `a - satisfies -> b` relates them; the others are `contains`, `copies`, `derives`, `verifies`, `refines` and `traces`.
