# Analysis: the captured Direct game

- **Date:** 2026-10-06
- **Capture:** `captures/capture-1791300326-1036127.jsonl` (gitignored), from [Play a captured internet game](../issues/04-play-a-captured-internet-game.md). Linux hosted on the mobile-data hotspot and captured. Windows joined on stock `v0.1.0`. One player at a time, five turns, a **Direct connection** throughout.
- **Serves:** [Analyse the capture](../issues/05-analyse-the-capture.md), for [What makes a Turn sync slow, and what do we fix?](../issues/06-what-makes-turn-sync-slow.md).
- **Reproduce:** `python3 analysis/analyse_capture.py captures/capture-1791300326-1036127.jsonl` (needs `zstandard`, `lz4`, `numpy`). It works on any capture where the capturing Helper hosts, so a Relayed game can be run through it unchanged.

Facts are measured from the capture. **[inference]** marks a reading of them.

## TL;DR

1. **A Turn sync is mostly the game computing.** In the four Turn syncs, 67–78% of the time nothing was in flight while one machine ran its upkeep. That was usually the Linux host, which spent up to 10 s on a single phase. Round trips took 19–29%, and the DLL's 10 ms poll took 1–3%. The Helper's own queueing was nil (≤ 0.5 ms).
2. **Each reliable message costs one round trip, and only one is ever in flight.** Not one of 483 host sends and 120 Joiner sends overlapped the previous message's ack. A message's ack arrives about 5–8 ms after the QUIC RTT. There are no bursts or windows.
3. **Bytes matter only when the game resyncs.** A normal Turn sync carries 1–5 KB. Turn sync 1 carried 110 KB, because a leaders checksum mismatched and the host resent every faction's leader data (68 messages of up to 2 KB, 7.9 s). The game start carried 286 KB. Each KB adds about 8–14 ms to its message's round trip.
4. **Large payloads compress to 9–16%.** Delta encoding against the previous message of the same kind does better only on resyncs (6% against 10% for zstd alone). On a normal Turn sync both are moot.
5. **Every retransmit is spurious.** 70% of the host's messages went out more than once (339 resends, +77% bytes). The game's resend timer stays at about 50–70 ms, just under the ack round trip. QUIC loses nothing, so no copy was ever needed.
6. **The slowest Turn sync (24.7 s):** about 16.7 s of compute, 5.5 s of network RTT, 1 s of bytes, 1.2 s of DLL polling on both sides, and no Helper queueing.

## Finding a Turn sync

Look for message types. Idle gaps don't work: a Turn sync has idle gaps of up to 10 s inside it, and the 2-second heartbeats never stop.

- **Start:** the last mover's end of turn. That is the Joiner's `0x8301`, answered within 15 ms by the host's `0x4301` (NEXT_TURN).
- **Phases:** each barrier is a Joiner `0x2303` and then the host's `0x4303`, with the phase at wire offset 24 and the turn at 28. Phase 2 uses `0x2305`/`0x4305` instead. Phases 0, 1, 5 and 6 always ran, phase 3 in four of five, phase 4 in two, and phase 7 (resync) only after a checksum mismatched.
- **End:** the host's `0x4309`, which names the faction that moves first in the next turn (wire offset 24).

Both Helpers see all of these (the Joiner receives the `0x43xx` broadcasts), so either side can tell a Turn sync is running, and which phase it's in.

A correction to [the JACKAL research](../../../docs/archive/research/smac-jackal-turn-sync.md) §3.1, which reads `0x4309` and `0x8301`/`0x4301` as belonging to different modes: this one-player-at-a-time game used both. `0x4309` hands the turn from one player to the next within a turn, and `0x8301`/`0x4301` end the turn. A simultaneous-moves game hasn't been captured.

## The Turn syncs

| | Start → end | Length | Host msgs (B) | Joiner msgs (B) | Barriers | Resync |
|---|---|---|---|---|---|---|
| Game start | 17:34:05 → 17:34:44 | 38.6 s | 195 (286 KB) | 12 (680) | 7 | bases |
| Turn sync 1 | 17:37:31 → 17:37:55 | **24.7 s** | 95 (110 KB) | 11 (620) | 7 | **leaders** |
| Turn sync 2 | 17:39:36 → 17:39:42 | 6.1 s | 20 (1.2 KB) | 8 (488) | 4 | none |
| Turn sync 3 | 17:40:35 → 17:40:44 | 9.1 s | 25 (1.4 KB) | 11 (652) | 5 | none |
| Turn sync 4 | 17:41:49 → 17:42:06 | 17.0 s | 31 (5.1 KB) | 10 (576) | 6 | vehicles |

Counts are unique messages, without resends. The game start includes the initial state sync, 167 messages that send the whole game (map, factions, leaders) before the first upkeep. Most host messages in a normal Turn sync are barriers (`0x4303`), two `0x1303` before each barrier, and small `0x4101` syncs.

