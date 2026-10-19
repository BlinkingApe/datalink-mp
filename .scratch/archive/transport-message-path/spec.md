# Spec: the Transport's message path, ready for Early acks

Status: ready-for-agent

Date: 2026-10-18. Comes from the architecture review of the same date (candidates A, B and C). Builds toward [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) and the Early ack tickets in [internet-play-speed-build](../../internet-play-speed-build/issues/).

## Problem Statement

The next planned work is **Early acks** (internet-play-speed-build tickets 07–10, then 12). It has to hook into the Transport's game-message path in about six places. Today that path has no owner:

- **"A friend" is spread over four places, each with its own lifetime:**
  - the per-friend connection entry, which holds the outbox
  - a process-wide Turn sync map, registered and unregistered separately
  - the traffic capture's process-wide ids
  - path tracking

  Early acks would add a fifth. Each of their hooks needs state that sits in a different module:
  - making an Early ack needs the game's inbox
  - ticket 09's `early_ack` line needs the capture id of the `out` line, which never leaves the per-friend enqueue
  - ticket 09's Turn sync log fields need counts that live elsewhere
- **The game's inbox has two stages,** a channel and a queue, written from 17 places. Decisions run only when the DLL polls, and while both inbox locks are held:
  - writing the capture's `lost` line
  - deciding that the host is gone (`SESSIONLOST`)
  - starting the five-attempt reconnect
- **A bug follows from that.** When a Game session begins, the inbox is emptied without looking at what it drops. A pending connection-lost notice is thrown away, and with it the reconnect and the capture's `lost` line. Trigger: a friend's connection drops just before the player clicks Host Game or Join Game, and the friend's Helper is never re-dialled.
- **The ordered writer has no seam.** No test can make a stream write fail. Ticket 08's test ("a reset stream replays in order") needs ticket 10's `reset:N` fault, while ticket 10 is listed as blocked by 08.
- **The process-wide Turn sync map is shared by every Transport in a test binary.** Two Transports watching the same friend overwrite each other's detector. Today that only affects logs. It becomes wrong once ticket 09's counts and ticket 12's page activity read that state for one Helper.
- **Three copies of the JACKAL header parsing,** with different rules, and Early acks would add a fourth.

For maintainers and agents, this means the Early ack rules would land as hooks across two large files. They would share a ledger between three threads. They could only be tested with real iroh endpoints and polling loops. And there is a lock-order trap: a Send that holds the Early ack lock while emptying the inbox would deadlock against a connection-lost handler that takes the same lock.

## Solution

Restructure the Transport's game-message path into three modules **before** the Early ack tickets, and keep today's behaviour, except for the bug fix and the earlier reconnect noted below.

1. **The friend pipeline:** one module per connected friend, per Transport.
   - It sees every outbound game message to that friend, every inbound game message from them, and the lifecycle events: Game session began, connection lost.
   - Behind its interface sit the single JACKAL header decoder, the Turn sync detector for that friend, and the capture lines for game messages, emitted in one fixed order.
   - Ticket 07's Early ack state becomes part of its implementation.
2. **The game inbox:** one module per Transport, holding everything the game will be handed, in the order it is handed over.
   - It owns the order of items, the system messages the game is owed, the CREATEPLAYERORGROUP dedup, and discarding stale items when a Game session begins.
   - What to do about a lost connection is decided when the loss happens, not when the DLL next polls.
3. **The frame sink:** a seam under the ordered writer, with two adapters:
   - the iroh stream, in production
   - an in-memory sink in tests, which can fail on cue

   The writer's rule (reopen once, retry, otherwise close the connection) becomes testable without a network. Ticket 08's replay test uses the in-memory sink, so ticket 08 no longer needs ticket 10.

Players see no change, apart from the fixed reconnect after a loss just before Host Game or Join Game. Peer protocol and IPC versions are unchanged, so this ships in a 0.1.x patch.

## User Stories

