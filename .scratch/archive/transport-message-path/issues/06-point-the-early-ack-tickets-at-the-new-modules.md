# 06: Point the Early ack tickets at the new modules

**What to build:** The Early ack tickets in [internet-play-speed-build](../../../internet-play-speed-build/issues/) describe what was actually built, so the next agent builds inside the new modules instead of adding hooks again.

- **Tickets 07–10:** rewrite the "where it lives" and hook notes against the friend pipeline, the game inbox and the frame sink, following the spec's [Effect on the Early ack tickets](../spec.md#effect-on-the-early-ack-tickets) table and correcting it wherever the build differed.
- **Ticket 08:** its reset-stream test uses the in-memory frame sink, and it no longer depends on ticket 10.
- **Ticket 17 (release Patch B):** add the earlier-reconnect line from [02](02-act-on-a-lost-connection-when-it-happens.md) to its release notes.
- **Close this effort** under the repo's finished-effort convention once every ticket here is resolved.

**Blocked by:** 01, 03, 05

**Status:** resolved

- [x] Tickets 07–10 name the new modules and contain no stale line numbers
- [x] Ticket 08's blocking no longer implies needing ticket 10
- [x] Ticket 17 carries the earlier-reconnect release note
- [x] `scripts/check-links.py` reports no new broken links
