---
title: Railroad
description: A grammar drawn as railroad tracks, from EBNF, PEG or combinator rules.
prompt: Draw our arithmetic expression grammar as a railroad diagram from its EBNF.
---

```mermaid
railroad-ebnf-beta
title "Arithmetic Expression Grammar"

expression = term ( ( "+" | "-" ) term )* ;
term = factor ( ( "*" | "/" ) factor )* ;
factor = number | "(" expression ")" ;
number = digit+ ;
digit = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" ;
```

## Syntax

- `railroad-ebnf-beta` reads EBNF: `rule = … ;` with `|` for choice, `( )` to group, and `*`, `+` and `?` to repeat.
- `railroad-peg-beta` reads PEG: `rule <- … ;` with `/` for choice.
- `railroad-beta` reads combinators: `sequence(…)`, `choice(…)`, `zeroOrMore(…)`, `oneOrMore(…)`, `terminal("…")` and `nonterminal("…")`.
- `title` names the diagram; each rule is a track.
