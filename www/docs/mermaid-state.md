---
title: State
description: States and the transitions between them, with composite states and concurrent regions.
prompt: Draw a state diagram of the fetch lifecycle — idle, loading, ready and failed, with retry.
---

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Loading : fetch
    Loading --> Ready : resolved
    Loading --> Failed : rejected
    Failed --> Loading : retry
    state Ready {
        [*] --> Fresh
        Fresh --> Stale : timeout
        Stale --> Fresh : refresh
    }
    Ready --> [*]
```

## Syntax

- `A --> B : event` is a transition; `[*]` is the start or the end.
- `state Name { … }` nests states; `--` inside one splits it into concurrent regions.
- `state x <<choice>>`, `<<fork>>` and `<<join>>` add pseudo-states.
- `note left of` or `right of` a state adds a note; `direction` sets the layout.
