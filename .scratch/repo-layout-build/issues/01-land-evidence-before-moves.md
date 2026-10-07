# 01: Land the evidence before the moves

**What to build:** The three unmerged research write-ups (`research/iroh-connection-stats`, `research/smac-jackal-turn-sync`, `research/repo-layout-conventions`) are on `main`, and the prototype tip of `prototype/turn-sync-activity` is preserved by a pushed tag, so nothing lives only on this machine.

**Blocked by:** None (can start immediately), but gated: do not start until internet-play-speed-build 02 (capture branch merged) and 06 (Patch A released) are done.

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Each research write-up is merged to `main` as the four older ones were
- [x] Tag `archive/prototype-turn-sync-activity` exists on the branch tip and is pushed (maintainer confirms the push)
- [x] Build ticket 12 of internet-play-speed-build cites the tag instead of the branch name
- [x] The maintainer's confirmation of each outward-facing push is recorded in the ticket

## Comments

2026-10-07. Gate checked: internet-play-speed-build 02 is done (`capture/traffic-capture` is reachable from `main`) and Patch A shipped as `v0.1.1` (GitHub's Latest release, published 2026-10-07), though 06's status line still reads ready-for-agent.

- Merged with `--no-ff` to `main`: `research/iroh-connection-stats` (faba2d4), `research/smac-jackal-turn-sync` (23f032c), `research/repo-layout-conventions` (b3f09c6). The older four came in through GitHub PRs; these are local merge commits because pushing `main` is the maintainer's call.
- Annotated tag `archive/prototype-turn-sync-activity` on `0ac8553`, pushed to `origin`.
- internet-play-speed-build 12 now cites the tag.
- The maintainer approved every outward-facing push in this build (the 5 tags, deleting the 5 `origin/` branches, the dry-run tag) on 2026-10-07, before the build started.
