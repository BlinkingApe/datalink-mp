# Test the Wine override without winecfg

Type: task (HITL)
Status: resolved
Blocked by:

## Question

What must a fresh Linux user actually do for Wine to load the shipped `dplayx.dll` from the game folder? The user's current setup has the override set through winecfg, which hides this.

Checklist (for the user):

1. In winecfg → Libraries, remove the `dplayx` override for the game's prefix.
2. With `dplayx.dll` in the game folder and no override, launch the game and check the helper log / in-game multiplayer menu: is the Iroh provider present? (Expected: no, Wine's builtin wins.)
3. Launch with `WINEDLLOVERRIDES="dplayx=n,b"` via:
   - a plain shell (`WINEDLLOVERRIDES=... wine thinker.exe`)
   - Faugus (launch options / env field; note the exact field name)
   - Steam non-Steam game (`WINEDLLOVERRIDES="dplayx=n,b" %command%`), if available
4. Record which work, the exact strings and UI field names, and the Wine/Proton versions.

Resolution records the facts the Linux setup text and README will be written from.


## Answer

**A fresh user must set `WINEDLLOVERRIDES=dplayx=n,b` in their launcher; dropping `dplayx.dll` in the game folder is not enough.** Tested 2026-09-30 on GE-Proton11-7 (umu-launcher 1.4.4, runtime `umu-steamrt4`), with the prefix's registry override removed:

| Run | Override | Result |
|---|---|---|
| Faugus 2.4.2 | none | Builtin wins: no DLL log, no "Iroh P2P" in the MP menu |
| Faugus 2.4.2 | `WINEDLLOVERRIDES=dplayx=n,b` in the env-var field, **unquoted** | **Works**: "Iroh P2P" listed |
| Shell, `umu-run thinker.exe` | `WINEDLLOVERRIDES="dplayx=n,b"` | **Works**: log has `DllMain: DLL_PROCESS_ATTACH`, "Iroh P2P" listed |
| Steam (flatpak), non-Steam game | `WINEDLLOVERRIDES="dplayx=n,b" %command%` | **Inconclusive**: needed Flatseal to grant access to the game folder, then a "C++ runtime error" on every Proton version tried. It's unknown whether the override is involved. |

Facts for the setup text:
- **Faugus.** The value goes in the per-game field that stores env vars labelled **Game Arguments** in the Faugus UI (`game_arguments` in `games.json`, alongside `PROTON_ENABLE_WAYLAND=1`), space-separated and without quotes.
- **Faugus quoting.** The field seems to shell-split its contents: `DPLAYX_LOG_FILE=Z:\...` produced no log file anywhere. Diagnostic paths given for Faugus must use forward slashes, or be quoted. This doesn't affect the override string itself.
- **Shell.** The generic `WINEDLLOVERRIDES="dplayx=n,b"` prefix is confirmed under Proton/umu. Plain system Wine was not tested (none installed).
- **Steam.** Untested in practice. By the standing "README claims only what has been tested" preference, it can't be listed as working. Whether to keep it at all is passed to Revise and accept ADR-0001.
- `n,b` is sufficient; no registry edit or winecfg is needed.

Afterwards the prefix's `user.reg` was restored from backup, including `"dplayx"="native"`.

## Comments

**2026-09-30, prep (agent):**
- Setup under test: Faugus 2.4.2 (flatpak) → umu-launcher 1.4.4 → GE-Proton11-7, runtime `umu-steamrt4`; prefix `/home/jct/Faugus/smax-will-to-power`; game `/home/jct/Games/AC-WTP_431/thinker.exe`. Steam is also a flatpak. No system Wine is installed.
- `terranx.exe` / `terranx_mod.exe` import `DPLAYX.dll` statically, so the DLL loads at game start. `crates/dplayx` logs `DllMain: DLL_PROCESS_ATTACH` to `DPLAYX_LOG_FILE` on load, which gives a yes/no signal without reaching the MP menu.
- Removed `"dplayx"="native"` from `[Software\\Wine\\DllOverrides]` in `pfx/user.reg`. Backup: `pfx/user.reg.pre-override-test`.
