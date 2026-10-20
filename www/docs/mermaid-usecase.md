---
title: Use case
description: Actors and the use cases they take part in, inside a system boundary.
prompt: Draw a use case diagram of the order system with the customer and clerk actors.
---

```mermaid
usecase-beta
direction LR
actor Customer("Customer")
actor Clerk("Clerk")
systemBoundary "Order system"
  Browse("Browse catalogue")
  Checkout("Place order")
  Refund("Issue refund")
end
Customer --> Browse
Customer --> Checkout
Clerk --> Refund
```

## Syntax

- `actor Name` declares an actor; `id("label")` a use case.
- `systemBoundary "name" … end` puts use cases inside a system.
- `a --> b` links an actor to a use case.
- `direction` sets the layout; `classDef`, `class` and `style` restyle.
