# 08: Branch and worktree policy

Type: grilling
Status: resolved
Blocked by: 03

## Question

The [ledger](../ledger.md) marks 15 branches and all 9 worktrees *not necessary*, and 5 unmerged branches *necessary*. What happens to each?

- **Merged and landed branches** (`prototype/ui-flow`, four `research/*` with their `origin/` copies, ten `worktree-agent-*`): delete locally and on `origin`? ADR-0002 names three of the research branches, and archived tickets name branches and commits. Do those names need to keep resolving (a tag, say), or is a commit hash reachable from `main` enough?
- **Unmerged, unpushed branches:** `capture/traffic-capture` merges through internet-play-speed-build 02. For `prototype/turn-sync-activity`, `research/iroh-connection-stats`, `research/smac-jackal-turn-sync` and `research/repo-layout-conventions`, do their docs merge into `main` (as the four older research write-ups did), get pushed and kept, or get tagged? This also fixes where tickets' "Research: branch …" pointers lead.
- **Worktrees:** remove all nine, and prune the two that point at the old `smac-helper-gui` path?
- **The convention going forward:** should `research/<name>` and `prototype/<name>` branches keep being made and left unmerged, or should findings land on `main` when the ticket closes?

Blocked by [How to archive what isn't necessary](03-how-to-archive.md), since tags and archive folders decide where a deleted branch's content stays reachable.

## Answer

Decided with the maintainer, 2026-10-06. They accepted every recommendation. [How to archive what isn't necessary](03-how-to-archive.md) already settled the merged branches, so only the open points are new here.

- **Already decided in 03, unchanged:** prune all 9 worktrees (including the two that point at the old `smac-helper-gui` path). Delete the 15 merged local branches (`prototype/ui-flow`, 4 `research/*`, 10 `worktree-agent-*`) and the 5 `origin/` copies, the `origin/` deletion as a maintainer-confirmed step. Tag the tips of the three `research/*` branches ADR-0002 names (`archive/research-<name>`) first. The other merged branches get no tag, and a commit hash reachable from `main` is enough for them.
- **Unmerged research branches** (`research/iroh-connection-stats`, `research/smac-jackal-turn-sync`, `research/repo-layout-conventions`): each write-up merges into `main` as the four older ones did, before the layout moves start. Then the branch and its worktree are deleted. After the merge the write-ups are ordinary docs: the layout build files them like any evidence doc (`docs/archive/`, with a row in its index), and `docs/traffic-capture.md`'s link to the jackal write-up is rewritten with them.
- **`prototype/turn-sync-activity`:** prototype code never merges. Tag its tip `archive/prototype-turn-sync-activity`, push the tag, then delete the branch and worktree. Build ticket 12 of internet-play-speed-build cites it, so its pointer changes from the branch name to the tag.
- **`capture/traffic-capture`:** untouched here. It merges through internet-play-speed-build 02 and is deleted after.
- **Convention going forward:** when a research ticket closes, its write-up lands on `main` and the branch is deleted. When a prototype ticket closes, the tip is tagged `archive/prototype-<name>`, pushed, and the branch is deleted. Ticket pointers then read "Research: `docs/...`" or "Prototype: tag `archive/prototype-<name>`", never a branch name. `docs/agents/issue-tracker.md` records it, beside the finished-effort rule from 03.
- **Build ordering this implies:** first merge the three research docs to `main` and tag the prototype, then take the `pre-layout` tag, then prune worktrees and delete branches. Pushing `main` (7 commits ahead of `origin`) is the maintainer's call, outside the layout build.
