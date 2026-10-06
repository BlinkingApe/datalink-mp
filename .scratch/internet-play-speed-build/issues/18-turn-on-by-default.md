# 18: Turn Early acks and fast wake on by default (HITL)

**What to build:** Once at least 3 real tester games with both switches on have been played, one with Windows hosting, and their captures show no hang, no `SEND MESSAGE TIME EXPIRED` and no delivery fault, flip both to on by default. The same variables become the off switches (`=0`). Release it as a 0.1.x patch. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md), Patch C.

**Blocked by:** 16, 17

**Status:** ready-for-human

- [ ] Three tester games reviewed, one with Windows hosting, no faults
- [ ] Both defaults flipped and `=0` turns each off
- [ ] Release notes written and the gate passes
