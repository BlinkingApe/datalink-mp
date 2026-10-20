# 04: Confirm colors in a real game

Type: task
Status: ready-for-human
Blocked by: 03

## What to build

A real multiplayer game on `wtp.exe` shows every faction in its own colors, the same on every machine. It can share a test session with [host-migration 04](../../host-migration/issues/04-confirm-in-a-real-game.md).

## Acceptance criteria

- [ ] Three players with at least one Alien Crossfire-only faction (such as the Pirates) and one original faction seated away from its usual slot (such as Morgan).
- [ ] Screenshots of the same map area from two machines, taken at about the same time, show each faction's flags, base labels and borders in its own color, matching on both machines.
- [ ] Every machine's DLL log shows the colors fix installed. Logs are kept locally in this effort's `captures/` folder, and the results are recorded on this ticket.

## Comments

2026-10-20, v0.1.3-rc.1 (shared session with host-migration 04): a three-player game on `wtp.exe` (Windows host, two Linux Joiners). Reported by the tester: all three factions had their correct colors. Logs from all three machines are kept locally in `.scratch/alien-crossfire-fixes/captures/2026-10-20-v0.1.3-rc.1-test4/` (`.7z`, not yet read by an agent). Not yet recorded: which factions were seated (the Pirates / a displaced Morgan), the same-area screenshots from two machines, and the DLL log's `classic colors` lines on each machine.
