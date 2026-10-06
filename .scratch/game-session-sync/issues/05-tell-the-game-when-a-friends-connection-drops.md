# Tell the game when a friend's connection drops

Type: task
Status: needs-triage
Blocked by:

Not before `0.1.0`: [ticket 02](02-shared-root-cause-across-08-09-10.md) accepted both defects below for that release, as known issues.

## What happens

Seen on `v0.1.0-rc.6` (2026-10-05):

- **09:** a Joiner's Helper goes away mid-game (the Joiner quits it, or presses Stop). The host's game isn't told. Its next send to that player shows "Oh no!! Send failed!", and a few seconds later the game's own "Player … not responding". Choosing Drop recovers. Nothing appears until the host's next action, so a host who waits sees nothing at all.
- **Killed host:** the host's game is killed while a Joiner sits in Multiplayer Setup. The Joiner's game stays on the setup screen, never told the session ended. No crash. Once the host hosts again, the Joiner can join without restarting anything.

## Leads, from reading the code (unconfirmed without logs)

- **09:** when a non-host peer's connection is lost, the host's Helper re-dials it up to five times and does nothing else (`convert_received_to_queued`, `ReceivedMessage::ConnectionLost`, `crates/iroh-transport/src/runtime.rs`). The loss also removes the player's route, so the game's next send fails with `NotConnected`. A possible fix: once the re-dial gives up, queue `DESTROYPLAYERORGROUP` for that peer's players and drop them from the session, as a received `PlayerLeft` does. Real DirectPlay sends it when a player times out. Unknown: whether SMAC takes a mid-game `DESTROYPLAYERORGROUP` quietly. In Multiplayer Setup it does.
- **Killed host:** the host's Helper closes the session (`dll_disconnected` → `close_session`) and sends `SessionClosed`. The Joiner's Helper turns that into `DPSYS_SESSIONLOST` for its game, which, in Multiplayer Setup, the game seems to ignore. A `dplayx.log` from the Joiner would show whether the message was handed over. If it was, SMAC may simply not act on it in the setup screen, and there may be nothing to fix.

## Needed

- [ ] Logs from a reproduction of each, on both sides: the Helper started from the terminal that sets `SMAC_HELPER_LOG_FILE`, and `DPLAYX_LOG_FILE` given a `Z:\…` path under Wine.
- [ ] Reproduce 09 at the Transport level: a Joiner's connection lost mid-session gives the host's game a `DESTROYPLAYERORGROUP`.
- [ ] Decide the fix for each, then confirm it in a real game.

## Comments

2026-10-06: the traffic capture built for internet-play-speed records what this ticket's logs were wanted for, on the capturing side:
- `sys` lines: each system message the DLL drained for the game, such as `DESTROYPLAYERORGROUP` or `SESSIONLOST`, or no line if the game never got one;
- `lost` and `reconnect` lines: when the Helper noticed the loss, and each re-dial;
- `send_failed` lines: the game's sends to a player whose route is gone;
- `ctl` lines: `SessionClosed` and `PlayerLeft` between Helpers.

It's on branch `capture/traffic-capture`, documented in `docs/traffic-capture.md`. [Play a captured internet game](../../internet-play-speed/issues/04-play-a-captured-internet-game.md) adds both reproductions after its speed game, with the capturing machine on the side whose friend drops. That covers the first Needed item for one side. The other side's `dplayx.log` is still needed if a lead stays unconfirmed. The Transport-level reproduction and the fix are still this ticket's own work.
