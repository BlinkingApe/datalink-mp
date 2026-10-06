# 05: Analyse the capture

Type: task
Status: resolved
Blocked by: 02, 04

AFK: the agent analyses the capture offline and commits a summary. The raw captures stay gitignored.

## Question

What does a Turn sync consist of, and where does its time go? From [the captured game](04-play-a-captured-internet-game.md), find:

- **Turn syncs:** how to find each one in the traffic (an idle gap, or a message kind), and each one's duration, message count and bytes, per direction.
- **Payloads:** the size distribution. How well do the large ones compress, with a few candidates (for example zstd, lz4 and deflate, at their usual levels)? How much do they shrink when encoded as a delta from the previous message of the same kind, before and after compression?
- **Time:** network round-trip time against the gaps between messages. Does the game send in bursts that then wait for something, the way windowed acks would? Using [what's known about JACKAL](02-what-is-known-about-jackal-and-turn-sync.md), is it the game's own pacing? How long do inbound messages sit in the Helper before the DLL drains them?
- **Retransmits:** repeated identical payloads in a short window.
- **Connection:** Direct or Relayed, and whether that changed during the game.
- **A rough budget:** for one slow Turn sync, how much of its time each cause accounts for (bytes on the wire, round trips, Helper queueing, game pacing).

Check the data against [the JACKAL research](02-what-is-known-about-jackal-and-turn-sync.md), and answer its open questions (section 9 of `docs/research/smac-jackal-turn-sync.md`):
- Does every send arrive with `dwFlags = 0`?
- How many reliable messages and barriers make up a Turn sync?
- How many retransmits are there, and are they spaced at 50 ms × 1.5ⁿ?
- What fraction of a Turn sync is round trips against compute?
- Are the header offsets right?

Commit the summary as a file in this directory, and link it from the answer. It feeds [What makes a Turn sync slow, and what do we fix?](06-what-makes-turn-sync-slow.md).

## Answer

2026-10-06. The full analysis is [the captured Direct game](../analysis/direct-game-capture.md). It's reproducible with [`analysis/analyse_capture.py`](../analysis/analyse_capture.py), which also works on a later capture, for example a Relayed one.

**A Turn sync is mostly the game computing, not the network.** In the four Turn syncs (6.1, 24.7, 9.1 and 17.0 s), 67–78% of the time nothing was in flight while one machine ran its upkeep. Usually that was the Linux host, which spent up to 10 s on a single phase. Round trips took 19–29%, the DLL's 10 ms poll took 1–3%, and the Helper's own queueing was nil. The game start (38.6 s, including the initial state sync) is the exception, at 54% round trips.

- **Finding a Turn sync:** by message type, not by idle gap. It starts at the Joiner's `0x8301` and the host's `0x4301`, has a barrier per phase (`0x2303` then `0x4303`, phase and turn at wire offsets 24 and 28), and ends at the host's `0x4309`. A normal one is 28–41 reliable messages and 4–7 barriers, about 2–7 KB.
- **Payloads:** 195 of 605 messages are 1–2 KB and carry 89% of the bytes. They compress to 9–16% (streamed zstd-3 or deflate-6 is best). Delta encoding beats compression alone only on resyncs (6% against 10%). Each KB adds about 8–14 ms to a message's round trip, so bytes matter only for the game start and resyncs. Turn sync 1 resent every faction's leader data after a checksum mismatch: 110 KB and 7.9 s.
- **Time:** strictly one message in flight on both sides, with no bursts. A message's ack wait is the QUIC RTT plus 5–8 ms. Each received message waits about 6 ms for the DLL's poll.
- **Retransmits:** 70% of the host's messages were sent more than once (339 resends, +77% bytes), and all of them were spurious. The timer starts at 50 ms and backs off about ×1.5, but it stays at 50–70 ms, under the RTT, all game.
- **Connection:** Direct throughout, RTT 52–77 ms median per Turn sync.
- **Budget for the slowest Turn sync (24.7 s):** about 16.7 s compute, 5.5 s network RTT, 0.9–1.5 s bytes, 1.2 s DLL polling (both ends), and 0 s Helper queueing.

**The research's open questions:**
1. Every send has `dwFlags = 0`.
2. A Turn sync is 28–41 reliable messages (106 with a resync) and 4–7 barriers.
3. The retransmits are as above.
4. Round trips are 19–29% of a Turn sync, and compute is 67–78%.
5. The header offsets are confirmed, and the DLL's ready/lobby labels do read the wrong offset.

The research's §3.1 needs one correction: a one-at-a-time game uses `0x4309` to hand the turn over, *and* `0x8301`/`0x4301` to end it.

**Upper bounds for the fixes, on this capture:** an ack that costs no round trip would save up to 21–32% of a Turn sync, and an event-driven DLL instead of the 10 ms poll about 5%. Compression would save 3–6% on a resync turn and under 1% otherwise. Nothing between Helpers touches compute. A Relayed connection, measured at 146 ms RTT before the game, would make Turn sync 1 about 33 s (projected, not measured).