**[inference]** `0x1303` goes from the host to the Joiner's player, carrying (faction 1 or 2, 1, turn). It probably tells the Joiner who has reported. In a two-player game the host also echoes each of the Joiner's own `0x2101` syncs and checksums back to it, as `0x4101`/`0x4307` sent to everyone. In this game that was 19 round trips spent returning the Joiner's data to the Joiner.

## Where the time goes

Each instant of a window is put in one bucket:

- **Round trip:** a host message is waiting for its ack.
- **DLL poll:** a message has reached the Helper, and the game hasn't drained it yet.
- **Compute:** nothing is in flight. It's credited to whichever side sends next.

| | Round trip | DLL poll | Host compute | Joiner compute |
|---|---|---|---|---|
| Game start | **20.8 s (54%)** | 1.3 s (3%) | 8.2 s (21%) | 8.3 s (21%) |
| Turn sync 1 | 7.2 s (29%) | 0.7 s (3%) | **14.2 s (57%)** | 2.6 s (10%) |
| Turn sync 2 | 1.2 s (19%) | 0.1 s (2%) | **4.6 s (76%)** | 0.1 s (2%) |
| Turn sync 3 | 2.1 s (23%) | 0.2 s (2%) | 0.7 s (8%) | **6.0 s (66%)** |
| Turn sync 4 | 3.4 s (20%) | 0.2 s (1%) | **12.5 s (73%)** | 0.9 s (5%) |

By phase, the long waits are compute. In Turn sync 1, phase 1 took 12.1 s (10 s of it host compute), phase 4 took 2.4 s, and phase 7 (the leaders resync) took 7.9 s, of which 5.9 s was round trips. In Turn sync 4, phase 3 took 7.7 s (7.1 s host compute). The analysis script prints the per-phase table.

