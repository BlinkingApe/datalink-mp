# 04: Confirm in a real game

Type: task
Status: ready-for-human
Blocked by: 01, 02, 03

## What to build

A real game with three players shows the host can leave mid-game and the others play on. Run it with the same setup as the 2026-10-20 logs: a Windows host, two Linux Joiners, all on `wtp.exe`, with logs collected on every machine (`docs/players/collecting-logs.md`). This can share a session with [alien-crossfire-fixes 04](../../archive/alien-crossfire-fixes/issues/04-confirm-colors-in-a-real-game.md).

This ticket also checks the one inference the fix rests on: that every game picks the same new host. If the games disagree, or SMAX doesn't cope with a mid-game host change, record what happened here and reopen ticket 02.

## Acceptance criteria

- [ ] The host quits from the in-game menu. Both Joiners' games stay open, and the players finish at least one more turn together.
- [ ] Both Joiners' games and Helpers name the same new host (the logs show who each took as host).
- [ ] The new host then quits. The last player carries on alone, or the game ends cleanly without exiting to the desktop.
- [ ] New game: the host presses Stop mid-game, past the warning. The Joiners carry on as above.
- [ ] Host leaving in Multiplayer Setup behaves as before (no exit, no crash).
- [ ] Logs from the run are kept locally in this effort's `captures/` folder, and the results are recorded on this ticket.

## Comments

2026-10-20, v0.1.3-rc.1 (shared session with alien-crossfire-fixes 04): reported by the tester, when the host quit mid-game the clients reported it and play switched to Rocky, the first Joiner. That covers the first criterion in outline. Not yet recorded: the same-new-host check in the logs, a further turn finished together, the new host quitting, Stop mid-game, and the Multiplayer Setup case. Logs are in the `captures/2026-10-20-v0.1.3-rc.1-test4` folder of the archived alien-crossfire-fixes effort (`.7z`, not yet read by an agent).

### Log read, same run (the extracted `x` folder beside them)

- 09:31:10 the host's Helper handles `CloseSession`; its DLL logs `cleanup complete` at 09:31:10.5.
- 09:31:12.33 both Joiners' Helpers receive `SessionClosed` and log `host left a game in progress, host role migrated`: Rocky `we_are_host=true`, Old_Black `we_are_host=false`. Both agree Rocky (the lower DPID, 0x20000) is the new host.
- Neither Joiner's DLL log shows an exit at that point. After 09:31:12 the only traffic is the small 12-byte keepalives and a couple of 44-byte messages already in flight; there are no further turn-sized messages in the DLL logs. So the logs show both games surviving the host's exit, but not a further turn played together. The tester reports play switched to Rocky.
- 09:31:39 Old_Black's game closes (`CloseSession`, `cleanup complete`); 09:32:06–08 Rocky closes, and Old_Black's Helper receives `SessionClosed` from it. Whether those were the players quitting is not in the logs.
- Not covered by this run as far as the logs show: Stop mid-game, and the Multiplayer Setup case.
