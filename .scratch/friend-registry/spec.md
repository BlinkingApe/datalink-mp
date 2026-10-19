# Spec: the friend registry

Status: ready-for-agent

Date: 2026-10-19. Comes from the architecture review of the same date (candidates 1 and 2, merged into one module), with candidates 3–7 explored in [their own section](#candidates-37-explored). Builds on [transport-message-path](../archive/transport-message-path/spec.md) and comes before Early ack tickets 07 and 08 in [internet-play-speed-build](../internet-play-speed-build/issues/).

## Problem Statement

Early ack ticket 07 is next. It makes the friend pipeline answer with new outcomes: hand the game an Early ack, drop a resend, swallow a real ack. The transport-message-path effort gave every friend a pipeline, but nothing carries out what that pipeline decides:

- **The outcome is thrown away.** Between the Transport's Send and the pipeline sit four shallow modules: the Send's own routing by DPID, three send functions on the connection manager (to a peer, to a player, to everyone), and the per-friend enqueue. The enqueue reads only the capture id from the pipeline's answer, the inbound path does the same, and nothing matches on the outcome. When ticket 07 adds outcome variants, the compiler will not point at a single place that has to act on them.
- **The place that would act can't reach the inbox.** An Early ack goes into the game inbox, which sits on the connection manager. The per-friend entry, where the pipeline's answer arrives, can't reach it. The broadcast path keeps only the last error, so it can't carry per-friend outcomes back to the Send either.
- **"A friend" is still spread over five places:**
  - the connection manager's entries, keyed by the friend's endpoint id
  - a DPID-to-friend routes map, written from nine places, three of them inside the stream handlers
  - a per-entry list of player ids that is written and never read
  - the session roster's node id and ticket for each player
  - the pipeline on the entry
- **Directed sends most likely fail after a re-dial (probable bug):**
  - When a friend's connection is lost, cleanup removes the friend's entry and every route to that friend's players, then starts the five-attempt re-dial.
  - The re-dial opens a new connection and stops. It sends no Hello, and nothing restores the routes.
  - Broadcasts reach the friend again, but a Send addressed to the friend's player fails with "player not found", on both sides, until something else re-adds the route.
  - In a two-player game the Joiner is told its session was lost anyway. The case that bites is a Joiner-to-Joiner connection in a game of three or more, where the game goes on with one pair unable to send each other directed messages.
  - No test covers it: the existing re-dial test only checks the friend is connected again.
- **A second connection to the same friend can kill the live one.** Join and the PlayerJoined handler both dial, so two Helpers can end up with two connections to each other. Entries are keyed by friend, not by connection:
  - a dial that completes second silently replaces the entry, dropping its pipeline without a "connection lost"
  - an incoming second connection gets a reader but no entry
  - when either connection closes, cleanup removes the entry for the *live* connection, tells its pipeline the connection was lost, and starts a re-dial
- **Ticket 08's rule has no home.** "No Early acks for the rest of the Game session after a loss" can't live in the pipeline, because the pipeline is dropped with the entry and the re-dial builds a fresh one.

For contributors, this means ticket 07 would add its outcome handling at four call sites across two files and work out per-friend routing by hand. Its only tests would be two real Transports and polling loops. For players in a game of three or more, a dropped Joiner-to-Joiner connection that heals may still leave that pair unable to message each other.

## Solution

Replace the connection manager's per-friend bookkeeping and its send functions with one module per Transport, the **friend registry**.

- **One record per friend.** It holds the friend's endpoint id, ticket and player DPIDs, the friend's current connection (a **link**) with its pipeline, any spare links, and the friend's facts scoped to the Game session that must outlive a link.
- **All sending to friends goes through it.** It routes a game message by DPID, including the host's loopback to its own server player. It calls each target friend's pipeline, matches on the outcome and carries it out. It also sends control messages, to one friend or to every friend, without the pipeline.
- **Inbound game messages go through it,** into the friend's pipeline and then the game inbox.
- **It owns the loss policy.** When a friend's current link ends, it decides what happens:
  - the friend was the host: tell the game its session is lost
  - otherwise: re-dial the friend with this ticket
  - or simply forget the friend

  The record of a friend being re-dialled is kept. When a new link to them opens, the registry re-attaches it, sends Hello first and restores the friend's routes.
- **It knows which link is current.** A second link to a friend becomes a spare. A spare ending changes nothing. When the current link ends and a spare is still open, the spare takes over.
- **Like the pipeline, it only says what should happen with anything async.** The connection manager keeps the iroh work (dialling, the re-dial driver, the readers and the ordered writers) and tells the registry about links opening and ending. The registry itself does no I/O and never touches iroh, so it is tested in-process.

Players see one change: after a friend's connection is lost and re-dialled, directed messages to that friend reach them again. Peer protocol and IPC versions are unchanged, so this ships in a 0.1.x patch.

## User Stories

1. As a contributor building Early acks, I want one module that calls a friend's pipeline and carries out its outcome, so that ticket 07's new outcomes are handled in one place.
2. As a contributor building Early acks, I want the outcome matched exhaustively wherever it is carried out, so that adding an outcome variant is a compile error until it is handled.
3. As a contributor building Early acks, I want the module that carries out outbound outcomes to reach the game inbox, so that "hand the game this Early ack now" is one call after the pipeline returns.
4. As a contributor building Early acks, I want the inbound outcome carried out in the same module, so that "swallow this real ack" and "release withheld Early acks" don't need a second hook in the stream reader.
5. As a contributor building ticket 08, I want facts scoped to the Game session on the friend's record, which survive a lost link and a re-dial, so that "no Early acks for the rest of this Game session after a loss" has somewhere to live.
6. As a contributor building ticket 08, I want each link to have its own pipeline that hears "connection lost" exactly once, when that link stops being current, so that clearing Early ack state follows the connection actually carrying the messages.
7. As a contributor building ticket 07, I want the registry to know a friend's player DPIDs, so that an Early ack can be made "from the friend's player" without a lookup in three maps.
8. As a contributor building game-session-sync ticket 05, I want "the re-dial gave up" to be an event the registry hears, so that telling the game a friend's players left has one obvious home.
9. As a contributor building ticket 12, I want connected friends and their paths read from the registry, so that the page's later reads of per-friend state come from the same place.
10. As a contributor writing tests, I want to drive the registry with stand-in links whose outboxes I hold, so that routing, loss and re-dial rules are tested in milliseconds without iroh.
11. As a contributor writing tests, I want to assert on the frames that come out of each friend's outbox and the items in the game inbox, so that tests check what crosses the registry's interface and not its maps.
12. As a contributor reading the code, I want one place that answers "who is this friend, which connection is theirs, and which players are behind it", so that I don't piece a friend together from five maps.
13. As a contributor reading the code, I want the Transport's Send to pass the facts it knows and delegate, so that routing by DPID lives in one module.
14. As a player in a game of three or more, I want a friend whose connection dropped and was re-dialled to receive my game's directed messages again, so that the game goes on normally once the connection heals.
15. As a player, I want my game's messages to a friend being re-dialled to fail just as they do today, so that the game's own handling of a missing player is unchanged.
16. As a player, I want a friend whose re-dial gave up to be forgotten, as today, so that nothing waits for a Helper that is gone.
17. As a player, I want two Helpers that dialled each other at the same moment to keep one working connection, so that a spare connection closing doesn't make them drop and re-dial each other.
18. As a player whose friend was the host, I want my game told the session is lost exactly as today when that connection drops, so that the game's own handling still runs.
19. As a player, I want a friend that was only connected for session discovery to be forgotten when its connection ends, without a re-dial, as today.
20. As a player, I want the game to receive exactly the bytes sent, in the order sent, so that this restructure can't cause a desync.
21. As a player, I want the host's messages to its own server player still to come back to it in order with friends' messages, so that hosting works as today.
22. As a player, I want a message to all players still to reach every friend once and never come back to me, as real DirectPlay behaves.
23. As a player, I want a re-dialled friend to hear our Hello before anything else on the new connection, so that their Helper can route my players' messages straight away.
24. As a tester reading a capture, I want the `lost`, `reconnect`, `send_failed`, `loopback` and game-message lines unchanged, so that the analysis script and earlier captures stay valid.
25. As a tester reading logs, I want the Turn sync and path log lines unchanged.
26. As a maintainer, I want ADR-0001 §2's threading rules still met (no blocking on the runtime, no lock held across an await, Send stays a plain enqueue), so that the Helper keeps its process structure.
27. As a maintainer, I want no module's lock held while another module's lock is taken on the message path, so that the deadlock trap the transport-message-path spec removed stays removed.
28. As a maintainer, I want per-friend FIFO kept: enqueue order is wire order, for game and control messages alike.
29. As a maintainer, I want the dead per-entry player list, the routes pass-throughs and the three send functions gone, so that there is one way to send to a friend.
30. As a maintainer, I want the capture's duplicate game-message branch removed, now that every outbound game message goes through a pipeline.
31. As a maintainer, I want at most two Transport-level tests added, so that the slow suite grows only by its wiring checks.
32. As a maintainer, I want the first test with three Helpers, so that the mesh is checked where the re-dial bug bites.
33. As a maintainer, I want `GLOSSARY.md` to define **Friend** and to say where the Game session is tracked, so that the vocabulary matches the code.
34. As a maintainer, I want tickets 07, 08 and 10 to point at the registry, so that the next agent builds inside it instead of adding hooks.

## Implementation Decisions

### The friend registry

- **One per Transport.** It replaces the connection manager's entries map, its routes map, the per-entry player list, the send functions (to a peer, to a player, broadcast, broadcast-except) and the routes interface (add, remove, look up by player).
- **The friend record:**
  - endpoint id
  - ticket, from our dial, the friend's Hello, or the roster facts at a loss
  - the friend's player DPIDs (these are the routes)
  - the current link and any spare links
  - facts scoped to the Game session that outlive a link. In this spec that is only the slot itself. Ticket 08 adds "lost a link this Game session".
- **A link** is one connection to a friend, as the registry sees it:
  - an identity, to tell two links to the same friend apart
  - the outbox into that connection's ordered writer
  - the friend pipeline for that link
  - an opaque connection handle, which the registry hands back to the connection manager for request/response streams, path reads and closing

  The connection manager builds a link when a connection is made (dialled or accepted): it spawns the writer, builds the pipeline with its RTT closure and gives the link to the registry. Tests build links from a plain channel and a stand-in handle.
- **Events the registry hears:**
  - a link opened to a friend (with the ticket, if we dialled)
  - a link ended
  - the re-dial of a friend gave up
  - a Hello from a friend's player, or a player joined, or a player left (route changes)
  - a Game session began
  - disconnect one friend, or all of them
- **Calls it answers:**
  - send a game message (from, to, flags, data) with the facts the Send knows: whether we are the host, the host's endpoint, the local player
  - receive a game message from a friend
  - send a control message to one friend, to every friend, or to every friend but one
  - is this friend connected (has a current link)
  - the connected friends, and the class of each current link's path
- **The registry never reads the session manager.** Roster facts are passed in by the caller, as the inbox already does. A route changes only through the registry's events, including the writes the stream handlers do directly today.
- **Only one Helper per Transport.** The registry is owned by the Transport and shared with the connection manager's tasks, never process-wide.

### Sending

- **Routing by DPID moves into the registry, unchanged in behaviour:**
  - all players: every friend with a current link, once each, never back to the sender
  - the server player, when we are the host: the local loopback into the game inbox, with its capture line
  - the server player, otherwise: the host's friend
  - any other DPID: the friend whose record lists it
  - a DPID no record lists, or a friend being re-dialled: the same error as today
- **For each target friend:** call its current link's pipeline, match on the outcome, and carry it out. Today the only outcome is "send it", which queues the encoded frame and its capture id on the link's outbox. Ticket 07 adds its variants here.
- **The result keeps today's meaning.** A broadcast attempts every friend and fails with the last error. A failed Send writes the capture's `send_failed` line, as today.
- **Control messages** are encoded once and queued on each target's current outbox, with their capture line as today, and never go through the pipeline.
- **Per-friend FIFO.** Game messages are sent only from the IPC thread, and control messages from it and from the runtime's tasks, as today. Whatever thread calls, a message is queued before the call returns, so enqueue order is wire order.

### Receiving

- A game message read from a friend goes to that friend's current pipeline. The registry matches on the inbound outcome and carries it out: today "deliver it", into the game inbox with its capture id.
- A game message from a sender with no record (the connection ended and the reader has not finished) is delivered as today, with its plain capture line and no pipeline.
- The stream reader stays in the connection manager. It hands game messages to the registry and keeps its other branches, except that their route writes become registry events.

### Links, loss and re-dial

- **The first link to a friend is current.** A later link to the same friend, from a simultaneous dial or an incoming connection, is a spare. Its reader still delivers what arrives on it.
- **A spare link ends:** it is removed. Nothing else happens: no loss, no re-dial, and the routes stay.
- **The current link ends while a spare is open:** the spare becomes current, and the old link's pipeline hears "connection lost". This is not a lost friend, so there is no `lost` line, no SESSIONLOST and no re-dial.
- **The current link ends and no spare is open:** the friend is lost. The old link's pipeline hears "connection lost". The caller passes the roster facts (was this friend the host, the friend's ticket from the roster), and the registry decides:
  - **the host:** write the capture's `lost` line, add "session lost" to the inbox, remove the record
  - **a friend with players and a ticket:** write the `lost` line, keep the record with no current link (directed sends to its players fail as today), and answer "re-dial with this ticket"
  - **anyone else** (a discovery-only friend): write the `lost` line and remove the record
