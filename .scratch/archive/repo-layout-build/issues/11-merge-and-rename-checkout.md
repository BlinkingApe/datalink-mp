# 11: Merge and rename the checkout (HITL)

**What to build:** The new layout is on `main` and the checkout folder is named for the project.

**Blocked by:** 10

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Layout branch merged to `main` (revert commit, not rewritten history, is the restore path afterwards)
- [x] With Claude Code closed, the checkout becomes `~/Documents/code/datalink-mp/datalink-mp` and the matching `~/.claude/projects/` key directory is moved; the one absolute path in any surviving capture note is updated
- [x] `git status` is clean, `git worktree list` shows the single new path, build and test pass, and a new Claude Code session in the new folder shows the old history

## Comments

2026-10-07. `layout` is merged to `main` as 2aef11e, a `--no-ff` merge, with the maintainer's approval. To restore, revert that merge. `main` isn't pushed; that's the maintainer's call.

The rest needs Claude Code closed, so it's left for the maintainer:

```bash
# with every Claude Code session for this repo closed
cd ~/Documents/code/datalink-mp
mv smac-iroh datalink-mp
mv ~/.claude/projects/-home-jct-Documents-code-datalink-mp-smac-iroh \
   ~/.claude/projects/-home-jct-Documents-code-datalink-mp-datalink-mp
cd datalink-mp
git status            # clean, apart from the untracked "Before v0.1.1 can be released" note
git worktree list     # one line: .../datalink-mp/datalink-mp  [main]
cargo build --locked -p datalink-mp && cargo test --workspace
claude                # /resume should list this session's history
```

No capture note needs updating. The one absolute path to the old folder was in `docs/capture_test_launch.txt`, deleted in 05; the capture checklist and `traffic-capture.md` use repo-relative paths. Older orphaned project keys are left alone, as the spec says.

2026-10-07. The maintainer renamed the checkout and moved the project key. Verified from the new folder: one worktree at `.../datalink-mp/datalink-mp [main]`, `cargo build --locked -p datalink-mp` and `cargo test --workspace` pass, no old `smac-iroh` project key remains.
