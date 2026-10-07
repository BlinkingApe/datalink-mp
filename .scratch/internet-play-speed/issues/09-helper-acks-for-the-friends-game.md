# 09: How does the Helper ack for the friend's game?

Type: grilling
Status: resolved
Blocked by: 06

## Question

[What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md) chose the Helper acking for the friend's game as the main fix: when its game sends a reliable message and the frame is on the friend's stream, the Helper hands its game the ack the friend's game would send. [The decision brief](../analysis/turn-sync-fixes.md), section 2A, sketches it, with rules 1–7. Settle the design in enough detail to hand to `/to-tickets`:

- **The rules, made concrete:**
  - keeping messages until the friend's real ack, and replaying them after a reconnect
  - no acks for a friend whose connection is down
  - the hold when recipients change in a game of three or more
  - the cap on messages in flight, and its number
  - dropping a resend whose original is still in flight on the same stream
  - which acks are swallowed, given setup messages go to player 0 and the setup kind `0x12`/`0x14` exchange is left alone
- **Where in the Helper it lives,** against the reconnect driver and the existing loopback queue.
- **The opt-in switch** for its first 0.1.x patch: what the player or tester sets (a setting, an environment variable), and what's logged. Also what turns it on by default in a later patch.
- **Capture lines** for made-up and swallowed acks, and the "one-way hop" bucket in `analysis/analyse_capture.py` that measures the target: ≤ 1 s network time per Turn sync Direct, ≤ 1.5 s Relayed, ≤ 2 s at the game start.
- **Validation before release:**
  - the three-instance Wine test
  - a fault-injection test that drops the connection mid-resync
  - a captured Direct and a captured **Relayed** game, which needs a dev switch that forces relaying
  - the compression condition: whether bytes still add ≥ 1 s on a Relayed resync turn or the game start
- **The checksum baseline:** how short a look at why two of five checksums mismatched on stock 0.1.0 is enough to tell a new desync from the existing ones.

## Answer

2026-10-06. The Helper's ack is now called an **Early ack** (in `GLOSSARY.md`), and the friend's game's own ack is the **real ack**.

**Two-player games only.** The Helper makes early acks only while the Game session holds our player and exactly one remote player. With three or more players it behaves exactly as `0.1.0`.
- **This replaces rule 3 of [What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md)**, which isn't safe. If the host broadcasts M1 and then M2 to friends X and Y, X can receive M2 and send something to Y before M1 reaches Y. The recipients never changed, so rule 3 would hold nothing.
- Pipelining is safe only while every message in flight and the new one are for the same single friend. If three-player speed is ever wanted, the rule is: early-ack only in that case, and otherwise hold the new message itself (not just its ack) until the earlier ones have real acks. In three-player games most Turn sync traffic is host broadcasts, so the gain would be small.

**The rules:**
- **What it acts on:** messages whose kind word is exactly `0x0004`, from our game to the one friend, whether directed or to `DPID_ALLPLAYERS`. The setup kind `0x14`/`0x12` is left alone by construction.
- **When:** at enqueue, synchronously inside `Transport::send`, before the Send IPC call returns. The early ack comes from the friend's player DPID and is the message's first 12 bytes with the kind word rewritten to `0x0002`. It goes into the game's inbox after the channel is drained, so it can't overtake messages from the friend that arrived earlier.
- **Kept until the real ack:** each message stays in flight, with its encoded frame, until the real ack arrives.
- **Cap:** 16 messages in flight per friend, with no byte cap. At the cap, the message still goes out at once, but its early ack is withheld until a real ack frees a slot. The cap also stops early acks on a dead connection that hasn't been declared lost yet. That takes 30 s (noq's idle timeout), and JACKAL's own 20 s timeout fires first, as today.
- **Swallowing:** only acks of kind exactly `0x0002` from the friend's player, for a seq the Helper has early-acked, including duplicates. Every other ack passes through.
- **Dropping resends:** a `0x0004` message is dropped if this same connection has already carried its seq. The key is the connection and the seq, not the DPID, because the game resends to the Joiner's DPID while the original went to `DPID_ALLPLAYERS`.
- **Reset:** all state is cleared when a Game session begins, because seqs restart with each one.
- **Connection lost:** clear the friend's state, make no early acks for the rest of that Game session, and don't replay across a reconnect. In a two-player game the session is over by then anyway: the Joiner's game gets `SESSIONLOST`. Today's behaviour carries on unchanged.
- **Stream reopened while the connection lives** (`connection.rs:193`): the writer replays the in-flight frames first, in order, then the frame it was retrying. The friend's JACKAL drops any seq not above the last one it saw, so copies it already has are dropped. Control messages lost on a reopen are an existing hole that this doesn't fix.

