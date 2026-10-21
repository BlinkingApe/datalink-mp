# 09: Early ack capture lines, hop bucket and delivery check

**What to build:** Capture and the Turn sync log say what Early acks did, and the analysis can judge the target. The capture (still format 1) gains `early_ack` (peer, seq, the `out` line's id, withheld_us), `real_ack` (peer, seq, rtt_us), `resend_dropped`, `replay` (peer, count) and a once-per-Game-session `early_acks` line (on/off and why: switch, players, connection_lost). `analyse_capture.py` gains the one-way hop bucket (at each change of direction, our message's `written` to the `in` of the friend's next message; network time is hops plus DLL polls) and a delivery check: one `real_ack` per early ack, no gaps in the friend's seqs, every message drained once. The Turn sync and game start log lines gain hop time and Early ack counts (made, withheld, swallowed). They also gain the link's figures over the sync, from the selected path's `Path::stats()` (the fields the capture's `stats` line already reads): datagrams sent, packets lost, congestion events and the median congestion window in packets (`cwnd` / `current_mtu`). Players send the log and a capture only when asked, and a lossy link should show in the log alone.

The counts and the `out` line's id come from the friend pipeline (`friend_pipeline.rs`), which already writes the game-message capture lines in one fixed order, capture line first and then the Turn sync detector. The pipeline has no snapshot interface yet; this ticket adds it for the counts.

**Blocked by:** 02, 05, 07

**Status:** resolved

- [x] New capture lines appear as described, documented in `docs/contributors/traffic-capture.md`
- [x] The analysis reports hop time, network time and delivery-check results on a capture with early acks
- [x] The Turn sync log lines carry the new fields
- [x] The Turn sync log lines carry datagrams sent, packets lost, congestion events and the median congestion window over the sync
- [x] A sample capture exercises the delivery check, including a failing case

## Comments

- 2026-10-21: Added the link figures to the Turn sync log lines. Diagnosing the [lossy Direct game](../analysis/lossy-direct-game.md) (about 5% loss, a 2–3 packet window) needed the capture's per-second `stats` lines; the log's median RTT alone looked normal.
