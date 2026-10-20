# 01: Research how `terranx.exe` colors factions in multiplayer

Type: research
Status: resolved
Blocked by: None (can start immediately)

## What to build

A write-up in `docs/` that gives ticket 03 everything it needs to port the classic-colors fix to Alien Crossfire. The SMAC fix assumes 7 factions in a fixed palette order (Gaians 1 … Peacekeepers 7). Alien Crossfire has more factions (Pirates, Morgan alongside the Spartans, and others) and may give each faction its color another way. So the write-up has to establish the mechanism first, not only the new addresses.

Use `terranx.exe` v2.0 (hash in the [map](../map.md)) and Thinker's source.

## Acceptance criteria

- [x] How Alien Crossfire picks a faction's color: which tables or files, and whether the SMAC "seat-indexed renderers versus faction-indexed borders" split holds.
- [x] The `terranx.exe` addresses (with the original bytes to check) for what the SMAC fix touches: the call that finalizes seating, the seating table and its stride, the palette tables, the flag-sprite stamper, and the border renderer's faction lookup. Or why one of them has no counterpart.
- [x] Whether WTP's `thinker.dll` patches or hooks any of those sites at runtime, or recolors factions itself, and how a fix should coexist with it.
- [x] Each fact marked as read from the binary, read from Thinker's source, or inferred.
- [x] A recommended approach for ticket 03, recorded under `## Answer` here with a pointer to the doc.

## Answer

Research: `docs/contributors/terranx-faction-colors.md`.

- Mechanism: in `terranx.exe` the four color tables (`0x68f9f0`, `0x68fa10`, `0x68fa30`, `0x68fa50`) are filled per slot by `load_faction_art(slot)` (`0x453710`) from that slot's own faction art file. `PALETTE.PCX` fills only entry 0. `AssignPlayerFactions` (`0x482d90`, called at `0x482cd8`) already ends with the flag stamper `load_flags` (`0x453f70`). Border and flags read the same slot index, so the SMAC split is not established and no border detour is needed.
- The SMAC patch does not port as is. Recommended for ticket 03: probe first, then hook `0x482cd8` and call `load_faction_art(seat)` for seats 1 to 7 with the seat's real faction file, then `load_flags()`.
- Thinker and WTP patch none of these sites (only `0x586050`, which does the same job) and do not recolor factions.
- Nothing was run in a live game; the stale-table cause is inferred.
