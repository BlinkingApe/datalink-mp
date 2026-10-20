# 02: The fixes crate picks its patches by game binary

Type: task
Status: ready-for-agent
Blocked by: None (can start immediately)

## What to build

When the game starts, the DLL works out which game binary it's in (SMAC's `terran.exe` or Alien Crossfire's `terranx.exe`, PRACX builds included) and logs it. It then applies only the patches and probes that exist for that binary. A patch with no version for this binary logs one plain line saying so, instead of today's "unknown binary version" error. A binary that's neither is reported as unknown, and no patches are applied.

This is a prefactor: it fixes nothing new in Alien Crossfire, but ticket 03 then only adds a `terranx.exe` entry. SMAC behaviour stays exactly as it is.

## Acceptance criteria

- [ ] The DLL log names the detected binary at startup.
- [ ] In `terran.exe`, every patch and probe installs as today.
- [ ] In `terranx.exe` (including under WTP), the log says each SMAC-only patch isn't available for this binary, with no errors, and nothing is written to game memory.
- [ ] Detection is covered by unit tests that don't need the game, for example from known header or byte samples.
- [ ] Each patch still checks its original bytes before writing, so a modded binary is refused safely.
