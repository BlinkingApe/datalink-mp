# 03: Clean up branches and worktrees

**What to build:** The repo shows only live branches and no stale worktrees, and every branch name that ADR-0002 cites still resolves through a tag.

**Blocked by:** 02

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Tags `archive/research-<name>` exist and are pushed for the three research branches ADR-0002 names
- [ ] All 9 worktrees are removed and `git worktree prune` clears the stale `smac-helper-gui` entries
- [ ] The 15 merged local branches (`prototype/ui-flow`, four `research/*`, ten `worktree-agent-*`) are deleted
- [ ] The 5 matching `origin/` branches are deleted only after the maintainer confirms
- [ ] `capture/traffic-capture` is untouched here (build 02 of internet-play-speed-build deals with it)
