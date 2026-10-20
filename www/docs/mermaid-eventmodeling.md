---
title: Event modeling
description: An event model: screens, commands and events along a timeline.
prompt: Draw an event model of adding an item to the cart — the UI, the command and the event on a timeline.
---

```mermaid
eventmodeling

timeframe 01 ui CartUI
timeframe 02 command AddItem
timeframe 03 event ItemAdded

resetframe 04 event External.InventoryChanged
timeframe 05 processor InventoryProcessor
timeframe 06 command ChangeInventory
timeframe 07 event Cart.InventoryChanged
```

## Syntax

- `timeframe n kind Name` (or `tf`) puts an element on the timeline: `ui`, `command` (`cmd`), `event` (`evt`) or `processor`.
- `[[Data]]` after an element points at a `data Name { … }` block.
