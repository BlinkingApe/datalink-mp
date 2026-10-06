# 02: Ship the traffic capture in main

**What to build:** The dev-only traffic capture (built on branch `capture/traffic-capture`) is merged into main and ships in the Helper, off by default behind `DATALINK_CAPTURE=<dir>`, so any tester's game can be analysed. Its format is in `docs/traffic-capture.md`; the analysis script and its summary move with it.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] The capture is on main and does nothing unless `DATALINK_CAPTURE` is set
- [x] `docs/traffic-capture.md` ships with it
- [x] The capture's tests pass on main
