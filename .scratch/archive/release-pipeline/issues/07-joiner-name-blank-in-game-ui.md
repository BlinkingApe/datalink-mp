# 07: Joiner's in-game player name is blank

**What happened:** Found during the 0.1.0 gate, testing `v0.1.0-rc.4`'s archives. In every test (Linux-Linux: Mint hosting on wifi, Rocky joining on a mobile hotspot; and Windows-Linux: the maintainer's own Windows 11 machine hosting on wifi, Rocky joining on a mobile hotspot), the game's Multiplayer Setup screen showed the Host's name correctly but left the Joiner's row blank, despite the Joiner's name being set. Purely cosmetic so far: both sides could still adjust UI elements and, in the tests that got that far, play a full game.

**Blocked by:** None

**Status:** resolved

Leads, unconfirmed:

- Ticket 06 (the leaked-Game-session defect, since moved to the [game-session-sync map](../../../game-session-sync/map.md)) already found that `GetSessionDesc` can hand the game a session description whose `lpszSessionName` is null (`crates/dplayx/src/directplay.rs`, around line 1468, marked "Caller must handle"). That investigation decided the null name wasn't the cause of the `Net::send` crash it was looking at, and left it as "a known oddity, not a cause." This ticket's repeated, cross-machine blank-name report is evidence that oddity is real and visible to players, even if harmless so far.

Needed:

- [x] Confirm whether the blank name is the Joiner's own row (as seen from the Host) or also shows blank to the Joiner viewing their own row: both, read from the game's code (see Comments)
- [ ] ~~Logs from a reproduction~~: not needed, the game's code showed the cause
- [x] Trace why the Joiner's name arrives blank, and fix it: in the Helper (see Comments)
- [x] Confirm in the real game, on a new `-rc.N`: the Joiner's name shows on its row, on both the Host's and the Joiner's screens

## Comments

**2026-10-04 (agent):** Cause found by disassembling the game (`terran.exe`, the binary the DLL's probe addresses target). It's the Helper's timing, not the session name.

- The session name lead was a red herring: `lpszSessionName` is the game's name, not a player's.
- The host's game registers a player once, when it handles that player's `CREATEPLAYERORGROUP` (0x64d229): it calls `GetPlayerName` and copies the `lpszLongName` into its player table (`RegisterPlayerInternal` @0x64d020). A player already in the table is never renamed.
- The game's `SETPLAYERORGROUPNAME` handler (its listener's vtable slot 3, 0x4f8b00) is a bare `ret 8`: renames are ignored.
- Joiners don't register players from `CREATEPLAYERORGROUP` at all. They copy every name from the host's roster, so the host's blank reaches every Joiner too, including the Joiner's own row.
- The game's `CreatePlayer` passes only a long name (short name null).
- The Helper told the host's game about the Joiner at the JoinRequest, which the Joiner sends at its Open, before its `CreatePlayer` names the player. The host's `GetPlayerName` answered `""`, the game stored that, and the `SETPLAYERORGROUPNAME` that followed did nothing.
- Fix: the host's Helper adds the Joiner to the session at the JoinRequest as before, but holds it back from the host's game until the Joiner's first name update (sent by its `CreatePlayer`) arrives, then announces it, already named. That's the order real DirectPlay uses. The eligibility byte is still baked in at the JoinRequest. The Joiner's game messages follow the name update on the same ordered stream, so the host's game always knows the Joiner before hearing from it. A Joiner that leaves before `CreatePlayer` produces a `DESTROYPLAYERORGROUP` for a player the host's game never knew, which the game ignores (0x64c690 returns early).
- Test: `test_host_game_hears_of_joiner_at_its_create_player_already_named` (`crates/iroh-transport/tests/mesh_networking.rs`). Background in `docs/ARCHITECTURE.md`.

**2026-10-04 (agent):** Reviewed before `v0.1.0-rc.5`; the fix ships unchanged. Things to watch in the real-game confirm, none believed to matter for SMAC:

- Only the host's game's CREATEPLAYERORGROUP is held back. Between the JoinRequest and the joiner's CreatePlayer (milliseconds), the host's `EnumPlayers`, `GetPlayerName` and the session's player count already include the nameless joiner, and so does the roster a later joiner gets in its `JoinResponse`. That was also true before the fix.
- Other joiners' Helpers still hand `PlayerJoined` to their games at the JoinRequest, unnamed. By the disassembly above, joiners don't register players from CREATEPLAYERORGROUP but copy the host's roster, so this should be harmless. A game with three or more players would confirm it: every row named on every screen.
- A joiner whose connection drops without a `PlayerLeft` is never removed from the host's roster. That's the same for named joiners and belongs to the [game-session-sync map](../../../game-session-sync/map.md), not this ticket.

**2026-10-04 (maintainer, recorded by agent):** Confirmed on `v0.1.0-rc.5`: the Joiner's name now shows. Closing.
