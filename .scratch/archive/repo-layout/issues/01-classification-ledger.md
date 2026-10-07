# 01: Build the classification ledger

Type: task
Status: resolved
Blocked by:

## Question

Every later decision on this map works from a full inventory. Build it as `.scratch/repo-layout/ledger.md`, a table with one row per:

- tracked file (group a folder into one row only when every file in it shares a verdict and a reason, such as a crate's `src/`)
- untracked file in the working copy (not gitignored)
- local branch, remote branch (`origin`, `upstream`) and worktree

Each row gives:

- **Verdict:** *necessary* or *not necessary*, under the rule in the map's Notes (builds, tests or ships it / a living doc / an active effort's planning)
- **Reason:** which of the three it meets, or why none
- **Confidence:** *confident* or *unsure*. Unsure rows need the maintainer, for example untracked docs, past RC checklists, or whether a `.scratch` effort is still active.
- **Inbound references:** every place that names this path, such as `Cargo.toml`, `build.rs`, `.github/workflows/release.yml`, `scripts/`, README, docs, ADRs, `CLAUDE.md`, `docs/agents/`, and other `.scratch` files. These are what a move would break.
- For branches and worktrees: merged into `main` or not, last commit date, and which ticket or effort made it.

End with a summary by category, and the list of unsure rows as questions for the maintainer. The task is done when the maintainer has answered them and the ledger records the answers.

## Answer

Resolved 2026-10-06. The ledger is [`ledger.md`](../ledger.md): every tracked and untracked path, the 20 local branches, the remote branches and the 9 worktrees, each with a verdict, reason, confidence and inbound references. The maintainer answered all five unsure rows, each as the agent leaned, and the ledger records the answers.

- **Not necessary:** `crates/smac-helper/` (prototype only), `tools/0001-wtp-through-the-ages.html`, the four merged `docs/research/*.md` write-ups, the 0.1.0 and rc.6 checklists, the rc4 notes, `docs/capture_test_launch.txt`; the `.scratch` efforts release-pipeline, ui-polish-0.1.1 and both archives; every merged or landed branch (`prototype/ui-flow`, four `research/*` with their `origin/` copies, ten `worktree-agent-*`); all nine worktrees, which are clean.
- **Necessary beyond the obvious code, build and living docs:** the captured-game checklist (reused by internet-play-speed-build 15, 16); the `.scratch` efforts repo-layout, internet-play-speed-build, internet-play-speed (the build effort's spec), post-0.1.0-polish and game-session-sync (both live); the unmerged branches `capture/traffic-capture`, `prototype/turn-sync-activity` and three `research/*` whose docs exist nowhere else.
- **Found along the way:** those five unmerged branches aren't pushed, and local `main` is 7 commits ahead of `origin/main`. The paths hard-coded in builds and CI are listed under the ledger's summary.
