# 07: Rename the local checkout folder

Type: grilling
Status: resolved
Blocked by: 01

## Question

The maintainer's checkout is `~/Documents/code/datalink-mp/smac-iroh`, a legacy name from upstream. What is the new path? For example, rename it to `datalink-mp/datalink-mp`, or flatten it to `~/Documents/code/datalink-mp`. And what has to follow the rename?

- **Worktrees:** `.git/worktrees/*` stores absolute paths, and two worktrees already point at an older `smac-helper-gui` path.
- **Claude Code's project memory:** `~/.claude/projects/` is keyed by the folder's path.
- IDE workspace settings, shell history and scripts with absolute paths (for example the Whisky launch notes).

Decide the order: after the branch and worktree cleanup, so fewer worktrees need repairing.

## Answer

**New path: `~/Documents/code/datalink-mp/datalink-mp`.** Only the last segment changes: `smac-iroh` becomes `datalink-mp`, matching the GitHub repo. The outer `datalink-mp/` stays as a container. Its stray `second.log` is not tracked anywhere and is the maintainer's to delete.

**Order: last.** The rename is the final step of the whole repo-layout build, after ticket 08's branch and worktree cleanup, the folder moves and the crate renames. By then all nine worktrees are gone, so there is nothing to repair. Run `git worktree prune` as part of that cleanup, which clears the two stale entries under the old `smac-helper-gui` path. If any worktree survives, run `git worktree repair` from the renamed checkout.

**What follows the rename** (found by searching the repo and `~/.claude`):

1. **Claude Code project memory.** With Claude Code closed, `mv ~/.claude/projects/-home-jct-Documents-code-datalink-mp-smac-iroh ~/.claude/projects/-home-jct-Documents-code-datalink-mp-datalink-mp`. Memory and `/resume` history then follow. The older orphaned keys (`smac-helper-gui`, `smac-helper-gui-smac-iroh`, `SMAC`) are left alone.
2. **`docs/capture_test_launch.txt`** (untracked) has `cd /home/jct/Documents/code/datalink-mp/smac-iroh` on line 1. Update it, or archive it with the rest of the capture material.
3. **`~/.claude/settings*.json`** and `.claude/` hold no path reference. The Faugus prefix and game paths in archived tickets are unrelated to the checkout, so they stay as historical record.
4. **Tracked files** hold no absolute checkout path. The `smac-iroh` hits in README, ADRs, research and archived tickets refer to the upstream repo and need no change. `.claude/` is excluded through `.git/info/exclude`.
5. **Not affected:** the `upstream` remote (`hdevalence/smac-iroh`, kept by ADR-0003), `origin`, and `target/` (gitignored, rebuilt on demand; `cargo clean` is optional because Cargo's fingerprints use relative paths).
6. IDE workspace files and shell history: none exist in the repo. The maintainer reopens the editor on the new folder.

**Proof it worked:** `git status` is clean and `git worktree list` shows the single new path. `cargo build` and `cargo test` pass. A new Claude Code session in the new folder shows the old memory.