**[inference]** "Compute" is the game running its upkeep locally (faction upkeep, the AI's moves, checksums). In three of the four Turn syncs the Linux host finished last, so it's the slower machine here. The machines and their load weren't controlled, so this says nothing yet about Wine.

### Inside a round trip

| | Median |
|---|---|
| out → written to QUIC (the Helper's queue) | < 0.5 ms max |
| QUIC RTT (iroh's estimate) | 52–77 ms per window (Turn sync 4: p90 214 ms) |
| ack wait over QUIC RTT (the far side's DLL poll and game) | +5–8 ms |
| ack arrives → the game drains it (this side's DLL poll) | 6 ms, max 13 ms |
| drain → the game's next send | 4–7 ms |

The DLL polls the Helper every 10 ms ([directplay.rs:309](../../../crates/dplayx/src/directplay.rs)). Every message therefore waits about 6 ms on average at each end before the game sees it.

**Cost of bytes.** A least-squares fit of ack wait against size, across all windows, gives 14 ms/KB, or 8.3 ms/KB with QUIC RTT as a second regressor. Medians: 57 ms for a message under 64 B, 80 ms for a 2 KB message. **[inference]** The hotspot's upload carries a few hundred KB/s, and the spurious resends (below) share it.

### Budget for the slowest Turn sync (1, 24.7 s)

| Cause | Time | How it was estimated |
|---|---|---|
| Game compute (both machines) | ~16.7 s (68%) | idle buckets |
| Network round trips | ~5.5 s (22%) | 95 host messages × the 58 ms median QUIC RTT |
| Bytes on the wire | ~0.9–1.5 s (4–6%) | 110 KB × 8–14 ms/KB |
| DLL polling, both ends | ~1.2 s (5%) | 0.7 s here, plus ~5 ms × 95 at the far end |
| Helper queueing | ~0 | out → written < 0.5 ms |

**[inference] What the fixes on the map could win, as upper bounds on this capture:**

- **An ack that costs no round trip,** if any design can get one, removes the round-trip and poll share: up to 32% of Turn sync 1, 21–25% of Turn syncs 2–4, and 57% of the game start.
- **Compression** removes most of the bytes share: 3–6% of Turn sync 1, under 1% of Turn syncs 2–4, and 5–9% of the game start.
- **Delta on top of compression** adds about 4 KB of savings, about 0.05 s.
- **An event-driven DLL instead of a 10 ms poll** saves about 12 ms per round trip (both ends): about 5% of Turn sync 1.
- **A Relayed connection would make it worse:** the 18 s on the relay before the game had a 146 ms median RTT, against about 60 ms Direct. At that RTT, Turn sync 1 would be about 33 s and the game start about 53 s. That projection isn't measured.
- Nothing between Helpers touches the compute share.

## Payloads

605 unique data messages, 431 KB. 195 of them are 1–2 KB, and they carry 89% of the bytes. The largest are setup messages: one `0x0f0d` of 6,018 B and `0x4f04` faction picks of 2,688 B. State syncs (`0x4101`) are split into chunks of at most 2,096 B on the wire, as the research found.

| | Raw | zstd-3 | lz4 | deflate-6 |
|---|---|---|---|---|
| ≥ 1 KB, each message alone | 395 KB | 12% | 16% | 11% |
| ≥ 1 KB, streamed (one compressor per direction, flushed per message) | | 9% | 13% | 9% |
| < 1 KB, each alone | 36 KB | 70% | 69% | 55% |
| < 1 KB, streamed | | 44% | 48% | 37% |

**Delta**, as XOR against the previous message with the same type, sync type, chunk and first argument, then zstd-3:

| | Raw | Have a predecessor | Delta + zstd | zstd alone |
|---|---|---|---|---|
| Game start | 287 KB | 29 of 207 | 14% | 15% |
| Turn sync 1 (leaders resync) | 111 KB | 99 of 106 | **6%** | 10% |
| Turn syncs 2–4 | 1.6–5.6 KB | all | 49–69% | 55–90% |

**[inference]** A streamed compressor per friend is worth more than delta encoding, and both only matter for the game start and resyncs.

## Retransmits

| | Messages | Sent more than once | Resends | Extra bytes |
|---|---|---|---|---|
| Host (Linux) | 484 | 297 (61%) | 339 | 325 KB on 423 KB (+77%) |
| Joiner (Windows) | 121 | 24 (20%) | 28 | 2.3 KB on 8.1 KB (+28%) |

- **Spacing:** first copy → second, median 61 ms (p10 40, p90 95). Second → third, median 107 ms. The first resends of the game came at 51 and 43 ms, so the timer starts at our DLL's `dwLatency = 50` as predicted, and backs off about ×1.5 per copy. It doesn't adapt above the ack round trip: the first-resend gap stayed at 50–70 ms in every quarter of the game.
- **All are spurious.** The transport is a reliable QUIC stream, so the first copy always arrived. 49 host resends were sent after the ack had already reached the Helper, while the DLL hadn't polled it yet.
- **Shape:** the first copy goes to `DPID_ALLPLAYERS` and the resends go to the Joiner's DPID. The receiver acks every copy. The host's DLL handed all 28 of the Joiner's duplicates to the game, which drops them by sequence number.
- **[inference]** On this link, resends cost bandwidth (a host message is sent about 1.7 times on average), not waiting. They're part of the bytes share above, not an extra cause. On a slower or relayed link they'd weigh more.

## Connection

**Direct** for the whole game, on one IPv6 path, from 17:31:57 until the Joiner quit at 17:44:04. Before the game, in Multiplayer Setup, the direct path closed once and the connection was Relayed through `euc1-1.relay.n0.iroh.link` for 18 s (median RTT 146 ms). The connection counters show 6 lost packets and no congestion events.

## The research's open questions (§9)

1. **Does every send arrive with `dwFlags = 0`?** Yes: all 2,657 game messages, including acks and heartbeats, have `guaranteed: false`.
2. **How many reliable messages and barriers make up a Turn sync?** 28–41 reliable messages without a resync (host 20–31, Joiner 8–11), and 106 with the leaders resync. 4–7 barriers (`0x4303`), plus the phase-2 `0x2305`/`0x4305` pair.
3. **How many retransmits, and are they 50 ms × 1.5ⁿ?** 339 from the host and 28 from the Joiner. They start at 50 ms and back off about ×1.5 within a message, but the timer stays at 50–70 ms, under the RTT, for the whole game (see Retransmits).
4. **What fraction of a Turn sync is round trips against compute?** Round trips 19–29% (plus 1–3% DLL poll), compute 67–78%. The game start is the exception, at 54% round trips.
5. **Are the header offsets right?** Yes. Kind at 0 (u16), sequence at 4 (u32, strictly increasing per sender), game type at 8, sender's faction at 12 (1 = host, 2 = Joiner), sync type at 24 and chunk index at 26 for `0x4101`/`0x2101`. For barriers, phase at 24 and turn at 28, and the faction to move at 24 for `0x4309`. Offset 2 is junk, never written. The setup messages (`0x4f02`–`0x4f04`, `0x0f0d`) have no faction at 12. So the research's §8 rows 4–5 conflicts are real: the `READY_SET`/`NEXT_TURN` labels in `crates/dplayx/src/directplay.rs` read the type at offset 0, which is the kind word.

**Other kinds seen:** `0x20` is a 12-byte heartbeat, sent each way to `DPID_ALLPLAYERS` every 2.00 s. `0x14` is a data message that the far side acks with `0x12`, used once, for the player-info message `0x0002` in setup.

## Not answered by this capture

- **A Relayed game.** The projection above is arithmetic, not measurement.
- **Simultaneous moves,** which the Apolyton reports single out as the slow mode.
- **Why the checksums mismatched.** Two of five checks did: leaders and vehicles, plus bases at the start. **[inference]** The transport keeps order and loses nothing, so the game's own state diverged. A cause on our side hasn't been ruled out, though, and resyncs are where the bytes are.
- **Why the host's compute took 4–10 s per phase.**
