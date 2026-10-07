# 02: Ship the traffic capture in main

**What to build:** The dev-only traffic capture (built in [internet-play-speed 03](../../internet-play-speed/issues/03-dev-only-traffic-capture.md)) is merged into main and ships in the Helper, off by default behind `DATALINK_CAPTURE=<dir>`, so any tester's game can be analysed. Its format is in `docs/contributors/traffic-capture.md`; the analysis script and its summary move with it.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] The capture is on main and does nothing unless `DATALINK_CAPTURE` is set
- [x] `docs/contributors/traffic-capture.md` ships with it
- [x] The capture's tests pass on main