**Where it lives:** a new module, `crates/datalink-transport/src/early_ack.rs`, holding each friend's state. The `ConnectionManager` owns it. The hooks:
- `Transport::send` (`runtime.rs:639`): drop resends, record the message, make its early ack.
- The stream reader (`handle_peer_message`, `connection.rs:894`): swallow real acks and release withheld early acks into the channel.
- Connection cleanup (`connection.rs:656`) and a new Game session (`runtime.rs:229`): clear the state.
- The writer task: replay on reopen.
- The reconnect driver isn't touched. "Two players" is checked at each send.

**Opt-in switch:** `DATALINK_EARLY_ACKS=1` on the Helper. It logs "early acks on" at start and once per Game session, and the page shows nothing.
- Only the host needs it on to get nearly all the gain.
- When early acks become the default, the same variable becomes the off switch (`=0`).
- **Default-on condition:** the validation below, plus at least 3 real tester games with early acks on, one of them with Windows hosting. Their captures must show no hang, no `SEND MESSAGE TIME EXPIRED` and no delivery fault.

**The traffic capture** merges into main and ships in the same patch, off by default as today, so any tester's game can be analysed.
- **New lines, still format 1:**
  - `early_ack`: `peer`, `seq`, the `out` line's `id`, `withheld_us`
  - `real_ack`: `peer`, `seq`, `rtt_us`
  - `resend_dropped`: `peer`, `seq`
  - `replay`: `peer`, `count`
  - `early_acks`, once per Game session: `on`/`off`, and why (`switch`, `players`, `connection_lost`)
- **`analyse_capture.py` gains a one-way hop bucket:** at each change of direction, the time from our message's `written` to the `in` of the friend's next message. **Network time** is hops plus DLL polls, judged against ≤ 1 s per Turn sync Direct, ≤ 1.5 s Relayed, and ≤ 2 s at the game start.
- **It also gains a delivery check:** one `real_ack` per early ack, no gaps in the friend's seqs, every message drained once.

**The checksum baseline:** the existing mismatches aren't explained first. A validation game passes if its delivery check is intact. The bytes and order are identical, so early acks can't then have caused a mismatch. Mismatch counts per category, with early acks on and off, are compared only as a sanity check.

**Validation before the opt-in patch, in order:**
1. **Transport tests**, in-process with two Transports as in `tests/mesh_networking.rs`:
   - an early ack appears before the friend has the message
   - the real ack is swallowed
   - a resend is dropped
   - the 17th early ack is withheld
   - no early acks with three players or after a loss
   - a reset stream replays
2. **A two-instance Wine game**, early acks on and captured, covering the game start and a few turns. It must pass the delivery check.
3. **A three-instance Wine game:** early acks stay off, and the game plays as in `0.1.0`.
4. **Fault injection**, through a dev-only cargo feature `dev-faults` that release builds don't have. Each fault runs during the game start's state sync, which happens every game, unlike a resync.
   - `DATALINK_FAULT=drop:N` closes the friend's connection after N early acks. The outcome must match `0.1.0`, with no extra hang.
   - `reset:N` resets the stream. Play must continue and pass the delivery check.
5. **A captured Direct internet game** on the reference setup, against the target.
6. **A captured Relayed game**, using `DATALINK_RELAY_ONLY=1` on one Helper (`Endpoint::Builder::clear_ip_transports()`). This switch ships in the Helper, undocumented for players, so a tester abroad can capture one without a special build.
   - It also settles **the compression condition**: if the uplink still adds ≥ 1 s to the game start or a resync turn, compression comes back as a new effort.
