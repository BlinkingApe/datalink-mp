# 08: Step 1: Game folder self-check and Wine setup text

**What to build:** Step 1 of the page tells the player whether the Helper is sitting in their Game folder next to the DLL and the game. If it is not, a banner says so, shows where the Helper is running from, and says what to do. On Linux and macOS the step also shows the Wine override with Copy buttons.

Scope, from the spec's "Game folder self-check", "Shared status and banners", "The page" and "Wine setup text" sections:

- **The check.** The Game folder is the folder containing the Helper's own executable, found through `current_exe()`, never the working directory. The check passes when the folder contains `dplayx.dll` and at least one of `thinker.exe` or `terran_PRACX.exe`. File names are compared without regard to case.
- **The result** reports the folder path, whether the DLL was found, and which game executable was found. It is evaluated at startup and again on status requests, so restoring a file clears the banner without a restart. It is a status field.
- **Library configuration** gains the Game folder path. The binary fills it from `current_exe()`; tests fill it with a temporary directory.
- **`not_game_folder` banner**, a condition banner: set while the check fails, cleared when it passes. Text: "This isn't your Game folder. Extract the whole archive into the folder that contains your game, then run datalink-mp from there." It shows the folder the Helper is running from. If only the DLL is missing, it adds: "your antivirus may have quarantined `dplayx.dll`; restore it or extract the archive again."
- **A failed check blocks nothing.** The Ticket is still shown.
- **The page.** Step 1 is done when the self-check passes. It shows the self-check result. If the check fails, the banner appears in this step instead. When the OS is not Windows, step 1 also shows the Wine setup text:
  - Intro: "Wine uses its own dplayx.dll unless you tell it to use ours. Add this to your launcher's environment."
  - Generic, with Copy: `WINEDLLOVERRIDES="dplayx=n,b"`, placed before the launch command.
  - Faugus, with Copy: `WINEDLLOVERRIDES=dplayx=n,b`, unquoted, pasted into the game's "Game Arguments" field, separated by a space from anything already there.
  - One line with no button: "Other launchers (Steam, Lutris…) should work: add the override to the launch environment, e.g. Steam launch options `WINEDLLOVERRIDES="dplayx=n,b" %command%`."
  - macOS adds: "Any Wine on macOS should work (e.g. CrossOver). Bottles live under the hidden Library folder: in Finder choose Go → Go to Folder (⇧⌘G)." and is labelled untested.
  - Closing hint: "If the game never shows 'Game connected', this override is the usual cause."
- The Helper never writes files: it only shows the override text.

This ticket introduces the banner list rendering under the header if no earlier ticket has.

Tests use the seam 1 harness with a temporary directory as the Game folder and assert on status fields.

**Blocked by:** 05 (UI mode tracer bullet)

**Status:** ready-for-agent

- [ ] An empty folder fails the check and status carries `not_game_folder`
- [ ] A folder with only the DLL fails, and the result says the DLL was found and no game executable was
- [ ] A folder with only a game executable fails, and the result says the DLL is missing
- [ ] A folder with the DLL and `thinker.exe` passes; so does one with the DLL and `terran_PRACX.exe`
- [ ] Differently-cased file names pass
- [ ] Adding the missing file clears the banner on a later status request, without a restart
- [ ] The result reports the folder path, and the folder comes from the executable's location, not the working directory
- [ ] With the check failing, the Ticket is still present in status
- [ ] The page ticks step 1 when the check passes and shows the banner with the folder path when it fails, with the quarantine sentence only when the DLL alone is missing
- [ ] The Wine setup text with its two Copy buttons appears when the OS is `linux` or `macos`, and not when it is `windows`
- [ ] The macOS-only lines appear only on macOS and are labelled untested
