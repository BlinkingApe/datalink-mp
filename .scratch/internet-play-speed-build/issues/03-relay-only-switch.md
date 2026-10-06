# 03: DATALINK_RELAY_ONLY switch

**What to build:** Setting `DATALINK_RELAY_ONLY=1` on a Helper forces its connections onto a **Relayed connection** (Iroh's `clear_ip_transports()` on the endpoint builder), so a tester abroad can capture a Relayed game without a special build. It ships in the Helper, undocumented for players, and logs at start that it is on.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With the switch on, a connection to a friend ends up Relayed
- [x] With it off, nothing changes
- [x] It logs when on
- [ ] A test or recorded manual check shows the path is Relayed

## Comments

Implemented in `crates/iroh-transport/src/runtime.rs` (`relay_only_enabled`, applied in `init_async`). Unset, empty, `0` and `false` are off; anything else is on. When on, the Helper logs at info: `DATALINK_RELAY_ONLY is on: IP transports cleared, all connections will be Relayed`.

The env parsing is unit-tested (`relay_only_switch_reads_env_value`). A Relayed path cannot be asserted offline: with IP transports cleared the endpoint needs a reachable relay server.

Manual check (needs internet): start two Helpers with `DATALINK_RELAY_ONLY=1 DATALINK_CAPTURE=<dir>`, connect them, and confirm the capture path and stats lines show a relay route for the selected path and never a direct IP one. Without the variable the same pair goes direct on one network.

Reopened after review: the Relayed path has not been observed. Run the manual check above and record the result here before resolving.
