---
title: Git graph
description: Commits, branches and merges, drawn as a history.
prompt: Draw a git graph showing a develop branch that gets two commits and merges back into main.
---

```mermaid
gitGraph
   commit
   commit
   branch develop
   checkout develop
   commit
   commit
   checkout main
   merge develop
   commit
   commit
```

## Syntax

- `commit` adds a commit to the current branch; `id:`, `tag:` and `type:` (`NORMAL`, `REVERSE`, `HIGHLIGHT`) describe it.
- `branch name` starts a branch and `checkout name` (or `switch`) moves to one.
- `merge name` merges a branch into the current one; `cherry-pick id:` copies a commit.
