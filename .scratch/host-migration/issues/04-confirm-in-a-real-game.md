# 04: Confirm in a real game

Type: task
Status: ready-for-human
Blocked by: 01, 02, 03

## What to build

A real game with three players shows the host can leave mid-game and the others play on. Run it with the same setup as the 2026-10-20 logs: a Windows host, two Linux Joiners, all on `wtp.exe`, with logs collected on every machine (`docs/players/collecting-logs.md`). This can share a session with [alien-crossfire-fixes 04](../../alien-crossfire-fixes/issues/04-confirm-colors-in-a-real-game.md).

This ticket also checks the one inference the fix rests on: that every game picks the same new host. If the games disagree, or SMAX doesn't cope with a mid-game host change, record what happened here and reopen ticket 02.

## Acceptance criteria

- [ ] The host quits from the in-game menu. Both Joiners' games stay open, and the players finish at least one more turn together.
- [ ] Both Joiners' games and Helpers name the same new host (the logs show who each took as host).
- [ ] The new host then quits. The last player carries on alone, or the game ends cleanly without exiting to the desktop.
- [ ] New game: the host presses Stop mid-game, past the warning. The Joiners carry on as above.
- [ ] Host leaving in Multiplayer Setup behaves as before (no exit, no crash).
- [ ] Logs from the run are kept locally in this effort's `captures/` folder, and the results are recorded on this ticket.
