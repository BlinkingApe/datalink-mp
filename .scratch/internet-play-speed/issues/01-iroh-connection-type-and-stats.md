# 01: How does a Helper tell a Direct from a Relayed connection, and what traffic stats does Iroh give?

Type: research
Status: resolved
Blocked by:

Research: branch `research/iroh-connection-stats` (commit `722e68a`), file `docs/research/iroh-connection-stats.md`.

## Question

The Helper uses `iroh` 1.0.2 (`crates/iroh-transport`, one `Connection` per friend, one long-lived ordered uni stream each way). In that version:

- How does the Helper find out whether a connection to a friend is a **Direct connection** or a **Relayed connection**, and learn when that changes mid-game (for example, a hole-punch that succeeds after the game starts)? Is there a watcher or event, or only polling?
- When relayed, can it tell which relay server is used (region or URL)?
- What per-connection stats does it expose: round-trip time, bytes and packets sent and received, lost or retransmitted packets, congestion window? Per path or per connection?
- Does anything in the endpoint's setup (the `N0` preset in `crates/iroh-transport/src/runtime.rs`) keep a connection relayed when a direct path could exist?

Cite the `iroh` 1.0 docs or source for each answer. [The traffic capture](03-dev-only-traffic-capture.md) logs these, and [What the Helper reports about each connection](07-what-the-helper-reports.md) builds on them.

## Answer

Resolved 2026-10-06 from iroh 1.0.2 and noq-proto 1.0.1 source. The details, with citations and a code sketch, are in `docs/research/iroh-connection-stats.md` on branch `research/iroh-connection-stats`.

- **Direct or Relayed:** there's a push API, so no polling is needed. `Connection::path_events()` is a stream of a path opened, closed or selected. `Connection::paths()` (a snapshot) and `paths_stream()` give each path's `is_selected()`, `is_ip()` and `is_relay()`. A connection is a **Direct connection** when its *selected* path is an IP path, because the relay path stays open as a backup after a hole-punch succeeds. There's no `ConnectionType` watcher in 1.0.2.
- **Which relay:** a relay path gives the relay's URL. There's no region API; the region only shows in n0's hostnames (`use1-1`, `euc1-1`, …).
- **Stats:** `Path::stats()` gives per-path RTT, congestion window, UDP datagrams and bytes each way, lost packets and bytes, congestion events and MTU. `Connection::stats()` only sums the counters across all paths, past and present, with no RTT or congestion window. There's no retransmit counter; lost packets and congestion events are the closest signals.
- **Our setup doesn't keep connections relayed.** The N0 preset leaves IPv4 and IPv6, port mapping, NAT traversal and hole-punch retries on. A pair that stays relayed does so because of its network (NAT type, firewall), not our config.
- **Noticed in our code, neither blocking a direct path:**
  - The comment in `runtime.rs` says the endpoint binds only IPv4, but it also binds IPv6.
  - The Ticket is built before the endpoint is online, so it usually carries no relay URL. That only affects how the first path is found (inferred from the source, not documented).

For [the capture](03-dev-only-traffic-capture.md): log `path_events()`, and sample the selected path's `Path::stats()` every second or so. For [what the Helper reports](07-what-the-helper-reports.md): "Direct" against "Relayed" (and the relay's host) can be shown live, and RTT and bytes come per path.
