# 07: Joiner's in-game player name is blank

**What happened:** Found during the 0.1.0 gate, testing `v0.1.0-rc.4`'s archives. In every test (Linux-Linux: Mint hosting on wifi, Rocky joining on a mobile hotspot; and Windows-Linux: the maintainer's own Windows 11 machine hosting on wifi, Rocky joining on a mobile hotspot), the game's Multiplayer Setup screen showed the Host's name correctly but left the Joiner's row blank, despite the Joiner's name being set. Purely cosmetic so far: both sides could still adjust UI elements and, in the tests that got that far, play a full game.

**Blocked by:** None

**Status:** ready-for-agent

Leads, unconfirmed:

- [ticket 06](06-null-pointers-in-the-game-setup-dialogs.md) already found that `GetSessionDesc` can hand the game a session description whose `lpszSessionName` is null (`crates/dplayx/src/directplay.rs`, around line 1468, marked "Caller must handle"). That ticket decided the null name wasn't the cause of the `Net::send` crash it was investigating, and left it as "a known oddity, not a cause." This ticket's repeated, cross-machine blank-name report is evidence that oddity is real and visible to players, even if harmless so far.

Needed:

- [ ] Confirm whether the blank name is the Joiner's own row (as seen from the Host) or also shows blank to the Joiner viewing their own row
- [ ] Logs from a reproduction: `DPLAYX_LOG_FILE=/path/dplayx.log` and `SMAC_HELPER_LOG_FILE=/path/helper.log`
- [ ] Trace why `lpszSessionName` (or whichever field carries the player name here) arrives null on the Joiner's entry specifically, and fix it
