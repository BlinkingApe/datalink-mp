# 03: Classic colors in Alien Crossfire

Type: task
Status: resolved
Blocked by: 01, 02

## What to build

In an Alien Crossfire multiplayer game, including under WTP, every faction wears its own colors on its unit flags, base labels, minimap dots and territory borders, the same on every machine. Today Rocky's Pirates showed orange and the laptop's Morgan showed black (see the [map](../map.md)). Build it the way ticket 01's write-up recommends, as the `terranx.exe` entry that ticket 02's per-binary choice makes room for. Like the SMAC fix, it must be render-only: no synced game state touched. `SMAC_NO_CLASSIC_COLORS=1` still turns it off.

## Acceptance criteria

- [ ] (code path built and logging; not run in a live game) In `terranx.exe`, the DLL log shows the colors fix installed at game start and the seating it rewrote the colors for.
- [x] The color-mapping logic is unit-tested for Alien Crossfire faction sets, including a faction the SMAC fix can't map (such as the Pirates) and single player (a no-op).
- [ ] (by source reading only, not run) With WTP running, the fix installs without fighting WTP's own patches (per ticket 01's findings).
- [x] `docs/players/faction-colors.md` covers Alien Crossfire and WTP, not only SMAC.

## Answer

Alien Crossfire (`terranx.exe`, which is also what WTP launches) now gets the classic-colors fix, built as ticket 01 recommended. It has not been run in a live game: no Windows/Wine game was available, so the first real MP game is the test.

What it does (`crates/datalink-fixes/src/classic_colors.rs`, `ac_colors.rs`):
- `selection.rs`: `ClassicColors` is available for `AlienCrossfire`. `BorderFix` stays SMAC-only, deliberately: ticket 01 found borders and flags read the same slot, so Alien Crossfire needs no border detour (this differs from ticket 02's note).
- `classic_colors::apply(binary)` picks the address set. For Alien Crossfire it first checks the original bytes at the seating call (`0x482cd8`, `e8 b3 00 00 00`), the seating function entry (`0x482d90`), `load_faction_art` (`0x453710`) and `load_flags` (`0x453f70`); any mismatch logs and installs nothing. Then it uses the same stub as the SMAC fix, pointed at the Alien Crossfire seating function.
- The hook runs after seating. It reads each seat's seating byte (`0x90db9b + seat*0x17c`) and faction file name (`MFactions[seat].filename`, `0x946a50 + seat*0x59c + 4`). If seating is not the default order, it calls the game's `load_faction_art(seat)` (cdecl, confirmed from the `ret` in the disassembly) for every seat that names a faction, then `load_flags()` once. Single player (default seating) does nothing.
- Pure, unit-tested logic in `ac_colors.rs`: `plan_reloads` (single player no-op, Pirates in seat 6, a two-player game with unused seats, permuted SMAC factions), `parse_filename`, `is_identity_seating`, `seat_log_line`. Because it reuses the game's own loader, it needs no per-faction color table, so the Pirates need no special case.
- Logging, for the live test: at start `classic colors: installed for Alien Crossfire (terranx.exe), hooked seating call at 0x00482CD8 ...`; at each game start one line per seat (`seat 6: seating byte 9, faction file PIRATES, colors reloaded`) and `seating rewritten, faction art reloaded for seats [...], flags restamped`. For default seating it says nothing to rewrite.
- `SMAC_NO_CLASSIC_COLORS=1` disables it. SMAC behavior is unchanged (same code path, tests pass).

Checks: `cargo test -p datalink-fixes` passes (31 tests); `cargo clippy -p datalink-fixes --target i686-pc-windows-gnu` builds with only the two warnings that were already there. I also confirmed the original bytes at the four sites against the installed `terranx.exe`.

Not tested in a live game, and inferred rather than known:
- That the cause is stale color tables. Ticket 01 inferred it; if `MFactions[seat].filename` is itself wrong for a seat, reloading from it will not help, and the fix would need the lobby-record name (research section 4, step 2). The per-seat log lines are what tell the two cases apart. I did not build that fallback, since nothing shows it is needed.
- "Single player" is detected as default seating (seat n holds faction n-1). This skips a reload that would be harmless anyway. A multiplayer game that happens to seat in default order is left alone too, which is right only if the tables are fresh then.
- The meaning of `[0x90e778]`, `[0x93d4f0]` and `[0x93d4f4]` is still unknown, as is whether the seating byte in Alien Crossfire is a lobby slot or a faction index; the fix does not depend on it.
- WTP coexistence rests on reading thinker-doer source (it patches neither the call site nor the two functions we call). Whether the installed `thinker.dll` is that exact build is unchecked, and the byte guard is what protects against a difference.
- Whether the hook running inside `AssignPlayerFactions`'s caller is safe for the game's loader (stack use, the shared scratch window `load_faction_art` writes) was reasoned, not observed.
- Calling the loaders needs the game's data files present; if a faction file failed to load the game's own error handling applies.

Docs: `docs/players/faction-colors.md` has an Alien Crossfire and WTP section; the one-line mention in `docs/contributors/building.md` now says both games.
