# Shared root cause across 08, 09 and 10, and a severity/warned classification

Type: task
Status:
Blocked by:

## Question

Three defects, originally release-pipeline tickets 08, 09 and 10, all surfaced during real multiplayer games and all look like the Game session (the DirectPlay-level session the game is in) falling out of sync with the Helper's actual connection state — the same shape as ticket 06's already-found cause (see [Confirm the leaked-Game-session fix in a real game](01-confirm-ticket-06-fix-in-a-real-game.md)), but reached by three different triggers instead of an ungraceful exit:

**08 — stuck "no game found" after a Joiner cancels once.** Linux-Linux: the Joiner found the Host's game, pressed Cancel on Multiplayer Setup, and "Join Game" then gave "game not found" — even after the Joiner fully closed and restarted the game and the Helper, re-pasting the same Ticket, against a Host whose session was never interrupted. Only the Joiner side was cycled, so this looks distinct from 06's leaked-session bug (which is about the *Host's* next hosting attempt). Possibly a DirectPlay session-discovery or lobby state issue on the Joiner's side that Cancel doesn't clean up. Unconfirmed: whether the Helper's iroh connection survives the Cancel, or it's purely game-side session enumeration that's stuck; whether a full Helper restart is even necessary to reproduce, versus just Cancel then Join Game again.

**09 — `Net::send` NULL pointer when the Host ends a game after an unnotified Joiner disconnect.** Windows-Linux over the internet: the game connected and played several turns; the Joiner quit, possibly without the Host's Helper registering the disconnect yet; the Host's "End Game" then produced the same `Net::send` "NULL pointer" signature as 06, but on a different path (ending an in-progress game, not re-hosting after one). Leads: whether the Helper notices a peer's iroh connection closing promptly, and whether that's surfaced to the DLL/game before "End Game" is allowed to send to that peer.

**10 — Stop pressed mid-game breaks the session three ways.** Found testing helper-web-ui ticket 17's Criterion 3 (2026-10-02, Windows and Linux), all mid-game: (a) Host presses Stop, clicks through the "return to the main menu first" warning — the Joiner's game immediately closed to desktop (possibly a crash), and a reconnect with the Host's new Ticket couldn't rejoin the same session ("No game found"); (b) Joiner presses Stop instead — the Host saw a "Net Underground" "Send failed!" popup at the next end of turn, then "Player not responding"; (c) same as (b), but neither side acknowledged the in-game "Drop the host/player" dialogue — a new iroh connection was established, but the next end of turn still produced "Send failed". **Per the map's standing preference:** (a)'s crash is in scope regardless of the warning; whether (b)/(c)'s "Send failed"/unresumable session is "not our problem" (an accepted cost of ignoring the warning) depends on whether it can happen *without* a crash — right now a crash is present at least once in this family, so treat all of 10 as in scope until that's separated out.

Resolve by establishing, for 08, 09 and 10:

1. Is there one shared mechanism (the Game session and the Helper's connection state falling out of sync, the same shape as 06) behind all three, or do they have independent causes?
2. For each: is it reachable without crashing anything (lesser) or does it crash a game (crash-class)? Is it reachable only by proceeding past the Stop warning (10), or can it happen in ordinary play (08, 09)?
3. Logs from a reproduction would help confirm any of these: `DPLAYX_LOG_FILE=/path/dplayx.log` and `SMAC_HELPER_LOG_FILE=/path/helper.log` on both sides, covering the trigger and the failure.
4. Reproduce at the IPC/Transport level where possible (as ticket 06 did), rather than relying on a real two-machine game each time.

## Comments

**2026-10-04 (agent):** A probable cause for 08 came out of [ticket 03](03-joiner-leaving-tells-the-host-its-session-was-lost.md). A Joiner's Helper that closes (its game cancelled out of Multiplayer Setup, or quit) tells every peer the whole session is over. The host's game is handed `DPSYS_SESSIONLOST` and, probably, stops hosting, so every later Join Game finds nothing, even after the Joiner restarts everything. Reproduced at the Transport level; the fix is ticket 03's. It's worth checking whether the same message is behind 10's "No game found" after a host-side Stop. 09 and the rest of 10 are untouched.

## Answer
