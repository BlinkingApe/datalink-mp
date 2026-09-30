# 16: Player documentation

**What to build:** A player who lands on the repository, or opens the text file in the archive, finds a quickstart for their OS that gets them from "extract" to "playing" without a terminal. The documentation claims only what has been tested.

Scope, from the spec's "Documentation" section (ADR-0003 for the name and credit):

- **The repository README** is rewritten for players, with the credit line "Based on smac-iroh by Henry de Valence" and a per-OS quickstart.
- **The existing build-and-install walkthrough** becomes the build-from-source document.
- **The architecture document** gets the new name.
- **`datalink-mp-README.txt`**: the same quickstart as plain text, to ship in the archives. Building the archives belongs to the release pipeline spec; this ticket only adds the file to the repository.
- **Standing rule: claim only what was tested.** Windows, and Linux through Faugus and the shell under GE-Proton, are tested. Steam, Lutris and plain system Wine are "should work". macOS is labelled untested throughout.
- **Quickstart wording:**
  - **All systems, first:** "You need Sid Meier's Alpha Centauri with Thinker or PRACX. Everyone playing needs the same release of datalink-mp."
  - **Windows:** (1) Extract the whole zip into your Game folder, the one containing `thinker.exe` or `terran_PRACX.exe`. (2) Double-click `datalink-mp.exe`. Windows will say it doesn't recognise the app: choose More info → Run anyway. If the firewall asks, allow access. (3) Your browser opens the datalink-mp page. Follow the four steps. (4) Keep the console window open while you play.
  - **Linux:** (1) Extract the whole `.tar.gz` into your Game folder. (2) In your launcher, add the Wine override: `WINEDLLOVERRIDES="dplayx=n,b"` (Faugus: paste it unquoted into Game Arguments). (3) Double-click `datalink-mp` and choose Run. (4) Your browser opens the page. Follow the four steps.
  - **macOS (untested):** (1) Extract the zip into your Game folder inside your Wine bottle. (2) Add the Wine override to your launcher. (3) Double-click `datalink-mp`. macOS will block it the first time: open System Settings → Privacy & Security, choose Open Anyway, enter your password, then double-click it again. (4) A Terminal window opens and must stay open. Your browser opens the page.
  - **Troubleshooting entries:** Smart App Control blocks unsigned apps and the only workaround is turning it off; how to restore a quarantined `dplayx.dll`; the game never shows "Game connected" (the override); your friend can't connect (ask for the current Ticket; check both have the same release).
- Power users: a short section on `datalink-mp host`, `datalink-mp join --ticket`, `--ui-port`, `--port`, `--no-browser` and the environment variables.
- The SmartScreen screenshot and the release notes belong to the release pipeline spec.

Use the glossary in `CONTEXT.md`: Helper, DLL, Ticket, Game folder, Stop, Release version.

**Blocked by:** 01 (Rename the Helper to `datalink-mp`)

**Status:** resolved

- [ ] The README opens with what the project is for a player, carries the credit line, and has the all-systems line and the three per-OS quickstarts with the wording above
- [ ] The README has the four troubleshooting entries
- [ ] Steam, Lutris and plain system Wine are described as "should work", and every macOS passage is labelled untested
- [ ] No passage claims a launcher or OS was tested beyond Windows and Linux through Faugus and the shell under GE-Proton
- [ ] The build-and-install walkthrough is now the build-from-source document, linked from the README, and uses the `datalink-mp` name
- [ ] The architecture document uses the `datalink-mp` name
- [ ] `datalink-mp-README.txt` exists as plain text with the same quickstart and troubleshooting content
- [ ] No document tells a player to type a command in order to play
- [ ] The documents use the glossary's terms and none of its "avoid" words
