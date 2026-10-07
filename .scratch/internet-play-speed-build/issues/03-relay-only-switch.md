# 03: DATALINK_RELAY_ONLY switch

**What to build:** Setting `DATALINK_RELAY_ONLY=1` on a Helper forces its connections onto a **Relayed connection** (Iroh's `clear_ip_transports()` on the endpoint builder), so a tester abroad can capture a Relayed game without a special build. It ships in the Helper, undocumented for players, and logs at start that it is on.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] With the switch on, a connection to a friend ends up Relayed
- [x] With it off, nothing changes
- [x] It logs when on
- [x] A test or recorded manual check shows the path is Relayed

## Comments

Implemented in `crates/datalink-transport/src/runtime.rs` (`relay_only_enabled`, applied in `init_async`). Unset, empty, `0` and `false` are off; anything else is on. When on, the Helper logs at info: `DATALINK_RELAY_ONLY is on: IP transports cleared, all connections will be Relayed`.

The env parsing is unit-tested (`relay_only_switch_reads_env_value`). A Relayed path cannot be asserted offline: with IP transports cleared the endpoint needs a reachable relay server.

Manual check (needs internet): start two Helpers with `DATALINK_RELAY_ONLY=1 DATALINK_CAPTURE=<dir>`, connect them, and confirm the capture path and stats lines show a relay route for the selected path and never a direct IP one. Without the variable the same pair goes direct on one network.

Manual check done 2026-10-07 (two Helpers with `DATALINK_RELAY_ONLY=1 DATALINK_CAPTURE`, captures in `~/Games/AC-WTP_431`: `capture-1791356538-1222191-HOST.jsonl`, `capture-1791356636-8110-JOINER.jsonl`). Each capture has 402 `stats` lines; every one has `route":"relayed"`, `open_paths":1`, `addr":"https://euc1-1.relay.n0.iroh.link./"`, and none has a direct IP route. RTT about 45-54 ms via the relay. The Relayed path is observed; resolved.