1. As a contributor building Early acks, I want one module per friend that sees each outbound and inbound game message, so that the Early ack rules live in one place instead of six hooks across two files.
2. As a contributor building Early acks, I want the friend pipeline's outbound result to carry the message's capture id, so that ticket 09's `early_ack` line can name the `out` line without threading the id back through three functions.
3. As a contributor building Early acks, I want the inbox to keep its own order, so that "the Early ack goes after everything that already arrived from the friend" holds just by adding to the inbox, rather than relying on the caller to empty the channel first.
4. As a contributor building Early acks, I want no module's lock to be held while another module's lock is taken on the message path, so that a Send can't deadlock against a connection-lost handler.
5. As a contributor building Early acks, I want the friend pipeline to hear "Game session began" and "connection lost" as events, so that clearing Early ack state is an event in the pipeline rather than a hook in connection cleanup and session start.
6. As a contributor building Early acks, I want one JACKAL header decoder with named classifications (any reliable data, exactly `0x0004`, exactly `0x0002`), so that the Turn sync detector, the capture and Early acks can't disagree by accident.
7. As a contributor building ticket 08, I want to make a stream write fail in a test, so that I can show a reset stream replays in order and delivers each message once, without ticket 10's cargo feature.
8. As a contributor building ticket 10, I want the fault-injecting adapter to sit at the existing frame-sink seam, so that `drop:N` and `reset:N` only need parsing of the environment variable and a third adapter.
9. As a contributor building ticket 09, I want the Turn sync detector for a friend to live in the same module as that friend's Early ack counters, so that the Turn sync log line can carry hop time and the counts of Early acks made, withheld and swallowed.
10. As a contributor building ticket 12, I want each Transport to own its friends' Turn sync detectors, so that the page can read a Helper's Turn sync state through the Transport instead of a process-wide map.
11. As a contributor writing tests, I want to feed game messages to the friend pipeline and assert on its results with no network, so that ordering and counting rules are tested in milliseconds instead of with two real endpoints and polling loops.
12. As a contributor writing tests, I want to replay the recorded Turn sync fixtures through the friend pipeline, so that the detector's behaviour is checked at the module that callers actually use.
13. As a contributor writing tests, I want to test the inbox by adding items and taking them out, so that ordering, deduplication and session-began discarding are tested without a Transport.
14. As a contributor writing tests, I want two Transports in one test binary to keep separate Turn sync state for the same friend, so that tests with several Transports don't cross-contaminate.
15. As a contributor writing tests, I want the writer's reopen-once rule tested with the in-memory sink, so that a stream failure is a test case rather than something only seen in the field.
16. As a player whose friend's connection dropped just before they clicked Host Game or Join Game, I want the Helper still to re-dial my friend, so that the game doesn't silently go on without them.
17. As a tester reading a capture, I want a lost connection always to write its `lost` line, even when a Game session begins straight after, so that the capture tells the whole story.
18. As a player whose friend's connection drops mid-game, I want the reconnect to start as soon as the loss is seen, not when my game next polls, so that the mesh heals as early as it can.
19. As a Joiner whose host's connection drops, I want my game told the session is lost exactly as today, so that the game's own handling still runs.
20. As a player, I want the game to receive exactly the bytes sent, in the order sent, so that this restructure can't cause a desync.
21. As a player, I want CREATEPLAYERORGROUP still sent once per player per Game session, so that the game doesn't see a friend join twice.
22. As a player starting a new game, I want messages left over from the previous Game session still discarded, so that the new game isn't handed stale messages.
23. As a host, I want my own messages to the server player still to come back to me in order with friends' messages, so that hosting works as it does today.
24. As a maintainer, I want the per-friend state to belong to the per-friend connection entry and to each Transport, so that a friend's state is removed when that entry is removed, in one step.
25. As a maintainer, I want the traffic capture still to be off by default and to cost nothing when off, so that release behaviour is unaffected.
26. As a maintainer, I want the capture's existing lines and format unchanged, so that `analyse_capture.py` and earlier captures stay valid.
27. As a maintainer, I want the Turn sync and path log lines unchanged, so that testers' logs read as before.
28. As a maintainer, I want ADR-0001 §2's threading rules still met (no blocking on the runtime, no lock held across an await, Send stays a plain enqueue), so that the Helper keeps its process structure.
29. As a maintainer, I want only 2–3 Transport-level tests added, so that the slow suite grows only by its wiring checks.
30. As a maintainer, I want tickets 07–10 to point at the new modules, so that the next agent builds inside them instead of reintroducing the hooks.

## Implementation Decisions

### The friend pipeline

- **One per connected friend, per Transport.** It is held on the per-friend connection entry that the connection manager already keeps. It is created and dropped with that entry, so a friend's state has one lifetime.
- **What it is told:**
  - each outbound game message to the friend, as it is queued
  - each inbound game message from the friend, as it is read
  - "Game session began"
  - "connection lost"
