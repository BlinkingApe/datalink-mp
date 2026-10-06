# 13: Validate Early acks in Wine (HITL)

**What to build:** The maintainer and an agent play games with early acks on and captured: a two-instance Wine game covering the game start and a few turns, which must pass the delivery check, and a three-instance Wine game, where early acks must stay off and the game plays as in 0.1.0. Note the result of each, and whether `DATALINK_FAST_WAKE` was on.

**Blocked by:** 07, 08, 09, 11

**Status:** ready-for-human

- [ ] Two-instance game passes the delivery check
- [ ] Three-instance game shows `early_acks` off with reason `players` and plays normally
- [ ] Results are recorded in the issue
