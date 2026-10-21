# 19: Captured game on a lossy long-distance link (HITL)

**What to build:** Re-play the [lossy Direct game](../analysis/lossy-direct-game.md) with Early acks on: the maintainer hosting, the same friend in Russia joining, two players. Capture on the host only (`DATALINK_CAPTURE`). In a two-player game one Helper sees both directions, so the friend needn't log. The friend can run any 0.1.x; Early acks and the congestion controller work from the host's Helper alone.

Play two games back to back, since loss varies with the time of day. Each covers the game start and at least two turns.

- **Game A:** `DATALINK_EARLY_ACKS=1` on the host, with the default congestion controller.
- **Game B:** the same, plus `DATALINK_CONGESTION=bbr` on the host ([20](20-congestion-controller-switch.md)).

Run `DATALINK_FAST_WAKE=1` the same way in both and note whether it was on.

Analyse both with `analyse_capture.py` and `analysis/link_stats.py`, and record for the game start and each Turn sync:
- network time;
- bytes and their share of the round trips (the compression condition);
- loss, congestion events and the congestion window;
- Early ack counts, including how long early acks were withheld at the cap;
- the delivery check.

The ≤ 1 s / ≤ 2 s targets were set on the reference setup and probably won't hold here without compression, so record figures, not a pass or fail. The figures feed [16](16-captured-relayed-game-and-compression-condition.md)'s compression answer and 20's default.

**Blocked by:** 13, 20

**Status:** ready-for-human

- [ ] Both games captured and analysed, with the figures above against the [first game](../analysis/lossy-direct-game.md)
- [ ] The delivery check passes in both
- [ ] Bytes' share of the game start and each Turn sync recorded for 16
- [ ] BBR against the default compared, and a recommendation recorded on 20: keep it opt-in, or plan it as a default
- [ ] Results are recorded in the issue
