# 09: Early ack capture lines, hop bucket and delivery check

**What to build:** Capture and the Turn sync log say what Early acks did, and the analysis can judge the target. The capture (still format 1) gains `early_ack` (peer, seq, the `out` line's id, withheld_us), `real_ack` (peer, seq, rtt_us), `resend_dropped`, `replay` (peer, count) and a once-per-Game-session `early_acks` line (on/off and why: switch, players, connection_lost). `analyse_capture.py` gains the one-way hop bucket (at each change of direction, our message's `written` to the `in` of the friend's next message; network time is hops plus DLL polls) and a delivery check: one `real_ack` per early ack, no gaps in the friend's seqs, every message drained once. The Turn sync and game start log lines gain hop time and Early ack counts (made, withheld, swallowed).

**Blocked by:** 02, 05, 07

**Status:** ready-for-agent

- [ ] New capture lines appear as described, documented in `docs/contributors/traffic-capture.md`
- [ ] The analysis reports hop time, network time and delivery-check results on a capture with early acks
- [ ] The Turn sync log lines carry the new fields
- [ ] A sample capture exercises the delivery check, including a failing case