- **What it returns:**
  - **For an outbound message:** the outcome for that message and its capture id. In this spec the outcome is always "send it". Ticket 07 adds "drop it as a resend" and "hand the game this Early ack now".
  - **For an inbound message:** the outcome. In this spec it is always "deliver it". Ticket 07 adds "swallow this real ack", plus any withheld Early acks a real ack releases.
- **The pipeline never takes another module's lock.** It returns what should happen, and its caller acts after releasing the pipeline's lock: queueing on the outbox, adding to the inbox. This rule removes the deadlock trap.
- **Behind its interface:**
  - the single JACKAL header decoder
  - the friend's Turn sync detector (the existing detector, unchanged)
  - the capture lines for game messages in and out, in one fixed order
- **Time and RTT are injected,** as the Turn sync detector already does. In production the RTT comes from the friend's connection. In tests it is a closure.
- **The process-wide Turn sync map and its register/unregister glue are removed.** Detection is per Transport. The Turn sync log lines are unchanged.
- **The capture's file stays process-wide.** It is dev-only, and production runs one Helper per process. Only *when* its game-message lines are written moves into the pipeline. Line kinds, fields and format 1 are unchanged.
- **One JACKAL header decoder,** with named classifications:
  - "carries reliable data" (any data bit), which the Turn sync detector and the capture use today
  - "is exactly reliable data `0x0004`" and "is exactly an ack `0x0002`", for Early acks

  Setup kinds `0x14`/`0x12` keep their current classification everywhere. The decoder is the internal JACKAL parser for the Transport. The DLL's own trace parsing is out of scope.
- **Path tracking stays as it is** (its per-connection logger and watcher, and the `peer_paths` read). It isn't on the per-message path.
- **No snapshot interface yet.** Tickets 09 and 12 add one when they need to read counts and Turn sync state.

### The game inbox

- **One per Transport.** It replaces the channel plus queue, and the CREATEPLAYERORGROUP set that currently sits on the Transport. It is a single ordered queue: items come out in the order they went in.
- **What can be added:**
  - a game message from a friend, with its capture id
  - a local loopback message (the host to the server player)
  - a system event for the game: player joined, player left, session lost, or an already-built system message
