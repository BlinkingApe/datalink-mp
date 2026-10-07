# 03: Clean up branches and worktrees

**What to build:** The repo shows only live branches and no stale worktrees, and every branch name that ADR-0002 cites still resolves through a tag.

**Blocked by:** 02

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Tags `archive/research-<name>` exist and are pushed for the three research branches ADR-0002 names
- [x] All 9 worktrees are removed and `git worktree prune` clears the stale `smac-helper-gui` entries
- [x] The 15 merged local branches (`prototype/ui-flow`, four `research/*`, ten `worktree-agent-*`) are deleted
- [x] The 5 matching `origin/` branches are deleted only after the maintainer confirms
- [x] `capture/traffic-capture` is untouched here (build 02 of internet-play-speed-build deals with it)

## Comments

2026-10-07. Done against the repo as it stood, which had moved on since the ledger:

- Tags `archive/research-windows-toolchain`, `archive/research-macos-artifact`, `archive/research-av-risk` (annotated, on each branch tip) are pushed.
- 12 worktrees removed, not 9: the ledger's 9 plus 3 made by internet-play-speed-build (`agent-a0e1f88…`, `agent-a4d291f…`, `agent-acf449d…`), all clean and merged. The two `smac-helper-gui` entries pointed at a path that no longer existed, but their folders were still under `.claude/worktrees/` (~10 GB, mostly `target/`). Their tracked files matched their merged branch tips, so the folders were deleted and `git worktree prune` cleared the entries. `git worktree list` shows only the main checkout.
- 21 local branches deleted: the ledger's 15 (`prototype/ui-flow`, four `research/*`, ten `worktree-agent-*`), the 3 new merged `worktree-agent-*` branches above, and the 3 `research/*` branches 01 merged. `worktree-agent-a1e1b2…` and `-acb13c…` needed `-D`; `git range-diff` and `git cherry` show their commits landed on `main` as `d3810c7` and an equivalent commit.
- `origin/` copies deleted with the maintainer's approval: `prototype/ui-flow`, `research/{av-risk,macos-artifact,transport-restart,windows-toolchain}`.
- Left alone: `capture/traffic-capture` (now merged; internet-play-speed-build 02 deletes it) and `prototype/turn-sync-activity` (tagged in 01; the branch is the live unmerged work the spec keeps).
