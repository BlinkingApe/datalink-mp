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

- **09:** when a non-host peer's connection is lost, the host's Helper re-dials it up to five times and does nothing else (`convert_received_to_queued`, `ReceivedMessage::ConnectionLost`, `crates/datalink-transport/src/runtime.rs`). The loss also removes the player's route, so the game's next send fails with `NotConnected`. A possible fix: once the re-dial gives up, queue `DESTROYPLAYERORGROUP` for that peer's players and drop them from the session, as a received `PlayerLeft` does. Real DirectPlay sends it when a player times out. Unknown: whether SMAC takes a mid-game `DESTROYPLAYERORGROUP` quietly. In Multiplayer Setup it does.
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

It was built in [internet-play-speed 03](../../internet-play-speed/issues/03-dev-only-traffic-capture.md) (commit `b90a677`), documented in `docs/contributors/traffic-capture.md`. [Play a captured internet game](../../internet-play-speed/issues/04-play-a-captured-internet-game.md) adds both reproductions after its speed game, with the capturing machine on the side whose friend drops. That covers the first Needed item for one side. The other side's `dplayx.log` is still needed if a lead stays unconfirmed. The Transport-level reproduction and the fix are still this ticket's own work.

2026-10-06: both reproductions were captured on the Linux side, in [Play a captured internet game](../../internet-play-speed/issues/04-play-a-captured-internet-game.md). The file is `capture-1791300326-1036127.jsonl` in `.scratch/internet-play-speed/captures/` (gitignored). There are no DLL logs and no Helper log from this run, and Windows ran stock `v0.1.0` with no capture. Times are the Linux machine's.

**09, a Joiner's Helper goes away mid-game** (Linux hosts, Windows joins, Direct connection):
- 17:44:02: last traffic. Each side was still sending its 2-second keep-alive (JACKAL kind 32). It was the Joiner's turn.
- 17:44:04.209: the connection closed. 17:44:04.217: `lost`, `host: false`.
- 17:44:19, 17:44:35, 17:44:52, 17:45:09, 17:45:26: `reconnect` attempts 1 to 5, all failed, about 16–17 s apart. The Helper gave up 82 s after the loss.
- **The game was handed nothing:** no `sys` line from the loss to the end of the capture.
- **No `send_failed` either.** The host's game kept no direct sends going during the Joiner's turn, only its keep-alive, which goes to `DPID_ALLPLAYERS`. A broadcast with no peers left returns `Ok` without sending (`broadcast_filtered`, `crates/datalink-transport/src/connection.rs:522`). So the game got no error at all. "Send failed" only shows when the host has a direct send to make, which is why C3 couldn't be reproduced on the Joiner's turn.
- The maintainer saw SMAC's own "Player … not responding" popup within about 30 s (exact time not noted). That's the game's own timeout. Drop recovered, and the game carried on.

This confirms the lead: the host's Helper re-dials, and the game is never told. Still unknown: whether SMAC takes a mid-game `DESTROYPLAYERORGROUP` quietly.

**Killed host** (Windows hosts, Linux joins and waits in Multiplayer Setup):
- 17:48:56: Linux joined (`SETSESSIONDESC` and two `CREATEPLAYERORGROUP` drained).
- 17:49:50.722: `ctl in SessionClosed` from the Windows Helper, which was still running.
- 17:49:50.734: **the game drained `SESSIONLOST`**, 12 ms later.
- 17:49:50.745: the Linux Helper sent `PlayerLeft` for its own player (131072). The Helper's `SessionClosed` handler only queues `SESSIONLOST`. A Joiner sends `PlayerLeft` only when its game calls `Close` (`close_session`) or `DestroyPlayer` on its own player (`destroy_player`). **So SMAC acted on `SESSIONLOST`: it left the session within 11 ms, but its setup screen didn't change.** The maintainer saw no response there for 30 s.
- 17:50:57 to 17:51:11: Windows hosted again, and Linux rejoined after cancelling its old setup screen, without restarting anything. 17:52:02: Linux's `PlayerLeft` as it quit, which Windows showed as "Player Rocky dropped".

On the killed-host side, the Helper and DLL do their part: the message is handed over, and the game reacts to it. What's left is SMAC's setup screen showing nothing, so there may be no Helper fix. Whether the page should say the session ended is for this ticket to decide.