- **The re-dial driver stays in the connection manager.** It is the same five attempts with the same backoff and the same `reconnect` capture lines, now started from the registry's answer. When a dial succeeds, the new link opens like any other.
- **Re-attaching.** When a link opens for a friend whose record has no current link, it becomes current. If a local player exists, the registry queues a Hello on it first, before any other frame. The friend's DPIDs are still on the record, so directed sends route again. The friend's Helper does the same on its side: its retained record re-attaches the incoming link.
- **The re-dial gave up:** the record is removed. Game-session-sync ticket 05 may later make this event tell the game.
- **Disconnecting.** Disconnecting one friend closes its links, and the loss is handled when they end, as today. This keeps today's behaviour, which the existing test of a Joiner being told its session was lost relies on. Disconnect-all, at shutdown, keeps today's behaviour.
- **A dial to a friend** checks "is this friend connected" (has a current link), not "has a record".

### Game session began

- Every current link's pipeline hears "Game session began", as today. Each record's facts scoped to the Game session are cleared. In this spec there are none yet.

### Locking and threading (ADR-0001 §2)

- The registry has one short synchronous lock. It is never held across an await, or while calling a pipeline, the inbox or the capture. The registry copies out the links it needs, releases its lock, and then calls the pipeline and acts.
- Send stays a synchronous enqueue on the IPC thread, with no `block_on`.
- The readers, the writers, cleanup and the re-dial driver stay on the Transport's runtime.

