# 02: The fixes crate picks its patches by game binary

Type: task
Status: resolved
Blocked by: None (can start immediately)

## What to build

When the game starts, the DLL works out which game binary it's in (SMAC's `terran.exe` or Alien Crossfire's `terranx.exe`, PRACX builds included) and logs it. It then applies only the patches and probes that exist for that binary. A patch with no version for this binary logs one plain line saying so, instead of today's "unknown binary version" error. A binary that's neither is reported as unknown, and no patches are applied.

This is a prefactor: it fixes nothing new in Alien Crossfire, but ticket 03 then only adds a `terranx.exe` entry. SMAC behaviour stays exactly as it is.

## Acceptance criteria

- [x] The DLL log names the detected binary at startup.
- [x] In `terran.exe`, every patch and probe installs as today.
- [x] In `terranx.exe` (including under WTP), the log says each SMAC-only patch isn't available for this binary, with no errors, and nothing is written to game memory.
- [x] Detection is covered by unit tests that don't need the game, for example from known header or byte samples.
- [x] Each patch still checks its original bytes before writing, so a modded binary is refused safely.

## Answer

The fixes crate now works out its game binary at startup and applies only the patches that exist for it.

- `crates/datalink-fixes/src/binary.rs`: `GameBinary { Smac, AlienCrossfire }`. `identify(header)` recognizes the binary from the PE header (TimeDateStamp, entry point, SizeOfImage) of the image at 0x400000, so `terranx_PRACX.exe`, WTP's `terranx_mod.exe` and the PRACX `terran` build all identify by their image, not their file name. `detect_current()` reads that header from the running process (Windows only). Unit tests use header samples, with literals read from the shipped executables.
- `crates/datalink-fixes/src/selection.rs`: the `AVAILABILITY` table says which binaries each `Patch` (`BlitFix`, `ClassicColors`, `BorderFix`, `DebugProbes`) has addresses for. `plan(Option<GameBinary>)` returns `Apply`, `NotAvailable(binary)` or `UnknownBinary` per patch. Tested without the game.
- `lib.rs`: `apply_all_patches()` and `install_debug_probes()` follow the plan. The log names the binary at startup; a patch with no version for it logs one "not available for ..." line (info, no error); an unrecognized binary logs one warning and nothing is applied or written. The probe thread isn't even started when probes aren't available.
- The patch modules are untouched, so SMAC behaviour and the original-bytes check before every write are unchanged.

Ticket 03 extends: add `GameBinary::AlienCrossfire` to `ClassicColors` (and `BorderFix`, which classic colors installs) in `AVAILABILITY`, and make `classic_colors.rs` / `border_fix.rs` hold per-binary addresses, chosen by passing the `GameBinary` into their `apply()` (it is available in `lib.rs` as `detected_binary()`). `apply()` there should still verify original bytes before writing.

Not exercised on a real Windows/Wine game from here; the crate builds for `i686-pc-windows-gnu` and its tests pass on Linux.
