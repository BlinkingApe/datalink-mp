# How `terranx.exe` colors factions in multiplayer

Research for the [alien-crossfire-fixes](../../.scratch/alien-crossfire-fixes/map.md) effort, ticket 01. It gives ticket 03 (classic colors in Alien Crossfire) the mechanism and the addresses. The SMAC fix is described in [faction-colors.md](../players/faction-colors.md) and lives in `crates/datalink-fixes/src/classic_colors.rs` and `border_fix.rs`.

**Bottom line.** Alien Crossfire does not sample its palette tables once from `PALETTE.PCX` like SMAC does. Each seat's colors are read from that seat's own faction art file (`GAIANS.pcx`, `PIRATES.pcx`, ...) by a function `terranx.exe` already has, and `AssignPlayerFactions` already re-stamps the flag sprites afterwards. So the SMAC patch (rewrite tables by seating, restamp, translate borders) does not port. A port should make the game redo its own per-seat color load from the right file, and it needs no border detour.

## Method and sources

Every fact below carries one of these tags.

- **[binary]**: read from `terranx.exe` v2.0 (sha256 `01901cbf7196b0c5d0df9540a029520f5df8fd9a6b343deef8b5663872805fcf`, the build in `docs/archive/research/smac-jackal-turn-sync.md`) with `objdump -d -M intel`, or from the game's data files. Byte dumps at the patch sites are identical in `terranx_PRACX.exe` and in `terranx_mod.exe` from the WTP install.
- **[Thinker]**: read from [induktio/thinker](https://github.com/induktio/thinker) `master` (`src/engine.cpp`, `config.cpp`, `game.cpp`, `map.cpp`, `patch.cpp`).
- **[WTP]**: read from [tnevolin/thinker-doer-clion](https://github.com/tnevolin/thinker-doer-clion) `master` at commit `2ca03634` (2026-10-01), the source of WTP's `thinker.dll`. I did not hash-match the installed `thinker.dll` (sha256 `04d61fb2...`) to that commit.
- **[inferred]**: my reading of what the above implies. Nothing here was checked in a running game.

The SMAC `terran.exe` (sha256 `0a61eb36...`) was disassembled the same way to line up the two games' code by shape.

## 1. The mechanism

### Seat indexing

- The color tables are indexed by the same small integer as `Factions[]` and `MFactions[]`: the player slot, 1 to 7, with 0 for natives. **[Thinker]** (`MFactions` at `0x946A50`, stride `0x59c`; `MFaction.filename` is at +4.)
- Territory ownership uses that same integer: `whose_territory` returns `sq->owner`, a `faction_id` in that range. **[Thinker]** (`map.cpp`). I read `0x4E3EF0` in `terranx.exe` calling it from the border renderer. **[binary]**
- Seven slots exist, however many faction files the game ships (14 in Alien Crossfire). **[binary]** (every table is 8 entries.)

### Where the colors come from

There are four tables of 8 `u32` entries each, at `0x68f9f0`, `0x68fa10`, `0x68fa30`, `0x68fa50`. **[binary]** In SMAC they are at `0x6aeeb4`, `0x6aeed4`, `0x6aeef4`, `0x6aef14`. The values are palette indexes, not RGB.

| Writer | What it does |
|---|---|
| `0x454403`-`0x454446` | Loads `PALETTE.PCX` and reads the four tables at x = `0x244 + 8*i`, y = `0x17`, `0x1f`, `0x27`, `0x2f`. The loop has a `test edi,edi; jne` guard, so only iteration 0 stores: **only entry 0 (natives) comes from `PALETTE.PCX`**. **[binary]** SMAC's equivalent at `0x45f6d8` stores all 8 entries. **[binary]** |
| `0x453710`, `load_faction_art(seat)` | Builds the file name `MFactions[seat].filename + ".pcx"`, loads it, and reads one pixel per table at x = 4, y = `0x2e3`, `0x2eb`, `0x2f3`, `0x2fb`. It stores them at `table[seat]` (`0x4537a9`, `0x4537c1`, `0x4537d9`, `0x4537e8`). **[binary]** The name `load_faction_art` is from **[Thinker]** (`engine.cpp:408`). |

I decoded the faction art files in the install. Their pixels at those four positions match the SMAC palette rows: Gaians `231`, Hive `242`, University `245`, Morgan `241`, Spartans `55`, Believers `92`, Peacekeepers `225`. The Alien Crossfire files have their own values (Cyborg `197`, Pirates `244`, Drones `99`, Angels `228`, Caretakers `142`, Usurpers `139`). **[binary]** (data files, not the exe). With the palette in `palette.pcx` those are green, blue, white, yellow, black, orange and lavender for the SMAC seven, and cyan for Pirates.

So in Alien Crossfire a seat's colors are whatever faction file was last loaded into `MFactions[seat].filename` when `load_faction_art(seat)` last ran. **[binary]**

### The flag stamper

`0x453f70` is the sprite stamper. It re-bakes the per-seat flag sprites by reading `table[seat]` (`0x454095`, `0x4540a8`, `0x4541cb`, `0x4541de`). **[binary]** Thinker names it `load_flags` (`engine.cpp:411`). **[Thinker]** SMAC's counterpart is `0x45f3a0`. **[binary]**

### The seating function

The function at `0x482d90` is SMAC's `AssignPlayerFactions` (`0x491630`): same shape, same seat copy loops, one caller. **[binary]** Thinker calls it `NetWin::prepare_game` (comments at `write_call(0x482E79 ...)`). **[Thinker]** Its sole caller is `call 0x482d90` at `0x482cd8`, in the same position as SMAC's `0x491583`. **[binary]**

What it does after the random fill of empty seats:

1. Copies each seat's record from `0x90db98 + seat*0x17c` to `0x93d520 + seat*0x19c`. If `[0x90e778]` is 0 or 1, the first byte of the copy is the seat index. Otherwise it is the seating byte. **[binary]**
2. Then, by the same `[0x90e778]` test, it takes one of two paths. **[binary]**
   - **0 or 1** (`0x482f9f`): for each seat 1 to 7 it copies the name of the lobby player record `[this+0x772c] + seatingbyte*0x190 + 0xa` into `MFactions[seat].filename` (and the other name fields), calls `0x586050(seat)`, then `0x5b0d70(seat)`. `0x586050` is `read_faction2`: it re-parses the faction file and calls `load_faction_art(seat)`. `0x5b0d70` is `compute_faction_modifiers`. **[binary]** for the call structure, **[Thinker]** (`config.cpp:576`, `patch.cpp:510,557`) for the names.
   - **2 or more** (network games, `0x482f46`): if `[0x93d4f0] == [0x93d4f4]` it skips straight to the end and reloads nothing. If they differ it copies lobby record `i` (not the seating byte) into `MFactions[i].filename` and calls `load_faction_art(i)` for `i` = 1 to 7. **[binary]** I did not work out what `0x93d4f0` and `0x93d4f4` hold; they are touched by the lobby code at `0x47b0xx`-`0x47e4xx`.
3. Ends with `call 0x453f70` (the stamper). **[binary]** (`0x483169`.) Thinker's own `config_game` does the same pair, `load_faction_art(i)` then `load_flags()`. **[Thinker]** (`game.cpp:1521-1523`.)

### Does the SMAC "seat-indexed versus faction-indexed" split hold?

- The border renderer's palette read is the same shape as SMAC's: `mov eax,[ebp-0x48]; test eax,eax; jle; mov eax,[eax*4+0x68f9f0]` at `0x464418`. **[binary]** The owner is the result of `0x4e3ef0` (`whose_territory`) at `0x4640a2`. **[binary]**
- `whose_territory` returns `sq->owner`. **[Thinker]** The flag stamper indexes by the same slot. **[binary]** So both renderers use the same index, with no second "canonical faction number". **[inferred]** (This rests on Thinker's reading of the tile owner; I did not trace the byte's writers in `terranx.exe`.)
- That means the SMAC split, where borders stay classic while units show the wrong color, **is not established for Alien Crossfire**. It could only appear if `table[seat]` is stale or wrong for both. **[inferred]**

### Why the 2026-10-20 game still showed wrong colors

The map notes record Pirates orange (the Believers' color) and Morgan black (the Spartans' color). The simplest fit, **[inferred]**, is stale tables, not a missing fix:

- At startup `read_factions` loads art for `MFactions[1..7]` in the default order, which is Gaians to Peacekeepers unless an ini file or `JENN282` picks randoms (`config.cpp:~910`). **[Thinker]** That fills `table[1..7]` with the SMAC seven's colors.
- For a network game the seating function only reloads art when `[0x93d4f0] != [0x93d4f4]` (above). For the other machines it leaves the startup tables in place. **[binary]**
- A faction seated 6th in the lobby, such as Pirates, would then wear `table[6]`, which is the Believers' orange. **[inferred]**

I can't confirm this without a log. That is why ticket 03's first step is a probe (section 4).

## 2. Addresses for the SMAC fix's sites

All in `terranx.exe` v2.0. "Original bytes" are the on-disk bytes, which also match `terranx_PRACX.exe` and `terranx_mod.exe`. **[binary]**

| Site | SMAC `terran.exe` | `terranx.exe` | Original bytes in `terranx.exe` |
|---|---|---|---|
| Call that finalizes seating | `0x491583` calls `0x491630` | `0x482cd8` calls `0x482d90` | `e8 b3 00 00 00` |
| Seating function entry | `0x491630` | `0x482d90` | `55 8b ec 6a ff 68 06 75 65 00` |
| Seating table (assigned-faction byte) | `0x917893`, stride `0x17c` | `0x90db9b`, stride `0x17c` (seat 1 at `0x90dd17`, end `0x90e77b`) | n/a (data) |
| Palette tables, 8 entries each | `0x6aeeb4`, `0x6aeed4`, `0x6aeef4`, `0x6aef14` | `0x68f9f0`, `0x68fa10`, `0x68fa30`, `0x68fa50` | n/a (data) |
| Table sampler | one pass at `0x45f6d8` | `load_faction_art` at `0x453710`, plus entry 0 only at `0x454403` | `55 8b ec 83 ec 10 56 8b 75 08 57 56` at `0x453710` |
| Flag-sprite stamper | `0x45f3a0` | `0x453f70` | `55 8b ec 6a ff 68 50 51 65 00 64 a1` |
| Border renderer's lookup | `0x470d2a` (`8b 45 bc 85 c0 7e 0d`) | `0x464418` | `8b 45 b8 85 c0 7e 0d` |
| Border: continue / skip targets | `0x470d31` / `0x470d3e` | `0x46441f` / `0x46442c` | `8b 04 85 f0 f9 68 00` at `0x46441f` |

Notes for reusing the SMAC code:

- The border owner is at `[ebp-0x48]`, not `[ebp-0x44]`. **[binary]**
- Calling convention: both are plain functions (no `this`). `load_faction_art` takes one stack argument (the slot). The stamper takes none; Thinker declares it `fp_none`. **[binary]**
- The seating function takes `this` in `ecx` and its frame setup is `push ebp; mov ebp,esp`, so a stub at `0x482cd8` can keep the SMAC shape: set up nothing, `call 0x482d90`, then call the hook. **[binary]**
- Two items have no direct counterpart: the "rewrite all four tables from `PALETTE.PCX` order" step (the tables are not in `PALETTE.PCX` order), and the faction-to-seat border translation (the border index already is the slot). **[inferred]**

## 3. WTP and Thinker at runtime

WTP's `wtp.exe` launches `terranx.exe` and injects `thinker.dll`. **[binary]** (`wtp.exe` prints "Cannot find terranx.exe" when it is missing.) The DLL patches code with `write_jump`/`write_call`/`write_bytes` at startup.

- **None of the sites in section 2 is patched**, except that `0x586050` (`read_faction2`) is jumped to a Thinker replacement. **[WTP]** (`patch.cpp:535`.) The replacement does the same job: `read_faction(&MFactions[id], 0); load_faction_art(id);` (`config.cpp:576-581`). **[WTP]**
- WTP also patches `0x482E79` and `0x482E96` (it swaps the random-number calls inside the seating function) and `0x5B0D70` (`compute_faction_modifiers`). **[WTP]** (`patch.cpp:686-687`; the `0x5B0D70` jump is in Thinker's `patch.cpp:557`.) These are inside or beside the seating function, but the call at `0x482cd8` and the function's first bytes are untouched.
- `0x4E3EF0` (`whose_territory`) is replaced by a Thinker/WTP function that returns `sq->owner` as the original does. **[WTP]** (`patch.cpp:463`, `map.cpp:1450`.) Border ownership is unchanged by it.
- **Neither Thinker nor WTP recolors factions.** A search of both trees finds no palette, color-table or `PALETTE.PCX` handling, only calls to `load_faction_art` and `load_flags` (Thinker `game.cpp:366, 1377, 1521-1523, 1786`; WTP `game.cpp:345, 1089, 1233-1235, 1501`). **[Thinker]** **[WTP]**
- They also fix a case where a loaded scenario skips `load_faction_art` (`patch.cpp:1252`, bytes written at `0x5AAEE0`). **[WTP]**

Coexistence: the fix should install its own patch late, treat a site as unpatched only if the bytes match the table above, and call the game's functions at `0x453710` and `0x453f70` instead of replacing them. Calling them is safe next to WTP because WTP calls the same two functions. **[inferred]** Run the fix after `thinker.dll` has applied its patches (the hook only runs at game start, long after DLL load), so `0x586050` already holds WTP's jump by then. That doesn't matter, because the fix never touches it.

Caution: the old SMAC patch checks its own call site's original bytes at DLL load. If WTP ever rewrote `0x482cd8`, a byte check would fail with "unknown binary version". The current WTP source does not touch it. **[WTP]** **[inferred]**

## 4. Recommended approach for ticket 03

1. **Probe first (no behavior change).** In the `0x482cd8` hook, log after the seating function returns: for seats 1 to 7, the seating byte, `MFactions[seat].filename` (at `0x946a50 + seat*0x59c + 4`), and `table[0..3][seat]` (the four tables). Run one real multiplayer game on `wtp.exe` and compare. The expected finding is a seat whose `table` entry does not match its file's colors (Pirates in seat 6 holding `92`). That would turn the section 1 inference into a fact. If the entries are already right, the bug is elsewhere and ticket 04's real-game check is the next step.
2. **Fix by redoing the game's own load.** After `AssignPlayerFactions` returns, for each seat 1 to 7 whose `MFactions[seat].filename` is non-empty, call `load_faction_art(seat)` (`0x453710`), then call `load_flags()` (`0x453f70`) once. This gives `table[seat]` the colors of the faction file the seat actually holds, and re-stamps every flag. No palette arithmetic, no PCX decoding, and it works for all 14 files and for custom factions.
   - If the probe shows `MFactions[seat].filename` is itself wrong for a seat (for example it still names the startup default), take the name from the lobby record `[this+0x772c] + seatingbyte*0x190 + 0xa` instead, and store it before calling `load_faction_art`. This mirrors what the game does on the 0/1 path. Use a save and restore of the file name rather than leaving a rewritten name behind, unless the probe shows the game state should hold it.
3. **No border detour.** The border and the flags read the same slot (section 1), so fixing `table[seat]` fixes both. Skip `border_fix` for Alien Crossfire and keep its SMAC address set separate (matches the map's "keep the SMAC patches" decision).
4. **Keep it render-only and idempotent.** `load_faction_art` writes only the table entry and a shared scratch window; the fix does not touch synced game state. Calling it twice with the same file gives the same result, so replays and reloaded games are safe. **[inferred]**
5. **Guard by bytes.** Install only if `0x482cd8` still holds `e8 b3 00 00 00`, and log which of the other section 2 sites matched, so ticket 02's per-binary selection can report "not available for this binary" cleanly.

## 5. What this doesn't settle

- Runtime values: nothing here was run. The stale-table explanation in section 1 is the best fit for the logs, not a confirmed cause.
- The meaning of `[0x90e778]` (0 or 1 versus 2 or more), and of `[0x93d4f0]` and `[0x93d4f4]`. I read the branches, not their writers.
- Whether `0x90db9b`'s seating byte is a lobby slot or a faction index in network mode. The 0/1 path indexes lobby records with it; the net path ignores it for naming.
- The meaning of the pixel rows 2 to 4 (`dark`, `text`, `shadow` in SMAC's naming). The fix doesn't depend on it, because it reuses the game's loader.
- Whether the installed `thinker.dll` is the exact `2ca03634` build.
