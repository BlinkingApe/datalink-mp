# Map: Post-0.1.0 UI/UX polish

Label: wayfinder:map

## Destination

A scoped, decided plan for the first post-0.1.0 polish round: which UI/UX ideas from the 0.1.0 gate testing ship, and — for the one with an open design question — how it works. Hands off to `/to-tickets` once this version actually starts; the ideas aren't blocking 0.1.0 and nothing here is built now.

## Notes

- Domain: `CONTEXT.md` (Helper, Game session, Ticket) and the helper-web-ui spec's "Session model" (the `ready`/`joining`/`joined`/`hosting` state table).
- Tracker: local markdown, this directory.
- Pulled out of helper-web-ui: tickets 20, 22 and 24 originated there (`../archive/helper-web-ui/issues/`) and carry over here unchanged, already fully specified — nothing left to decide, just relocated out of the v1 spec since they're not blockers. Ticket 21 is also pulled out, but stays unticketed fog (see Not yet specified) rather than becoming a decision ticket yet. Ticket 23 (the second-Helper-persists defect) was considered for this move and kept out: it's a regression against already-implemented behavior, not a deferred idea, so it stays in helper-web-ui as a near-term defect.
- Skills for grilling tickets: `grilling` + `domain-modeling`.

### Decided while charting

- **20, 22 and 24 need no further decision.** Each is already fully specified; they carry over as settled scope, unordered relative to each other (ordering is an implementation-time call):
  - [Show the host's connected-peer count earlier](issues/01-show-friends-connected-earlier.md) (was helper-web-ui ticket 20): surface the peer count at step 2, not just step 4.
  - [Document that Simultaneous Moves needs Joiners to click Make Ready](issues/02-document-make-ready-for-simultaneous-moves.md) (was helper-web-ui ticket 22): a line on the page or in `datalink-mp-README.txt`.
  - [Clarify the quarantine sentence in the not_game_folder banner](issues/03-quarantine-banner-wording.md) (was helper-web-ui ticket 24): say plainly that the folder is right and only the DLL is missing.
- **Term: Host is hostable**, for the condition ticket 21 is about (the Host's game has reached the point where it can accept a Join). Avoids colliding with the Helper's own `ready` connection-state value, which means something else (no peers, no dial in progress).
- **Ticket 21's design question stays unanswered for now.** The automatic-signal-vs-manual-signal choice depends on what the Helper-to-Helper status channel looks like once the [game-session-sync map](../game-session-sync/map.md)'s work lands — deciding today risks deciding against a foundation that's about to move.

## Decisions so far

<!-- one line per closed ticket -->

None yet — this map was charted by relocating existing, already-specified ideas (see "Decided while charting") rather than by resolving open tickets.

## Not yet specified

- **Tell Joiners when the Host is hostable** (parked reference: [issues/04-signal-joiners-when-host-is-hostable.md](issues/04-signal-joiners-when-host-is-hostable.md), carried over from helper-web-ui ticket 21). Two angles were on the table — an automatic Helper-to-Helper signal once the Host's game is actually hostable, versus a manual host-side signal, versus just rewording the Joiner's step 3 to say "wait for your friend" — none chosen yet. Blocked, informally, on the [game-session-sync map](../game-session-sync/map.md): decide this only after that map's shape of Helper/Game-session status is settled, since the signal this ticket wants may ride on the same channel.

## Out of scope

