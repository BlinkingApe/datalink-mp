# A Joiner leaving tells the host's game its session was lost

Type: task
Status: resolved
Blocked by:

## What happened

Found on `v0.1.0-rc.5` (2026-10-04), while running [ticket 01](01-confirm-ticket-06-fix-in-a-real-game.md)'s real-game check. The host closed the game from the Multiplayer Setup screen and quit it. The Joiner cancelled and quit too. Both Helpers stayed connected to each other. The host started the game again and hosted a new game, and the Joiner's Join Game said **"no game was found"**.

This is probably also the cause of the old release-pipeline ticket 08, one of the defects [ticket 02](02-shared-root-cause-across-08-09-10.md) is investigating. There, the Joiner cancelled out of a found game while the host stayed in Multiplayer Setup, and "game not found" stayed even after the Joiner restarted its game and its Helper.

## Cause, found in the code and reproduced at the Transport level

Two Helper defects add up to this:

1. **A Joiner's close is sent as the end of the whole session.** `Transport::close_session` (`crates/iroh-transport/src/runtime.rs`) broadcasts `Message::SessionClosed { reason: "Host closed session" }` whatever this Helper's role is. A Joiner's Helper closes when its game calls `Close`, and also from `SessionController::dll_disconnected` when the game quits without closing (the ticket 06 fix). Either way, every peer is told the session is over. The host's Helper turns that into `DPSYS_SESSIONLOST` for the host's game (`convert_received_to_queued`, `ReceivedMessage::SessionClosed`). Real DirectPlay does differently: a non-host's Close sends `DPSYS_DESTROYPLAYERORGROUP` for its players, and only the host going away loses the session.
2. **Messages outlive the session they belong to.** The game's inbox (the Transport's `message_rx` channel and its `message_queue`) is never emptied when a session closes or a new one starts. When the host's game has already quit, the Joiner's `SessionClosed` waits there. The next game the host starts is handed it once it has hosted again.

The likely chain in today's run: the host's game hosts again and reads the stale `DPSYS_SESSIONLOST`. It stops hosting (unconfirmed: probably by calling `Close`). The host's Helper then answers the Joiner's `SessionQuery` with nothing, because only a hosting Helper answers one. The Joiner finds no game. In ticket 08's run, the same message reached the live host game directly, with the same result.

A third, smaller oddity, unconfirmed whether it matters: a Joiner's Helper that receives the host's `SessionClosed` queues `DPSYS_SESSIONLOST` for its game but keeps its own session state until its game closes.

## Tests

Both are in `crates/iroh-transport/tests/mesh_networking.rs`, marked `#[ignore = "fails until game-session-sync ticket 03 is fixed"]`. Remove the `#[ignore]` as part of the fix. Run them with `cargo test -p iroh-transport --test mesh_networking -- --ignored`.

- `test_joiner_that_closes_tells_the_host_its_player_left_not_that_the_session_is_lost` fails today: the host's game hears `DPSYS_SESSIONLOST` and never a `DESTROYPLAYERORGROUP` for the Joiner.
- `test_messages_from_a_closed_session_do_not_reach_the_next_game_the_host_hosts` fails today: after both close and the host hosts again, the new session's game is handed `DPSYS_SESSIONLOST` (0x31).

## What to build

A suggested shape, not yet decided:

- [x] In `close_session`, broadcast `SessionClosed` only when this Helper is the host. A Joiner instead broadcasts `PlayerLeft` for its own player(s) still in the session, the way `destroy_player` does (without queuing anything for its own game, which is leaving), and then closes.
- [x] On receipt, act on `SessionClosed` only when it comes from the session's host. That way a fixed host is safe from a Joiner still running `-rc.5` or earlier, which matters while testers mix builds. The wire format doesn't change, so the Peer protocol version stays the same.
- [x] Begin each Game session with an empty inbox: discard what's in `message_rx` and `message_queue` (and reset `createplayerorgroup_sent`) when `create_session` runs and at the start of `join_session`, before the `JoinRequest` goes out. Anything waiting at that point belongs to an earlier session.
- [x] Both tests above pass without `#[ignore]`, and the full suite still passes.
- [x] Confirm in the real game, on a new `-rc.N`: today's sequence (both games leave Multiplayer Setup and quit, the Helpers stay connected, the host hosts again) lets the Joiner find and join the game. Also ticket 08's sequence (the Joiner cancels out of a found game while the host stays in Multiplayer Setup, then joins again).

Helper logs (`SMAC_HELPER_LOG_FILE`) from the host would confirm the unconfirmed step: a `CloseSession` right after the new game's session was created.

## Answer

**2026-10-04 (agent):** Fixed in the Helper, in the shape suggested above, plus two guards. Real-game confirmation is still to do; it moves to the tickets that wait on it (below).

- **A Joiner leaving no longer ends the session.** `Transport::close_session` broadcasts `SessionClosed` only when this Helper hosts. A Joiner broadcasts `PlayerLeft` for its own player instead and queues nothing for its own game. With no session, it sends nothing.
- **Only the host can end a session.** A received `SessionClosed` is acted on only when it comes from our session's host (`handle_peer_message`). A Helper from `-rc.5` or earlier still sends one when its Joiner leaves; a fixed host ignores it, so its game no longer hears `DPSYS_SESSIONLOST`. It also hears no `DESTROYPLAYERORGROUP` for that Joiner, which is accepted while builds are mixed. The wire format is unchanged and the Peer protocol version stays the same.
- **Each Game session starts with an empty inbox.** `create_session` (once it succeeds) and the start of `join_session` discard everything in `message_rx` and `message_queue` and reset `createplayerorgroup_sent`.
- **Guard (added): a `PlayerLeft` must belong to our session.** It is acted on only for a player in our session, and only when it comes from that player's own Helper or from the host. This covers what emptying the inbox can't: a late message from an old session arriving after the new one starts, whose DPID may by then belong to a new Joiner (the host hands out `0x20000` again).
- **Guard (added): leaving a session forgets its host.** `SessionManager::close_session` clears `host_node_id`. Kept, a former host's later `SessionClosed` or lost connection would be read as the end of the next session.

The two tests pass without `#[ignore]`. They fail without the fix. `cargo test --workspace` passes.

Not changed: the "third oddity" above. A Joiner whose host closed still keeps its session state until its own game closes. With the guards in place, its late `PlayerLeft` reaches no one who would act on it.

**Still to confirm in the real game, on a new `-rc.N`:**
- Today's sequence (both games leave Multiplayer Setup and quit, the Helpers stay connected, the host hosts again, the Joiner joins): rerun as part of [ticket 01](01-confirm-ticket-06-fix-in-a-real-game.md), which needs that exact run anyway.
- Ticket 08's sequence (the Joiner cancels out of a found game, then joins again): goes to [ticket 02](02-shared-root-cause-across-08-09-10.md), which owns 08.


**2026-10-05 (maintainer, recorded by agent):** Confirmed on `v0.1.0-rc.6`. After both games left Multiplayer Setup and quit with the Helpers still connected, the host hosted again and the Joiner found and joined the game ([ticket 01](01-confirm-ticket-06-fix-in-a-real-game.md)). The Joiner cancelling out of a found game, then joining again, works every time too, including after the Joiner restarts its game or its Helper ([ticket 02](02-shared-root-cause-across-08-09-10.md)).
