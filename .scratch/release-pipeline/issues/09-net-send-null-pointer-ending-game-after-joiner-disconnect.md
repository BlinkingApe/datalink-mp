# 09: Net::send NULL pointer when the Host ends a game after an unnotified Joiner disconnect

**What happened:** Found during the 0.1.0 gate, in the internet game against `v0.1.0-rc.4`'s archives (2026-10-02): the maintainer's own Windows 11 machine hosting on wifi, the maintainer's own Linux laptop (Rocky) joining on a mobile data hotspot. The game connected, and the two played several turns at a speed comparable to the earlier Linux-Linux hotspot test. The Joiner then quit the game, possibly without the Host being notified. When the Host clicked "End Game", a white error popup titled `Net::send`, "NULL pointer", appeared — the same error signature as [ticket 06](06-null-pointers-in-the-game-setup-dialogs.md), but on a different path (ending an in-progress game, not re-hosting after one). The maintainer's guess: the Host hadn't yet registered the Joiner's disconnect and tried to send a disconnect to a peer that was already gone.

This blocks [release-pipeline ticket 04](04-release-gate-and-publish-0-1-0.md)'s gate on `v0.1.0-rc.4`: per that ticket's rule, a defect found in the gate's real game means a new `-rc.N` and a fresh gate.

**Blocked by:** None

**Status:** ready-for-human

Leads, unconfirmed:

- Likely the same family of bug as ticket 06 (session/peer teardown not fully handled before the game code touches it), but in the disconnect-while-playing path rather than the leaked-session-on-restart path ticket 06 fixed.
- Whether the Helper notices a peer's iroh connection closing promptly, and whether that's surfaced to the DLL/game before "End Game" is allowed to send to that peer.

Needed:

- [ ] Logs from a reproduction: `DPLAYX_LOG_FILE=/path/dplayx.log` and `SMAC_HELPER_LOG_FILE=/path/helper.log` on the Host, covering the Joiner's quit and the Host's "End Game"
- [ ] Reproduce at the IPC/Transport level (as ticket 06 did) rather than relying on a real two-machine game each time
- [ ] Fix, then confirm in a real game: Joiner quits mid-game, Host clicks End Game, no `Net::send` error
