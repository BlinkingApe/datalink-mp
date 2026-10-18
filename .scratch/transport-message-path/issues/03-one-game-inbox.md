# 03: One game inbox

**What to build:** Everything the game will be handed sits in one **game inbox** per Transport: a single ordered queue. It replaces the channel plus queue and the CREATEPLAYERORGROUP set that sits on the Transport.

- **What can be added:**
  - a game message from a friend, with its capture id
  - the host's local loopback to the server player
  - a system event: player joined, player left, session lost, or an already-built system message
- **System messages:** the inbox builds the DirectPlay system message for each event. Callers pass in the roster facts it needs, such as the player count, or a leaving player's name and flags looked up before removal. The inbox never reads the session manager.
- **Dedup:** a second "player joined" for the same player in one Game session isn't added.
- **Game session began:** empties the inbox and resets the dedup.
- **Take and count:** these are the DLL's Receive and GetMessageCount, and they agree. Take writes the capture's `drained` or `system_drained` line, as today.
- **Removed sends:** the session discovery and session list notices, which today are queued and then dropped, are no longer sent.

The handle is shared between the IPC thread and the runtime's reader and cleanup tasks. It has one short synchronous lock, never held across an await or while another module's lock is taken.

See the spec, [The game inbox](../spec.md#the-game-inbox).

**Blocked by:** 02

**Status:** resolved

- [x] In-process inbox tests:
  - [x] items come out in the order added, across friend messages, loopback and system events
  - [x] a duplicate "player joined" is not added
  - [x] Game session began empties it and allows the same player to join again
  - [x] count matches take
  - [x] each system message matches today's bytes
- [x] The Transport no longer has a channel stage or its own dedup set
- [x] The game receives exactly the bytes sent, in order: the end-to-end delivery, closed-session and Joiner-leaving tests pass unchanged
- [x] Transport tests that only checked inbox ordering or dedup move to the inbox's tests, rather than being kept twice
- [x] `cargo test` passes, including the Helper's UI tests

## Comments

Built `crates/datalink-transport/src/inbox.rs`: `GameInbox` (one ordered queue, dedup set, `session_began`, `take`, `count`, `SystemEvent` builders) replacing the Transport channel, queue and CREATEPLAYERORGROUP set. The connection manager and Transport add to it directly. `ReceivedMessage` is gone, and the discovery/session-list sends with it. Roster facts (player count, leaver name/flags) are looked up by callers. Inbox unit tests cover order, dedup, session-began, count/take and system-message bytes.
