# 01: The friend registry carries every message to and from friends

**What to build:** Each Transport has one **friend registry**, and everything sent to or received from friends goes through it. Behaviour is unchanged. This is the prefactor that gives Early ack ticket 07 one place to carry out the friend pipeline's outcomes.

- **The friend record:**
  - endpoint id
  - ticket
  - player DPIDs (these are the routes)
  - its link
  - an empty slot for facts scoped to the Game session that must outlive a link
- **A link** is one connection to a friend, as the registry sees it: an identity, the outbox into its ordered writer, its friend pipeline, and an opaque connection handle. The registry hands the handle back to the connection manager for request/response streams, path reads and closing. The connection manager builds the link when a connection is dialled or accepted and gives it to the registry.
- **Sending a game message:**
  - The Send passes what it knows (whether we are the host, the host's endpoint, the local player) and delegates.
  - The registry routes by DPID exactly as today: all players, the host's loopback to its own server player, the server player via the host's friend, or one friend by DPID.
  - For each target it calls the link's pipeline, matches on the outcome exhaustively and carries it out. Today that means queueing the frame and its capture id.
  - Errors and the `send_failed` capture line are as today.
- **Control messages** go to one friend, every friend, or every friend but one. They are encoded once, queued with their capture line as today, and never pass through the pipeline.
- **Receiving:**
  - A game message read from a friend goes to the friend's pipeline. The registry matches on the inbound outcome and delivers into the game inbox.
  - A game message from a sender with no record is delivered as today, with its plain capture line.
- **Routes change only through registry events.** These are a Hello, a player joining and a player leaving, including the writes the stream handlers do directly today.
- **"A Game session began"** reaches every link's pipeline through the registry.
- **Connected friends and their path classes** are read from the registry.
- **Today's lifecycle is kept for now.** A link ending removes the friend, and the loss policy and re-dial run as today. Ticket 02 moves them into the registry.
- **Locking:** one short synchronous lock. It is never held across an await, or while calling a pipeline, the inbox or the capture. Send stays a synchronous enqueue on the IPC thread (ADR-0001 §2).
- **What goes:**
  - the connection manager's entries and routes maps
  - the per-entry player list, which is never read
  - the send-to-peer, send-to-player, broadcast and broadcast-except functions
  - the routes interface
  - the capture's outbound game-message branch, now dead
- **Glossary:**
  - add **Friend**, as worded in the spec
  - fix **Game session** to say it is tracked in the Transport's session state, not the IPC layer

See the spec: [The friend registry](../spec.md#the-friend-registry), [Sending](../spec.md#sending), [Receiving](../spec.md#receiving), [Locking and threading](../spec.md#locking-and-threading-adr-0001-2).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] In-process registry tests, using stand-in links (a channel whose receiving end the test holds, and an RTT of "none") and a real game inbox:
  - all players reaches each friend once, and not the sender
  - the server player loops back to the inbox when we are the host, and reaches the host's friend otherwise
  - a DPID reaches only its friend, and an unknown DPID fails with today's error
- [ ] In-process: frames on one friend's outbox decode to the messages sent, in call order, with game and control messages interleaved
- [ ] In-process: game messages from friends reach the inbox in arrival order, and one from a sender with no record is still delivered
- [ ] Both pipeline outcomes are matched exhaustively, with no field read past the outcome
- [ ] The registry never reads the session manager; roster facts are passed in
- [ ] Capture lines, Turn sync log lines and path log lines are unchanged
- [ ] `GLOSSARY.md` has **Friend** and the corrected **Game session** line
- [ ] `cargo test` passes, including the Transport's mesh-networking tests and the Helper's UI tests, unchanged
