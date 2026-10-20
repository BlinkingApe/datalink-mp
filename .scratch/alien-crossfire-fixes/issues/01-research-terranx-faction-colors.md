# 01: Research how `terranx.exe` colors factions in multiplayer

Type: research
Status: ready-for-agent
Blocked by: None (can start immediately)

## What to build

A write-up in `docs/` that gives ticket 03 everything it needs to port the classic-colors fix to Alien Crossfire. The SMAC fix assumes 7 factions in a fixed palette order (Gaians 1 … Peacekeepers 7). Alien Crossfire has more factions (Pirates, Morgan alongside the Spartans, and others) and may give each faction its color another way. So the write-up has to establish the mechanism first, not only the new addresses.

Use `terranx.exe` v2.0 (hash in the [map](../map.md)) and Thinker's source.

## Acceptance criteria

- [ ] How Alien Crossfire picks a faction's color: which tables or files, and whether the SMAC "seat-indexed renderers versus faction-indexed borders" split holds.
- [ ] The `terranx.exe` addresses (with the original bytes to check) for what the SMAC fix touches: the call that finalizes seating, the seating table and its stride, the palette tables, the flag-sprite stamper, and the border renderer's faction lookup. Or why one of them has no counterpart.
- [ ] Whether WTP's `thinker.dll` patches or hooks any of those sites at runtime, or recolors factions itself, and how a fix should coexist with it.
- [ ] Each fact marked as read from the binary, read from Thinker's source, or inferred.
- [ ] A recommended approach for ticket 03, recorded under `## Answer` here with a pointer to the doc.
