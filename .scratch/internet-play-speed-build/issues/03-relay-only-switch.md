# 03: DATALINK_RELAY_ONLY switch

**What to build:** Setting `DATALINK_RELAY_ONLY=1` on a Helper forces its connections onto a **Relayed connection** (Iroh's `clear_ip_transports()` on the endpoint builder), so a tester abroad can capture a Relayed game without a special build. It ships in the Helper, undocumented for players, and logs at start that it is on.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With the switch on, a connection to a friend ends up Relayed
- [ ] With it off, nothing changes
- [ ] It logs when on
- [ ] A test or recorded manual check shows the path is Relayed
