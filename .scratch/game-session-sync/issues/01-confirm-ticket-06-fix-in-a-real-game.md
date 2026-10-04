# Confirm the leaked-Game-session fix in a real game

Type: task
Status:
Blocked by:

## Question

Originally release-pipeline ticket 06. **Cause, already found** (2026-10-01, by reading the code and reproducing at the IPC level): when the game's connection to the Helper ended without `CloseSession` — a crash or a close while in the setup screen — the Helper only marked the game as not connected; the Game session stayed open. The next Host Game got "Failed to create session (already in session?)", the DLL returned `DPERR_CANTCREATESESSION`, and the game carried on into Multiplayer Setup with no session behind it, so `Net::send` failed on the first change (a white "NULL pointer!!" popup) and some drop-downs didn't react.

**Fix landed:** `SessionController::dll_disconnected` closes the Game session when the game disconnects without closing it itself (tells connected friends the session ended via `SessionClosed`, same as `CloseSession` always did). The regression test `test_game_that_left_without_closing_its_session_can_host_again` (`crates/datalink-mp/tests/ipc.rs`) passes.

**What's left is only the real-game confirm**, not run yet on `rc.4` or later: host a game, close it while sitting in Multiplayer Setup (not via CloseSession), restart the game, host again with the same Helper. Expected: no `Net::send` NULL pointer, and the setup screen's drop-downs react normally. If this still fails, the fix doesn't cover the real-game path the test modeled and the root cause needs revisiting.

## Comments

**2026-10-04 (maintainer, recorded by agent):** Run on `v0.1.0-rc.5`, with a Joiner taking part: the host closed the game from Multiplayer Setup and quit it, the Joiner cancelled and quit too, both Helpers stayed connected, and the host started the game and hosted again. The Joiner's Join Game then said "no game was found". The cause is a different defect, now [ticket 03](03-joiner-leaving-tells-the-host-its-session-was-lost.md), so this ticket's own question is still open. Not recorded: whether the host's new Multiplayer Setup showed the `Net::send` NULL pointer, and whether its drop-downs reacted. Rerun once ticket 03 is fixed.

## Answer