### What goes

- The per-entry player list (dead state).
- The send-to-peer, send-to-player, broadcast and broadcast-except functions, and the routes interface. Their callers use the registry.
- The capture's game-message branch for outbound messages. Every outbound game message now goes through a pipeline, which writes that line.

### Glossary

- Add **Friend**: another player's Helper that this Helper is connected to, or is re-dialling, in the current Game session or for session discovery. _Avoid_: peer (used in code for the same thing; fine there, not in docs), node.
- Fix **Game session**: it is tracked in the Transport's session state, not in the IPC layer.

## Testing Decisions

- **What makes a good test here:**
  - it drives the registry only through its interface: links opening and ending, Sends, received messages, events
  - it asserts only on what crosses the interface: frames on each friend's outbox, items in the game inbox, Send results, the registry's re-dial answer
  - it never asserts on the registry's maps, its lock or log text
  - it survives tickets 07 and 08 changing the implementation
- **Friend registry, in-process, no network.** Stand-in links (a channel whose receiving end the test holds, a stand-in handle, an RTT of "none") and a real game inbox. Assert:
  - **routing:**
    - all players reaches each friend once and not the sender
    - the server player loops back to the inbox when we are the host, and reaches the host's friend otherwise
    - a DPID reaches only its friend
    - an unknown DPID fails with today's error
  - **order:** frames on one friend's outbox decode to the messages sent, in call order, with game and control messages interleaved
  - **inbound:** game messages from friends reach the inbox in arrival order, and one from a sender with no record is still delivered
  - **loss:**
    - losing the host's link puts SESSIONLOST in the inbox and forgets the host
    - losing a friend with players answers "re-dial with this ticket", and directed sends to its players fail
    - losing a discovery-only friend answers nothing and forgets it
  - **re-attach:**
    - a new link for a friend being re-dialled carries Hello as its first frame
    - after that, directed sends to the friend's players arrive on the new outbox
    - after "re-dial gave up", they fail
  - **spares:**
    - a second link does not replace the current one, and frames keep going to the first
    - a spare ending changes nothing
    - the current link ending with a spare open moves frames to the spare, with no loss answer and nothing in the inbox
