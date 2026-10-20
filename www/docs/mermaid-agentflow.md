---
title: Agent flow
description: An agent's steps, tools and decisions, grouped into flows.
prompt: Draw our review agent's steps as an agent flow — gather changes, analyse the diff, run the linter tool, then decide if it's clean.
---

```mermaid
agentflow-beta TB
  flow reviewer["Review Agent"]
    changes["Gather changes"]@{ shape: input }
    analyse["Analyse diff"]@{ shape: task }
    lint["run_linter"]@{ shape: tool }
    ok["Clean?"]@{ shape: decision }

    changes --> analyse --> lint --> ok
  end
```

## Syntax

- `flow id["label"] … end` groups an agent's steps.
- `id["label"]@{ shape: … }` is a step: `input`, `task`, `tool` and `decision` among its shapes.
- `a --> b` orders steps, and chains as `a --> b --> c`.
