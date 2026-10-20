# Map: The game fixes work in Alien Crossfire

## Destination

The DLL's game fixes, starting with classic faction colors, work in the game people actually play: Alien Crossfire's `terranx.exe`, usually through `wtp.exe` (Will To Power, built on Thinker). Today every patch and probe in the fixes crate holds addresses from SMAC's `terran.exe`, so none installs in an Alien Crossfire game.

## Notes

- Evidence: the 2026-10-20 LAN logs (Windows host, two Linux Joiners, all on `wtp.exe`), kept locally in [host-migration](../../host-migration/map.md)'s `captures/2026-10-20-wtp-lan/`.
  - Every game launch on every machine logged "original bytes don't match - unknown binary version" for the blit fix and the classic-colors fix, and "unexpected bytes at address" for each debug probe.
  - The host's log shows the colors fix "hooked" once, at 06:34. That was a separate 4-second launch of SMAC (`terran.exe`), not the game that was played.
  - In the game, Rocky's Pirates showed orange (the Believers' color) and the laptop's Morgan showed black (the Spartans'). That fits the unfixed game's seat-colored units (`docs/players/faction-colors.md`), but the logs don't show the seating, so it's unconfirmed.
- Binaries: `terranx.exe` v2.0, sha256 `01901cbf7196b0c5d0df9540a029520f5df8fd9a6b343deef8b5663872805fcf`, the same build as `docs/archive/research/smac-jackal-turn-sync.md`. The PRACX build (`terranx_PRACX.exe`) has the same bytes at the patch sites checked so far. Thinker's source names many `terranx.exe` functions and addresses.
- WTP's `thinker.dll` patches `terranx.exe` in memory at startup. A site may hold different bytes once WTP has run than on disk, and WTP may already change faction colors itself.

### Decided while charting

- **Keep the SMAC patches.** The crate chooses its patches by binary rather than swapping one address set for the other.

## Tickets

1. [Research how `terranx.exe` colors factions in multiplayer](issues/01-research-terranx-faction-colors.md)
2. [The fixes crate picks its patches by game binary](issues/02-patches-picked-by-game-binary.md)
3. [Classic colors in Alien Crossfire](issues/03-classic-colors-in-alien-crossfire.md)
4. [Confirm colors in a real game](issues/04-confirm-colors-in-a-real-game.md)

## Decisions so far

<!-- one line per closed ticket -->
- 04: Confirmed in real `wtp.exe` games on v0.1.3-rc.1: every faction had its correct color in three games; screenshots waived, and the fix's own effect wasn't isolated (tables were already right before the reload). [Result](issues/04-confirm-colors-in-a-real-game.md#result)
- 03: `terranx.exe` gets the classic-colors fix: after seating, if not default order, it calls the game's `load_faction_art` per seat then `load_flags`, no border detour, byte-guarded, with per-seat logging; built and unit-tested but not run in a live game. [Answer](issues/03-classic-colors-in-alien-crossfire.md#answer)
- 02: The crate detects `terran.exe` vs `terranx.exe` from the image PE header and applies patches via a per-binary table; ticket 03 adds `AlienCrossfire` entries. [Answer](issues/02-patches-picked-by-game-binary.md#answer)
- 01: `terranx.exe` fills its color tables per slot from each faction's art file (`load_faction_art`) and already restamps flags after seating; the SMAC patch doesn't port, so ticket 03 should probe, then call the game's own loaders from a hook at `0x482cd8`, with no border detour. Research: `docs/contributors/terranx-faction-colors.md`.

## Not yet specified

- Porting the backwards-blit fix (and any debug probes still wanted) to `terranx.exe`. Ticket 02 makes the crate report it as "not available for this binary". Whether it's needed in Alien Crossfire is unknown.
