# 04: Direct/Relayed badge and path-change log

**What to build:** Each friend's row on the page shows plain text, "Direct" or "Relayed (slower)", with no alarm colour and nothing until a path is selected. It changes when the connection changes path. The Helper logs at info level, always on, a line per path change: peer, Direct or Relayed, and the remote address kind (IPv4, IPv6, relay). Decided in `.scratch/internet-play-speed/issues/07-what-the-helper-reports.md`.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The badge appears on each friend's row once a path is selected and updates on a path change
- [ ] No RTT or other figures on the page
- [ ] A log line per path change with peer, kind and address kind
- [ ] UI tests cover the badge
