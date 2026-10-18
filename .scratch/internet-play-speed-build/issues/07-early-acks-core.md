# 07: Early acks core

**What to build:** With `DATALINK_EARLY_ACKS=1` on the Helper, a two-player game's reliable messages get an **Early ack** at once, instead of waiting a round trip. The design is settled in `.scratch/internet-play-speed/issues/09-helper-acks-for-the-friends-game.md`; [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 2 summarises it. It lives in the friend pipeline (`crates/datalink-transport/src/friend_pipeline.rs`), one per friend, which already sees every outbound and inbound game message and decodes JACKAL through `jackal.rs`. The pipeline only says what to do; the caller acts after the call returns.

In short: it acts only on kind `0x0004` messages to the one friend, made when `FriendPipeline::outbound` sees the message, as the message's first 12 bytes with the kind word rewritten to `0x0002`, from the friend's player DPID, added to the game inbox (`inbox.rs`, `GameInbox`) by the caller after the pipeline returns. Today `OutboundOutcome` has only `Send` and `InboundOutcome` only `Deliver`; this ticket adds the variants for "send and give the game an Early ack", "drop this resend", "swallow this real ack" and "deliver and release withheld Early acks". Each message is kept until the **real ack**, which is swallowed (only exact `0x0002` acks for a seq early-acked, duplicates included). A resend the same connection already carried is dropped. The cap is 16 in flight per friend: at the cap the message still goes out, but its early ack is withheld until a real ack frees a slot. All state clears on the pipeline's lifecycle events: `FriendPipeline::game_session_began` and `connection_lost`. Both exist as empty hooks; `connection_lost` is already called from the connection manager's cleanup, and `game_session_began` is not yet called by anything, so wiring it to the Game session start is part of this work (a follow-up may do it first). With three or more players, behaviour is exactly 0.1.0. It logs "early acks on" at start and once per Game session.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With the switch off, behaviour is identical to today
- [ ] In-process two-Transport tests (as in `tests/mesh_networking.rs`): an early ack appears before the friend has the message, the real ack is swallowed, a resend is dropped, the 17th early ack is withheld, no early acks with three players
- [ ] In-process pipeline tests (no network, injected clock and RTT) cover the same rules through `outbound`/`inbound` results
- [ ] Setup kinds `0x12`/`0x14` are left alone
- [ ] The game receives exactly the bytes sent, in order

## Comments

- 2026-10-18: Build this inside the friend pipeline and the game inbox from [transport-message-path](../../archive/transport-message-path/spec.md), which lands first. That spec's "Effect on the Early ack tickets" table replaces the hook list above. All five of its tickets are built, and the description above now names the modules as built. The "two players" check stays at `Transport::send`, from the session roster, and is passed to the pipeline with each outbound message.