- **The inbox builds the DirectPlay system message for each system event.** Any roster facts it needs (the player count for "joined", a leaving player's name and flags) are passed in by the caller when the event is added, so the inbox never reads the session manager. Callers look up a leaving player's name before removing them from the roster.
- **CREATEPLAYERORGROUP dedup:** a second "player joined" for the same player in one Game session is not added. The set clears when a Game session begins.
- **"Game session began"** empties the inbox and clears the dedup. With connection-loss policy moved out (below), everything in the inbox is stale at that point, so emptying it can't lose a reconnect or a capture line.
- **Taking and counting:** take-next and count are the DLL's Receive and GetMessageCount. Taking writes the capture's `drained` or `system_drained` line, as today. Count and take agree, with no hidden second stage.
- **Thread safety:** a handle shared between the Transport (the IPC thread) and the connection manager's reader and cleanup tasks (the runtime). It uses one short synchronous lock, never held across an await or while any other module's lock is taken.
- **Session discovery and session list notices are not inbox items.** Today they go into the channel and are dropped at drain time. Those two sends are removed.

### Connection-loss policy moves to the loss

- When a friend's connection ends, the connection manager's cleanup acts straight away, on the runtime:
  - it writes the capture's `lost` line
  - if the lost friend was the host, it adds "session lost" to the inbox
  - otherwise it starts the existing five-attempt reconnect driver, unchanged

  It also tells that friend's pipeline "connection lost" before the entry is removed.
- **This fixes the session-began bug.** It also starts the reconnect when the loss is seen rather than at the next DLL poll. That second change is deliberate and noted in the release notes.
- The cleanup still removes the friend's entry and routes as today. Whether it should check which of a friend's connections ended (review candidate F) is out of scope.

### The frame sink seam

- **The ordered writer writes through a frame sink** that can:
  - open the ordered stream (the magic and Peer protocol version preamble)
  - write one length-prefixed frame
  - close the connection when the writer gives up
- **Two adapters:**
  - the iroh adapter, the production behaviour today
  - an in-memory adapter, which records frames and preambles and fails a chosen open or write

  Ticket 10's dev-faults adapter is a third, wrapping the iroh one behind its cargo feature.
- **The writer's rule is unchanged:**
  - open lazily on the first frame
  - on a write failure, reopen once and retry that frame
  - if the reopen or the retry fails, close the connection
  - mark the capture's `written` after each frame
- **The writer has no replay yet.** Ticket 08 adds "on reopen, write the friend's in-flight frames first", getting them from the friend pipeline. It takes the pipeline's lock briefly and never across an await.

### Threading (ADR-0001 §2)

- Send stays a synchronous enqueue on the IPC thread, with no `block_on`.
- The pipeline and inbox locks are synchronous, short, and never held across an await or while taking another module's lock.
- The reader, cleanup and writer stay on the Transport's runtime.

### Effect on the Early ack tickets

The new hook map for tickets 07–10:

| Ticket's hook | Where it goes now |
|---|---|
| Make an Early ack at Send | The pipeline's outbound result |
| Drop a resend | The pipeline's outbound result |
| Swallow a real ack and release withheld Early acks | The pipeline's inbound result |
| Clear state at Game session start and on a loss | The pipeline's lifecycle events |
| Replay on reopen | The writer, asking the pipeline |
| Ticket 08's reset test | Uses the in-memory frame sink, so 08 isn't blocked by 10 |
| Ticket 09's counts and the capture id | Come from the pipeline |
| Ticket 12's page reads | Need a pipeline snapshot, exposed through the Transport |

- The "two players" check stays where ticket 07 puts it: at Send, from the session roster. Its result is passed to the pipeline with each outbound message.

## Testing Decisions

- **What makes a good test here:**
  - it drives a module only through its interface and asserts on what that interface returns or what comes out the other end: results, inbox order, frames written
  - it doesn't assert on internal maps, lock contents or log text
  - it survives the Early ack tickets changing the implementation
- **Friend pipeline, in-process, no network.** Feed it game messages and lifecycle events with an injected clock and RTT, then assert:
  - every outcome is "send" or "deliver"
  - capture ids pass through
  - the recorded fixtures (`turn_sync_turn`, `turn_sync_game_start`) produce the same Turn sync reports when replayed through the pipeline as through the detector directly
  - two pipelines for the same friend in different Transports don't share state
- **The Turn sync detector's own tests stay.** It is an internal seam with rich fixture tests already. Don't duplicate them at the pipeline.
- **JACKAL decoder:** small table tests for each named classification, including setup kinds `0x14`/`0x12` and payloads shorter than the header.
- **Game inbox, in-process.** Assert:
  - items come out in the order added, across friend messages, loopback and system events
  - a second "player joined" for the same player is not added
  - "Game session began" empties it and resets the dedup, so a player can join again
  - count matches what take returns
  - the system message built for each event matches today's (prior art: the system-message helpers checked in the Transport tests)
- **Ordered writer with the in-memory frame sink.** Drive the writer through its outbox and assert:
  - frames arrive in order, after one preamble
  - one failed write leads to a reopen, a second preamble and the same frame retried, then later frames in order
  - a failed reopen or a failed retry closes the connection and writes nothing more
- **Transport wiring tests,** with real loopback endpoints (prior art: `tests/mesh_networking.rs`, `poll_until`; at most three new):
  - the existing end-to-end ordered delivery, closed-session-messages and Joiner-leaving tests still pass unchanged
  - **regression for the bug:** a friend's connection is lost and the host's game hosts a new Game session before polling; the host still reconnects to the friend
- **Replace, don't layer.** Existing Transport tests that only checked inbox ordering or dedup may move to the inbox's tests. The end-to-end tests stay.
- **Whole suite:** `cargo test` passes, including `tests/mesh_networking.rs` and the Helper's `tests/ui.rs`, which are untouched.

## Out of Scope

- The Early ack rules themselves, the replay on reopen, the dev-faults feature and the new capture lines: tickets 07, 08, 10 and 09, built on top of this.
- The page's Turn sync activity (ticket 12), and the pipeline snapshot it needs.
- Making the traffic capture's file per Transport.
- Which connection a friend's entry belongs to when there are two (review candidate F).
- A Game session module (review candidate D) and the controller's connections seam (candidate E).
- The DLL, the IPC protocol, the Peer protocol, and the DLL's own JACKAL trace parsing.
- Game-session-sync ticket 05 (telling the game when a re-dial gives up).

## Further Notes

- Tickets 07, 08 and 10 cite hook line numbers that have already drifted. Once this lands, their "where it lives" notes should point at the friend pipeline, the inbox and the frame sink. Ticket 08's "Blocked by" no longer implies needing 10.
- ADR-0005 places the Early ack module under the connection manager. The friend pipeline is held on the connection manager's per-friend entries, so this follows the ADR.
- The release notes for the patch that ships this should mention the earlier reconnect after a lost connection.
