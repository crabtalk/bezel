---
title: XY chart
description: Bars and lines over a category or numeric x axis.
prompt: Chart monthly revenue for the year as bars with a line over them.
---

```mermaid
xychart
    title "Sales Revenue"
    x-axis [jan, feb, mar, apr, may, jun, jul, aug, sep, oct, nov, dec]
    y-axis "Revenue (in $)" 4000 --> 11000
    bar [5000, 6000, 7500, 8200, 9500, 10500, 11000, 10200, 9200, 8500, 7000, 6000]
    line [5000, 6000, 7500, 8200, 9500, 10500, 11000, 10200, 9200, 8500, 7000, 6000]
```

## Syntax

- `title`, then `x-axis` as a list of categories or `min --> max`, and `y-axis` as a label and a range.
- `bar [ … ]` and `line [ … ]` add series, one value per x.
- `xychart horizontal` turns the chart on its side.
