# Faction colors in multiplayer

**TL;DR: SMAC's correct colors follow faction identity — Spartans are black,
the Hive is blue, Gaians are green, always. Single player renders them
correctly; a vanilla bug makes multiplayer units, flags, and base labels wear
the wrong faction's colors. This mod fixes it: every faction wears its
classic colors in multiplayer too. Disable with `SMAC_NO_CLASSIC_COLORS=1`.**

## The color system

`PALETTE.PCX` contains four 8-entry color rows — primary, dark, text, shadow
— sampled once at startup (into the tables at
`0x6aeeb4`/`0x6aeed4`/`0x6aeef4`/`0x6aef14`), one entry per faction in
canonical order: 1 Gaians (green), 2 Hive (blue), 3 University (white),
4 Morgan (yellow), 5 Spartans (black), 6 Believers (orange),
7 Peacekeepers (lavender).

Renderers reach this table through two different owner indexes:

- **Territory borders** use the tile's territory owner byte (`tile[7]`, via
  the territory query at `0x4f97a0`), which holds the owner's **canonical
  faction number** → borders always show the faction's true classic color.
- **Unit flags, base labels, minimap dots** use the owner's **seat number**
  (the player-roster index assigned at game setup; unit flag sprites are
  pre-stamped per seat at load, `0x45f3a0`).

In single player faction *i* always sits in seat *i*, so both indexes agree
and everything is classic. In multiplayer the game re-seats factions at game
start (generally not in canonical order, and possibly differently from your
lobby position) — and the seat-indexed renderers then show the wrong
faction's colors. Observed example: Spartans seated 2nd render blue units and
flags (the Hive's color) while their territory borders remain correctly
black. The 1999 game behaved the same way over LAN/IPX.

## The fix in this mod (`classic_colors`, on by default)

Physically re-seating factions would touch every seat-keyed game structure
(rosters, message routing, synced state) — too risky. Instead the fix is
render-only. When the game finalizes seating at game start (the
`AssignPlayerFactions` call, hooked at `0x491583`), the mod:

1. rewrites the four palette tables so `table[seat] = original[faction of
   that seat]` — labels, minimap, and overlays immediately show true faction
   colors;
2. re-runs the game's own unit-flag sprite stamper (`0x45f3a0`) so unit and
   base flags are rebaked with those colors;
3. translates the border renderer's faction index to the seat index
   (`0x470d2a` detour), so borders read the rewritten tables correctly and
   stay classic.

No synced game state is touched; all machines run the same DLL and render
identically. In single player seating is identity and the whole fix is a
no-op. `SMAC_NO_CLASSIC_COLORS=1` disables it (vanilla behavior: borders
classic, units seat-colored; `SMAC_BORDER_MATCH_UNITS=1` then optionally
recolors borders to match the wrong units instead).

## Other visual details worth knowing

- **You only see the territory borders of factions the game considers you in
  contact with** (the border renderer checks the diplomacy table before
  drawing). Early in a game, a neighboring faction's territory may have no
  visible border at all.
- Border dashes only appear at ownership-change edges on tiles you have
  explored. A base whose whole explored neighborhood is its own territory
  shows no ring — the boundary is beyond the explored edge.
- Unit *body* sprites are mostly fixed art (colony pods and rovers are
  tan-yellow for everyone); only the small flag on the unit carries the owner
  color.
- Unit/base flag *poles* render in a fixed green regardless of owner (static
  art).

## What would actually be a networking bug

All machines in a session share the same seating and territory state, so the
same unit, base, or border must show the **same color on every machine**
(wrong in the same way everywhere). If two machines ever disagree about the
color of the same thing on the map, that is real state divergence — please
report it, ideally with simultaneous screenshots of the same map region from
two machines.
