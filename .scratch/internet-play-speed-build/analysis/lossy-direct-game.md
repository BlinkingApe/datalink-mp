# Analysis: a lossy long-distance Direct game

- **Date:** 2026-10-20 (game), 2026-10-21 (this write-up)
- **Capture:** `captures/2026-10-20-internet-test3/dl-logs_HOST_Internet_test3.7z` (gitignored). The game is `dl-logs/capture/capture-1792517016-7092.jsonl` inside it. Of the other three, `capture-1792515918-3608.jsonl` is an earlier attempt whose game start never finished (not analysed), and the two small ones hold no game. The archive also holds the host's `dll.log` and `helper.log`.
- **Setup:** two players, `wtp.exe`, on 0.1.3-rc.1 or later. The maintainer hosted (Windows) and captured. The friend joined from Russia on a laptop over Wi-Fi, on cable internet, and didn't log. A **Direct connection** over IPv4 the whole game (QUIC RTT median 102 ms, p90 121 ms). The players report that both one-player-at-a-time and simultaneous moves worked.
- **Reported by the players:** loading took 2–3 minutes, and 30–40 s passed between turns.
- **Serves:** [16](../issues/16-captured-relayed-game-and-compression-condition.md) (the compression condition), [19](../issues/19-captured-game-on-a-lossy-long-distance-link.md) and [20](../issues/20-congestion-controller-switch.md).
- **Reproduce:** `python3 ../../internet-play-speed/analysis/analyse_capture.py <capture>` (needs `zstandard`, `lz4`, `numpy`) for the windows, budgets, resends and compression. `python3 analysis/link_stats.py <capture> 'game start=19:31:58-19:35:33' 'turn sync 1=19:41:58-19:42:50' 'between=19:35:33-19:41:58'` for loss and the congestion window.

Facts are measured from the capture. **[inference]** marks a reading of them.

## TL;DR

1. **The link lost about 5% of packets under load.** 4.5% during the game start and 5.5% during Turn sync 1, against 0.8% between them. QUIC counted 56 congestion events during the game start and 22 during Turn sync 1.
2. **So QUIC kept its sending window small.** Its median was 3.3 packets during the game start and 2.0 (the minimum) during Turn sync 1, where it sat at the minimum for 70% of the time.
3. **A 2 KB message took 945 ms to be acked** (median), against a QUIC RTT of 97 ms. Small messages took 106–142 ms. In the earlier [captured Direct game](../../internet-play-speed/analysis/direct-game-capture.md), each KB added 8–14 ms. Here it added about 300 ms.
4. **The game's resends made it worse.** The game resends after about 180 ms (median), so most 2 KB messages went out two or three times, and some up to seven. The host's resends added 156% on top of 433 KB of real data. Every copy queued on the same stream as the next real message.
5. **Most of the game start was waiting on the network.** The game start took 214 s: 157 s round trips (73%), 42 s the friend's machine computing (20%), and 14 s the host computing (6%). Turn sync 1 took 51 s, 95% of it round trips.
6. **The payloads compress well.** Messages of 1 KB or more compress to 8% (zstd-3, streamed) or 12% (zstd-3, per message). All payloads together compress to 10% streamed and 15% per message.

## The windows

| | Took | Host msgs (bytes) | Host sends with resends | Round trips | Joiner compute | Host compute |
|---|---|---|---|---|---|---|
| Game start, 19:31:58 → 19:35:32 | 214.4 s | 190 (285 KB) | 464 (768 KB) | 157.4 s (73%) | 41.9 s (20%) | 13.7 s (6%) |
| Turn sync 1, 19:41:58 → 19:42:49 | 51.1 s | 103 (131 KB) | 223 (310 KB) | 48.7 s (95%) | 0.6 s | 1.1 s |

- In the game start, 155 s went to the phase-0 data transfer (167 host messages, mostly `0x4101` sync records). The friend's computing was mostly one 29 s stretch before phase 4 and an 11.5 s stretch at the end of phase 0.
- Turn sync 1's 46.7 s phase 7 carried 81 host messages (mostly `0x4101`). In the earlier game phase 7 ran only after a checksum mismatched, but the analysis finds no mismatched slot in either checksum check here. Why it ran is open.

## The link

| Window | Datagrams | Lost | Congestion events | cwnd median | Sent |
|---|---|---|---|---|---|
| Game start | 1485 | 67 (4.5%) | 56 | 3.3 pkts | 947 KB in 216 s |
| Turn sync 1 | 505 | 28 (5.5%) | 22 | 2.0 pkts | 384 KB in 53 s |
| Between them | 839 | 7 (0.8%) | 5 | 2.0 pkts | 124 KB in 386 s |

**[inference]** Loss rose only when the game sent hard, which fits a congested Wi-Fi hop or queue somewhere on the long route rather than a fixed random loss. The capture can't tell which.

The friend's spare paths opened and closed several times, but the selected path stayed the same Direct IPv4 path all game. The `sendmsg error 10040` warnings in the Helper log come from iroh's IPv6 checks against the relay server, not from traffic to the friend.

## Resends

| Copy | n | Median gap |
|---|---|---|
| 1 → 2 | 237 | 181 ms |
| 2 → 3 | 109 | 436 ms |
| 3 → 4 | 48 | 495 ms |
| 4 → 7 | 35 | 472–706 ms |

The host sent 363 messages as 792 copies (429 resends). Only 8 copies went out after the real ack had already reached the Helper. **[inference]** The rest were the game's timer firing while the first copy was still queued behind a small sending window. Each resend took more of that window and delayed the next message.

## What the fixes should do here

All **[inference]**, a rough estimate, not a replay model.

- **Early acks** ([07](../issues/07-early-acks-core.md)) remove the 945 ms waits and the 156% resends, because the game gets its ack at once and the Helper drops resends the stream already carries. The link then moves only the real data. At the windows measured (2–3.3 packets of 1452 B per ~100 ms RTT, about 29–48 KB/s), the game start's 285 KB takes about 6–10 s and Turn sync 1's 131 KB about 3–4.5 s. That's against 157 s and 49 s of round trips today. The cap of 16 messages in flight is not the limit here: 16 × 2 KB is several times what the window lets through in a round trip.
- **Compression** then matters. Those 6–10 s and 3–4.5 s are all bytes, so at 8–12% they become about 0.5–1.2 s and 0.3–0.5 s. ADR-0005's condition (bytes still adding ≥ 1 s to a resync turn or the game start after Early acks) looks met on this link. A capture taken with Early acks on has to confirm it, so [19](../issues/19-captured-game-on-a-lossy-long-distance-link.md) is that capture.
- **The congestion controller** ([20](../issues/20-congestion-controller-switch.md)) could raise the 29–48 KB/s. The default controller halves its window on each loss and sat near its minimum. BBR is designed not to treat every lost packet as congestion.
- **Not touched:** the friend's 42 s of computing during the game start. **[inference]** Even with every fix, loading on this pairing stays around 45–60 s.
