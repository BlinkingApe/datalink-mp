# 04: Confirm colors in a real game

Type: task
Status: resolved
Blocked by: 03

## What to build

A real multiplayer game on `wtp.exe` shows every faction in its own colors, the same on every machine. It can share a test session with [host-migration 04](../../host-migration/issues/04-confirm-in-a-real-game.md).

## Acceptance criteria

- [x] Three players with at least one Alien Crossfire-only faction (such as the Pirates) and one original faction seated away from its usual slot (such as Morgan).
- [ ] Screenshots of the same map area from two machines, taken at about the same time, show each faction's flags, base labels and borders in its own color, matching on both machines.
- [x] Every machine's DLL log shows the colors fix installed. Logs are kept locally in this effort's `captures/` folder, and the results are recorded on this ticket.

## Comments

2026-10-20, v0.1.3-rc.1 (shared session with host-migration 04): a three-player game on `wtp.exe` (Windows host, two Linux Joiners). Reported by the tester: all three factions had their correct colors. Logs from all three machines are kept locally in `.scratch/alien-crossfire-fixes/captures/2026-10-20-v0.1.3-rc.1-test4/` (`.7z`, not yet read by an agent). Not yet recorded: which factions were seated (the Pirates / a displaced Morgan), the same-area screenshots from two machines, and the DLL log's `classic colors` lines on each machine.

### Log read, same run (extracted under `captures/…/x/`)

- All three DLL logs show `game binary: Alien Crossfire (terranx.exe)` and `classic colors: installed … hooked seating call at 0x00482CD8`, then at game start (09:28:42 on the Joiners, 09:28:40 on the host) the line `seating rewritten, faction art reloaded for seats [1..7], flags restamped`.
- Seats 1–3 are the same on all three machines: SPARTANS, PIRATES, MORGAN (seating bytes 4, 8, 3). So the game had a Pirates seat and a Morgan seat away from her usual slot. Seats 4–7 differ between machines (unused seats).
- **The reload changed nothing.** For every seat, on every machine, the color tables read identically before and after the reload. So the tables were already right when the hook ran. The colors were correct, but these logs cannot show the fix caused that. The research's stale-table theory is not what these tables looked like at this moment. To settle it, play the same seating on `v0.1.2` (no fix) or with `SMAC_NO_CLASSIC_COLORS=1`.
- Still missing: same-area screenshots from two machines.

### Result

2026-10-20: the tester ran two more games with the Joiner order mixed up and each player choosing random factions. In both, every faction had the correct color. The tester accepted that as enough. The two-machine screenshots criterion was waived, not met: no screenshots were taken. The extra two games' logs were not supplied, so the DLL-log criterion is ticked from the first run (all three machines logged the fix installed and the seating rewritten). The fix itself was not shown to be what made the colors right (tables were identical before and after the reload); no `SMAC_NO_CLASSIC_COLORS=1` comparison was run. Resolved.
