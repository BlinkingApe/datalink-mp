# 16: Captured Relayed game and the compression condition (HITL)

**What to build:** Play a captured game with `DATALINK_RELAY_ONLY=1` on one Helper (a tester abroad if one can be scheduled), with early acks on. Judge against the Relayed target (≤ 1.5 s per Turn sync). Settle the compression condition: do bytes still add ≥ 1 s to a resync turn or the game start? If so, record that compression returns as a new effort (a Peer protocol change, so a minor release); if not, record that the speed work needs no Peer protocol change.

**Blocked by:** 03, 15

**Status:** ready-for-human

- [ ] Capture analysed and compared with the ≤ 1.5 s target
- [ ] The compression condition is answered, yes or no, with figures