- **The pipeline, inbox, frame sink and Turn sync detector keep their own tests.** Don't duplicate them at the registry.
- **Transport wiring tests,** with real loopback endpoints (prior art: the Transport's mesh-networking tests and their `poll_until` helper; at most two new):
  - **regression for the re-dial bug:** extend the existing "friend lost just before a new Game session is still re-dialled" setup. After the host re-dials the Joiner, the host's game sends to the Joiner's player directly, and the Joiner's game receives it.
  - **three Helpers:** a host and two Joiners in one Game session. The Joiner-to-Joiner connection is dropped and re-dialled, and each Joiner's game then sends to the other's player directly, and it arrives. This is the suite's first test with three players.
- **Replace, don't layer.** The connection manager has no unit tests to remove. The existing end-to-end tests stay unchanged, and they must pass: ordered delivery, closed-session messages, a Joiner leaving, a Joiner whose host's connection is lost, and the re-dial test.
- **Whole suite:** `cargo test` passes, including the Helper's UI tests, which are untouched.

## Out of Scope

- The Early ack rules, replay on reopen, dev faults and the new capture lines: tickets 07, 08, 10 and 09, built on top of this.
- What the game is told when a re-dial gives up (game-session-sync ticket 05). This spec only gives that event a home.
- Changing the re-dial's attempts, backoff or trigger.
- The request/response streams (session query, join request) and their handler, apart from their route writes becoming registry events.
- Ordering between an old and a reopened stream on the receiving side. It matters only once ticket 08 replays; see candidate 5 below.
- Candidates 3, 4, 6 and 7 below, apart from the dead capture branch and the glossary fixes, which this spec does.
- The DLL, the IPC protocol and the Peer protocol.

## Candidates 3–7, explored

Each was read against this spec and the open tickets. Verdicts are recommendations, not decisions.

### 3. A Game session module

- **After this spec,** ticket 08's "lost a link this Game session" lives on the friend record. What remains with no home:
  - ticket 07's "two players" fact and its "early acks on" log line, once per Game session
  - ticket 09's `early_acks` line, once per Game session, with its reason (switch, players, connection lost)

  All are small, and ticket 07 already says the two-player check stays at the Send.
- **New finding: routes duplicate the roster.** Each remote player in the session roster carries its node id. The registry's DPIDs per friend say the same thing. A Game session module could make the roster the one source and let the registry ask it, but that couples the registry to the session manager, which this spec deliberately avoids.
- **The end of a Game session is still not an event.** It ends at five places (close, the DLL disconnecting, SESSIONLOST from a loss, SessionClosed from the host, Stop). Nothing in tickets 07–10 needs "ended" as long as "began" clears the state, which it does.
- **Verdict:** not before ticket 07. Revisit after ticket 09 if the once-per-Game-session lines end up scattered, and fold candidate 4 in then. The glossary drift is fixed here.

### 4. Peer protocol rules as decisions

- This spec moves the stream handlers' route writes into registry events. That removes one of the four kinds of effect they have. What's left: roster changes, inbox events, replies and dials.
- The rules with bug history (JoinRequest idempotency, the PlayerLeft "belongs" check, SessionClosed only from the host, queries answered only by the host) still need two real Transports to test.
- The new three-Helper wiring test is the first three-player coverage, which lowers the urgency.
- **Verdict:** do it with candidate 3, since most remaining effects land on the Game session's roster. No ticket now.

### 5. An ordered link in both directions

- **The receiving side, read again:**
  - When the writer reopens, the old send stream is replaced, and a new reader task starts for the new stream while the old one may still hold buffered frames.
  - Each game-message dispatch is synchronous, so frames never interleave mid-message. But nothing makes the old reader finish before the new one starts.
  - Today this can't reorder anything that matters: the reopened stream starts with the frame that failed, and JACKAL drops a seq it already has.
  - Once ticket 08 replays in-flight frames on reopen, the tail of the old stream and the replay can interleave.
- **Ticket 10's `drop:N`** counts Early acks, which the frame sink can't see. `reset:N` belongs at the frame sink, while `drop:N` belongs where Early acks are counted: the pipeline, or the registry acting on its outcomes.
- **Verdict:** don't build a two-way link seam now. Add to ticket 08: "the receiving side dispatches nothing from a reopened stream until the old stream's reader has finished", with a test at whatever seam ticket 08 settles on. Add to ticket 10: `drop:N` is counted where Early acks are made, and only `reset:N` wraps the frame sink.

### 6. A capture handle

- The registry's own tests can't see the `lost`, `send_failed` or `loopback` lines either, since the capture is a process-wide global that unit tests never start. That's acceptable here: none of those lines change.
- Ticket 09 is the first that needs capture-line order as a test assertion: `early_ack` naming its `out` line, `real_ack`, `resend_dropped`, `replay`.
- **Verdict:** do it as part of ticket 09, or as a ticket just before it. Give the pipeline, the registry, the inbox and the writer a capture handle with off, file and in-memory recorder adapters, keeping the file process-wide. This spec removes the one duplicate branch now.

### 7. A Transport seam under the controller

- The controller still builds Transports directly. Every UI test starts a real endpoint, and Stop's failure test exhausts the process's file descriptors.
- Ticket 12 will add page reads of Turn sync state through the Transport. After this spec, those reads come from the registry, alongside connected friends and paths.
- **Verdict:** decide when ticket 12 is specified. If ticket 12 adds more than a couple of Transport reads to the controller, specify the seam then, with the production Transport and a fake as its two adapters.

## Further Notes

- **Suggested ticket order:**
  1. the registry with sending and receiving, behaviour unchanged
  2. links, loss and re-attach, with the re-dial fix and both wiring tests
  3. removing the dead code and the capture branch, and the glossary fixes
  4. pointing tickets 07, 08 and 10 at the registry, including candidate 5's two notes

  The first can't fix the bug alone. The second depends on it.
- ADR-0005 places the Early ack module under the connection manager. The pipeline stays per link, and the registry the connection manager uses carries out its outcomes, so this follows the ADR.
- Ticket 07's description says the Game session start isn't yet wired to the pipelines. It is, as of the last commit on this branch. The pointer update should correct that line.
- The release notes for the patch that ships this should mention that directed messages reach a re-dialled friend again.
