---
title: Gantt
description: Tasks on a calendar, in sections, each after another or from a date.
prompt: Draw a Gantt chart of the release plan — design for two weeks, then build after it, with QA as a milestone.
---

```mermaid
gantt
    title A Gantt Diagram
    dateFormat YYYY-MM-DD
    section Section
        A task          :a1, 2014-01-01, 30d
        Another task    :after a1, 20d
    section Another
        Task in Another :2014-01-12, 12d
        another task    :24d
```

## Syntax

- `dateFormat` is how dates in the source are read; `axisFormat` how the axis prints them.
- `section` starts a group; `Task : id, start, length` is a task.
- A start is a date or `after id`; a length a duration such as `30d`, or an end date.
- `done`, `active`, `crit` and `milestone` before the id mark a task.
