# 01: Land the evidence before the moves

**What to build:** The three unmerged research write-ups (`research/iroh-connection-stats`, `research/smac-jackal-turn-sync`, `research/repo-layout-conventions`) are on `main`, and the prototype tip of `prototype/turn-sync-activity` is preserved by a pushed tag, so nothing lives only on this machine.

**Blocked by:** None (can start immediately), but gated: do not start until internet-play-speed-build 02 (capture branch merged) and 06 (Patch A released) are done.

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Each research write-up is merged to `main` as the four older ones were
- [ ] Tag `archive/prototype-turn-sync-activity` exists on the branch tip and is pushed (maintainer confirms the push)
- [ ] Build ticket 12 of internet-play-speed-build cites the tag instead of the branch name
- [ ] The maintainer's confirmation of each outward-facing push is recorded in the ticket
