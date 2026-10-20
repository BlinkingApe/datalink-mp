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
