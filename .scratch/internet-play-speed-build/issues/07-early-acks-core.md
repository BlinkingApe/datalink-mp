# 07: Early acks core

**What to build:** With `DATALINK_EARLY_ACKS=1` on the Helper, a two-player game's reliable messages get an **Early ack** at once, instead of waiting a round trip. The design is settled in `.scratch/internet-play-speed/issues/09-helper-acks-for-the-friends-game.md`; [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 2 summarises it. It lives in the friend pipeline (`crates/datalink-transport/src/friend_pipeline.rs`), one per link to a friend, which already sees every outbound and inbound game message and decodes JACKAL through `jackal.rs`. The pipeline only says what to do; the friend registry (`friend_registry.rs`, `FriendRegistry`) acts after the call returns.

In short: it acts only on kind `0x0004` messages to the one friend, made when `FriendPipeline::outbound` sees the message, as the message's first 12 bytes with the kind word rewritten to `0x0002`, from the friend's player DPID, added to the game inbox (`inbox.rs`, `GameInbox`) by the friend registry after the pipeline returns. Today `OutboundOutcome` has only `Send` and `InboundOutcome` only `Deliver`; this ticket adds the variants for "send and give the game an Early ack", "drop this resend", "swallow this real ack" and "deliver and release withheld Early acks". Each message is kept until the **real ack**, which is swallowed (only exact `0x0002` acks for a seq early-acked, duplicates included). A resend the same connection already carried is dropped. The cap is 16 in flight per friend: at the cap the message still goes out, but its early ack is withheld until a real ack frees a slot. All state clears on the pipeline's lifecycle events: `FriendPipeline::game_session_began` and `connection_lost`. Both exist as empty hooks, and the friend registry already calls both: `connection_lost` from `FriendRegistry::link_ended` when a link stops being current, and `game_session_began` from `FriendRegistry::game_session_began`, which the Transport calls as a Game session starts (hosting or joining). With three or more players, behaviour is exactly 0.1.0. It logs "early acks on" at start and once per Game session.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With the switch off, behaviour is identical to today
- [ ] In-process two-Transport tests (as in `tests/mesh_networking.rs`): an early ack appears before the friend has the message, the real ack is swallowed, a resend is dropped, the 17th early ack is withheld, no early acks with three players
- [ ] In-process pipeline tests (no network, injected clock and RTT) cover the same rules through `outbound`/`inbound` results
- [ ] Setup kinds `0x12`/`0x14` are left alone
- [ ] The game receives exactly the bytes sent, in order

## Comments

- 2026-10-18: Build this inside the friend pipeline and the game inbox from [transport-message-path](../../archive/transport-message-path/spec.md), which lands first. That spec's "Effect on the Early ack tickets" table replaces the hook list above. All five of its tickets are built, and the description above now names the modules as built. The "two players" check stays at `Transport::send`, from the session roster, and is passed to the pipeline with each outbound message.
- 2026-10-19: Build this inside the friend registry from [friend-registry](../../archive/friend-registry/spec.md), not with new hooks. All in `crates/datalink-transport/src/friend_registry.rs`:
  - **Outbound outcomes.** `FriendRegistry::send_game` routes by DPID and calls `queue_game` for each target link, which destructures `OutboundResult` and matches `OutboundOutcome` exhaustively. Add the new variants there. "Send and give the game this Early ack" queues the frame on `link.outbox` and then adds the Early ack to the inbox; "drop this resend" queues nothing. `queue_game` is a free function today, so pass it `&self.inbox` or make it a method.
  - **Inbound outcomes.** `FriendRegistry::receive_game` calls the current link's pipeline and matches `InboundOutcome`. "Swallow this real ack" delivers nothing; "deliver and release withheld Early acks" delivers the message, then adds the released Early acks to the inbox. No hook in the stream reader is needed. A message read on a spare link also goes through the friend's current pipeline, so a real ack is matched whichever connection it arrived on.
  - **Game session start.** The description's old line saying `game_session_began` wasn't wired is fixed above: it already reaches every current link's pipeline.
  - **The two-player fact** is passed in with the Send's other facts: add it to `SendFacts`, which `Transport::send` builds from the session roster, and pass it on to `FriendPipeline::outbound`.
  - **The friend's player DPIDs** are on the friend's record (`Record::players`, the routes), so the Early ack's "from" DPID needs no other lookup.
  - Keep the registry's locking rule: copy the `Arc<Link>` out, release the lock, and only then call the pipeline and the inbox.
