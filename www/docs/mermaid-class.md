---
title: Class
description: Classes with their members, and the relations between them.
prompt: Draw a class diagram of the Animal hierarchy with Duck, Fish and Zebra and their fields and methods.
---

```mermaid
classDiagram
    Animal <|-- Duck
    Animal <|-- Fish
    Animal <|-- Zebra
    Animal : +int age
    Animal : +String gender
    Animal: +isMammal()
    Animal: +mate()
    class Duck{
        +String beakColor
        +swim()
        +quack()
    }
    class Fish{
        -int sizeInFeet
        -canEat()
    }
    class Zebra{
        +bool is_wild
        +run()
    }
```

## Syntax

- `class Name { … }` lists members; `+`, `-`, `#` and `~` set visibility, and `()` makes a method.
- `Name : +member` adds one member outside the block.
- `<|--` is inheritance, `*--` composition, `o--` aggregation, `-->` association, `..>` dependency, `..|>` realization.
- `: text` after a relation labels it; quoted cardinalities sit at either end.
- `namespace Name { … }` groups classes; `note` and `note for Name` add notes.
