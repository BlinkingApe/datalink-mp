# 14: Validate fault injection (HITL)

**What to build:** Using `dev-faults`, run each fault during the game start's state sync. `DATALINK_FAULT=drop:N` must give the same outcome as 0.1.0, with no extra hang. `reset:N` must let play continue and pass the delivery check.

**Blocked by:** 10, 13

**Status:** ready-for-human

- [ ] `drop:N` outcome matches 0.1.0
- [ ] `reset:N` play continues and the delivery check passes
- [ ] Results are recorded in the issue
