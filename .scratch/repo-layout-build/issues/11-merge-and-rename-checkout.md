# 11: Merge and rename the checkout (HITL)

**What to build:** The new layout is on `main` and the checkout folder is named for the project.

**Blocked by:** 10

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Layout branch merged to `main` (revert commit, not rewritten history, is the restore path afterwards)
- [ ] With Claude Code closed, the checkout becomes `~/Documents/code/datalink-mp/datalink-mp` and the matching `~/.claude/projects/` key directory is moved; the one absolute path in any surviving capture note is updated
- [ ] `git status` is clean, `git worktree list` shows the single new path, build and test pass, and a new Claude Code session in the new folder shows the old history
