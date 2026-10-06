# 03: The quarantine banner says the folder is right

**What to build:** When the self-check finds the game executable but not `dplayx.dll`, the not_game_folder banner still opens with "This isn't your Game folder. Extract the whole archive into the folder that contains your game…", and only then adds the quarantine sentence. A player reads that as "wrong folder", although the folder is right. In that case the banner should say plainly that this looks like the Game folder and only `dplayx.dll` is missing, probably quarantined by antivirus: restore it or extract the archive again. When the game executable is missing, the banner keeps today's "This isn't your Game folder" wording. Which of the two wordings shows is decided as the quarantine hint is today, from the self-check's results.

From the [post-0.1.0-polish map](../../post-0.1.0-polish/map.md): [Clarify the quarantine sentence in the not_game_folder banner](../../post-0.1.0-polish/issues/03-quarantine-banner-wording.md).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Game executable found, DLL missing: the banner says the folder looks right and only `dplayx.dll` is missing, with no "This isn't your Game folder"
- [ ] Game executable missing: today's wording, unchanged
- [ ] The UI tests cover both wordings
