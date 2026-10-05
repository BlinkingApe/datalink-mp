# Map: Game session sync across exit, disconnect and Stop

Label: wayfinder:map

## Destination

A decision, per originating defect (release-pipeline 06, 08, 09 and 10), on whether the Helper must keep the game's **Game session** in sync with its own connection state across an ungraceful game exit, a silent peer disconnect, and Stop pressed mid-game — and a design for whichever of those turn out to need one. Any defect judged an acceptable consequence of proceeding past an existing warning (Stop's "return to the game's main menu first") closes here as out of scope, paired with a wording ticket so the warning says what actually happens. Hands off to implementation afterward; building is a separate effort.

## Notes

- Domain: Rust workspace; see `CONTEXT.md` (Helper, DLL, **Game session**, Ticket, IPC version, Peer protocol version) and the helper-web-ui spec's "Session model" and "Shared status and banners" sections — the currently accepted behavior this map may revise. `docs/adr/0001-web-ui-frontend.md` covers Stop's intent.
- Tracker: local markdown, this directory.
- Pulled out of release-pipeline: tickets 06, 08, 09 and 10 originated there (`../release-pipeline/issues/`), where each has the original repro detail and dated history; release-pipeline ticket 04 (the gate) now blocks on this map's resolution instead of on those tickets directly. Ticket 07 (Joiner's in-game name blank) was considered and left out: a data field not making it across doesn't obviously implicate session lifecycle.
- Standing preference: Stop mid-game already warns the player. Proceeding anyway may reasonably cost the Game session (the other player has to restart), but must not be allowed to crash the other player's game — that distinction decides what's "not our problem" here.
- Skills for grilling tickets: `grilling` + `domain-modeling`.

### Decided while charting

- **Scope is per-defect, not a single yes/no.** 06 (crash on ungraceful exit) and 09 (silent disconnect) aren't warned-against actions a player chose; 10 (Stop mid-game) partly is. One verdict for all four would force the same answer onto different situations.
- **Severity cuts across the warned/unwarned split.** Even for Stop (warned), a crash in the other player's game is in scope to at least understand; only "the session doesn't survive, no restart needed" is a candidate for "not our problem."
- **"Not our problem" still owes a wording fix.** Any defect closed as an accepted consequence of Stop's warning graduates a ticket to make that warning say concretely what can happen (crash or unresumable session), not just close silently.
- **Ticket 06 is split from the 08/09/10 investigation.** It already has a landed fix and a known cause; what's left is a real-game confirmation, a different kind of work from root-causing 08/09/10.
- **Term: Game session.** The DirectPlay-level session the game itself is in (created by Host Game, joined by Join Game, ended by CloseSession or a crash), tracked today in the IPC layer the DLL talks through — distinct from the Helper's own connection state (`ready`/`joining`/`joined`/`hosting`, driven by the Transport and its peers). Added to `CONTEXT.md`.

## Decisions so far

<!-- one line per closed ticket -->

- [A Joiner leaving tells the host's game its session was lost](issues/03-joiner-leaving-tells-the-host-its-session-was-lost.md): fixed in the Helper. Only the host's close ends a session (a Joiner's close sends `PlayerLeft`, and a received `SessionClosed` counts only from the host), and each Game session starts with an empty inbox. Real-game confirmation is folded into tickets 01 and 02, and done on `-rc.6`.
- [Confirm the leaked-Game-session fix in a real game](issues/01-confirm-ticket-06-fix-in-a-real-game.md): confirmed on `-rc.6`. A host whose game quit or was killed in Multiplayer Setup hosts again with no `Net::send` popup and working drop-downs.

## Not yet specified

- Whether one mechanism (e.g. the Helper telling the DLL/game a peer or the session ended) covers all of 08, 09 and 10, or each needs its own handling — depends on [Shared root cause across 08, 09 and 10](issues/02-shared-root-cause-across-08-09-10.md).
- The design itself, for whichever of 08/09/10 turn out to need one.
- The wording fix to the Stop confirmation, for whichever defects land as "not our problem."

## Out of scope

