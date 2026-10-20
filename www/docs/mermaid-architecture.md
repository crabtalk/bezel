---
title: Architecture
description: Services and groups with icons, joined at the side each edge leaves and enters.
prompt: Draw an architecture diagram of our API — a server, a database and two storage disks inside an API group.
---

```mermaid
architecture-beta
    group api(cloud)[API]

    service db(database)[Database] in api
    service disk1(disk)[Storage] in api
    service disk2(disk)[Storage] in api
    service server(server)[Server] in api

    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db
```

## Syntax

- `group id(icon)[Label]` and `service id(icon)[Label]` declare boxes; `in group` nests one.
- `a:L -- R:b` joins two services at a side each: `L`, `R`, `T` or `B`. `-->` and `<--` put an arrow at an end.
- `junction id` splits an edge four ways.
- `cloud`, `database`, `disk`, `internet` and `server` are the built-in icons.
