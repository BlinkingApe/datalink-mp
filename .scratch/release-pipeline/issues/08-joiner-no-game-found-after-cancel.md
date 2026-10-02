# 08: Joiner gets "no game found" after cancelling once, even after a full restart

**What happened:** Found during the 0.1.0 gate, testing `v0.1.0-rc.4`'s archives, Linux-Linux (Mint hosting, Rocky joining). The Joiner found the Host's game, then pressed Cancel on the Multiplayer Setup screen. Trying again from the game's menu (Multiplayer → Iroh P2P → Join Game) gave "game not found". The Joiner then closed the game, stopped and quit the Helper, reopened the Helper, and re-pasted the same Ticket from the Host — whose Helper and game had never been interrupted and was still sitting in Multiplayer Setup the whole time. Still "no game found".

Only the Joiner side was cycled; the Host's session was never closed or restarted, so this looks distinct from [ticket 06](06-null-pointers-in-the-game-setup-dialogs.md)'s leaked-session bug (which is about the Host's own next hosting attempt failing). It may be a DirectPlay session-discovery or lobby state issue on the Joiner's side that a Cancel doesn't clean up.

**Blocked by:** None

**Status:** needs-triage

Needed:

- [ ] Logs from a reproduction: `DPLAYX_LOG_FILE=/path/dplayx.log` and `SMAC_HELPER_LOG_FILE=/path/helper.log` on both sides
- [ ] Confirm whether the Helper's own peer connection (iroh) survives the Joiner's Cancel, or whether it's the game's DirectPlay session enumeration that's stuck
- [ ] Try the same sequence without a full Helper restart (just Cancel, then Join Game again) to narrow down whether quitting the Helper is necessary to reproduce
