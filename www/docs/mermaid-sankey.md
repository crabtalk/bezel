---
title: Sankey
description: Flows between nodes, each as wide as its value. The source is CSV rows.
prompt: Draw a Sankey diagram of where our energy comes from and where it goes.
---

```mermaid
sankey

Solar,Electricity,60
Wind,Electricity,45
Gas,Electricity,30
Gas,Heating,50
Electricity,Homes,70
Electricity,Industry,65
Heating,Homes,50
```

## Syntax

- After the header, each line is `source,target,value` — CSV, with quotes around a name holding a comma.
