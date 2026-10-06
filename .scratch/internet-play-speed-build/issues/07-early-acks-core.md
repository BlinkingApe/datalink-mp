# 07: Early acks core

**What to build:** With `DATALINK_EARLY_ACKS=1` on the Helper, a two-player game's reliable messages get an **Early ack** at once, instead of waiting a round trip. The design is settled in `.scratch/internet-play-speed/issues/09-helper-acks-for-the-friends-game.md`; [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 2 summarises it. It lives in a new `early_ack.rs` owned by the `ConnectionManager`.

In short: it acts only on kind `0x0004` messages to the one friend, made at enqueue inside `Transport::send`, as the message's first 12 bytes with the kind word rewritten to `0x0002`, from the friend's player DPID, placed in the inbox after the channel is drained. Each message is kept until the **real ack**, which is swallowed (only exact `0x0002` acks for a seq early-acked, duplicates included). A resend the same connection already carried is dropped. The cap is 16 in flight per friend: at the cap the message still goes out, but its early ack is withheld until a real ack frees a slot. All state clears when a Game session begins. With three or more players, behaviour is exactly 0.1.0. It logs "early acks on" at start and once per Game session.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With the switch off, behaviour is identical to today
- [ ] In-process two-Transport tests (as in `tests/mesh_networking.rs`): an early ack appears before the friend has the message, the real ack is swallowed, a resend is dropped, the 17th early ack is withheld, no early acks with three players
- [ ] Setup kinds `0x12`/`0x14` are left alone
- [ ] The game receives exactly the bytes sent, in order
