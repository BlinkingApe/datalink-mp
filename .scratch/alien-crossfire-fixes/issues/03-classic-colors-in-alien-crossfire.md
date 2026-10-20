# 03: Classic colors in Alien Crossfire

Type: task
Status: ready-for-agent
Blocked by: 01, 02

## What to build

In an Alien Crossfire multiplayer game, including under WTP, every faction wears its own colors on its unit flags, base labels, minimap dots and territory borders, the same on every machine. Today Rocky's Pirates showed orange and the laptop's Morgan showed black (see the [map](../map.md)). Build it the way ticket 01's write-up recommends, as the `terranx.exe` entry that ticket 02's per-binary choice makes room for. Like the SMAC fix, it must be render-only: no synced game state touched. `SMAC_NO_CLASSIC_COLORS=1` still turns it off.

## Acceptance criteria

- [ ] In `terranx.exe`, the DLL log shows the colors fix installed at game start and the seating it rewrote the colors for.
- [ ] The color-mapping logic is unit-tested for Alien Crossfire faction sets, including a faction the SMAC fix can't map (such as the Pirates) and single player (a no-op).
- [ ] With WTP running, the fix installs without fighting WTP's own patches (per ticket 01's findings).
- [ ] `docs/players/faction-colors.md` covers Alien Crossfire and WTP, not only SMAC.
