# 03: Clarify the quarantine sentence in the not_game_folder banner

**What to build:** Idea from helper-web-ui ticket 17's manual check (2026-10-02). The `not_game_folder` banner's quarantine sentence ("your antivirus may have quarantined `dplayx.dll`; restore it or extract the archive again") reads, on first encounter, like the generic "this isn't your Game folder" message — even though the folder is in fact correct and only the DLL is missing. Worth wording it to say plainly that the folder looks right and only `dplayx.dll` is the problem, so a player doesn't assume they extracted to the wrong place.

**Blocked by:** None

**Status:** resolved: decided while charting the map. Nothing to decide; it gets built once `/to-tickets` turns this map into build tickets.

Not a blocker for 0.1.0. Carried over from helper-web-ui ticket 24 onto the [post-0.1.0-polish map](../map.md).
