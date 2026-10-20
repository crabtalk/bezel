---
title: Quadrant
description: Points placed on two axes split into four named quadrants.
prompt: Plot our feature ideas on a quadrant chart of effort against impact.
---

```mermaid
quadrantChart
    title Reach and engagement of campaigns
    x-axis Low Reach --> High Reach
    y-axis Low Engagement --> High Engagement
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.23]
    Campaign C: [0.57, 0.69]
    Campaign D: [0.78, 0.34]
    Campaign E: [0.40, 0.34]
    Campaign F: [0.35, 0.78]
```

## Syntax

- `x-axis Low --> High` and `y-axis Low --> High` label the axes.
- `quadrant-1` to `quadrant-4` name the quadrants, from the top right anticlockwise.
- `Name: [x, y]` places a point, both between 0 and 1.
