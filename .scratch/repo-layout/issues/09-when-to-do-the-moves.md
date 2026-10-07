# 09: When to do the moves

Type: grilling
Status: resolved
Blocked by: 05, 06, 08

## Question

Does the layout build run before, during or after `internet-play-speed-build`? That effort has 18 open tickets (01–06 Patch A, 07–12 and 17 Patch B, 13–16 and 18 HITL validation) and the unmerged `capture/traffic-capture` branch (8 commits, holds the tickets themselves). They cite `crates/iroh-transport`, `crates/dplayx`, `docs/traffic-capture.md`, `analyse_capture.py` and ADR-0005, all of which the target tree or the crate renames move. If it runs mid-effort, who rewrites the tickets' paths, and does the build split into a docs/packaging half and a crate-rename half?

## Answer

Decided with the maintainer, 2026-10-06. They accepted the recommendation: the layout build runs **between Patch A and the code tickets**.

- **Before the layout build:** [internet-play-speed-build](../../internet-play-speed-build/) 01–06. Ticket 02 merges `capture/traffic-capture` into `main` and the branch goes. Ticket 06 releases Patch A, so testers get the measurement patch with no delay. Patch A is Helper-only and small, so the old paths cost little.
- **Layout build:** the whole of it in one run, crate renames and checkout-folder rename included. The `pre-layout` tag is taken at the Patch A release commit (after 08's branch merges, as 08 orders).
- **After the layout build:** build tickets 07–12 (the code-heavy early acks, `dev-faults`, fast wake, page activity), 13–16 (HITL validation), 17 (Patch B) and 18. No code is in flight during the renames, so nothing needs rebasing.
- **Ticket paths:** the layout build rewrites the paths and crate names in every still-open build ticket (and ADR-0005 and `docs/traffic-capture.md`) in the same commit as the moves. That is one step in the move list, covered by the link check in the proof step.
- **Branch and worktree cleanup:** happens at the start of the layout build, as 08 orders. It needs the capture branch gone, so it also waits for build 02.
- **Checkout folder rename:** still last (07). Any open build session should be closed first.
- **Archiving `.scratch/internet-play-speed/`:** stays a note on the build effort, done when it finishes (03), not part of the layout build.
- **If Patch A slips:** the layout build must not start until build 02 is merged and 06 is released. If 06 stalls for long, the maintainer can still choose to run the layout build without it, but that is a new decision, not this one.
