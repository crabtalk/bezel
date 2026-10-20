---
title: Sequence
description: Messages between participants over time, with activations, notes and numbering.
prompt: Draw a sequence diagram of placing an order — customer, web app, API gateway and bank.
---

```mermaid
sequenceDiagram
  autonumber
  actor Customer
  participant Web as Web app
  participant API as API gateway
  participant Bank
  Customer->>Web: Place order
  Web->>API: POST /orders
  activate API
  API->>Bank: Authorise payment
  Bank-->>API: Approved
  API-->>Web: 201 Created
  deactivate API
  Web-->>Customer: Order confirmed
  Note over Customer,Bank: One order, one transaction
```

## Syntax

- `participant` and `actor` declare who takes part, in order; `as` gives one a label.
- `->>` is a solid arrow, `-->>` dashed, `-)` async, `-x` a cross.
- `activate` / `deactivate`, or `+` / `-` on an arrow, mark a lifeline busy.
- `Note left of`, `right of` or `over A,B` adds a note; `autonumber` numbers the messages.
- `loop`, `alt` / `else`, `opt`, `par`, `critical`, `break` and `rect` box a run of messages.
