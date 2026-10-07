# Research: Direct/Relayed status and connection stats in iroh 1.0.2

Supports the internet-play-speed work. This covers how the Helper can tell whether each friend's connection is a Direct connection or a Relayed connection, notice when that changes mid-game, name the relay, read per-connection stats, and whether anything in our endpoint setup keeps connections relayed.

Sources: this repo at commit `7d26254`, plus the locked dependency sources `iroh 1.0.2`, `iroh-base 1.0.2`, `noq 1.0.1` and `noq-proto 1.0.1` (from `Cargo.lock`, read under `~/.cargo/registry/src`). Paths below like `iroh-1.0.2/src/...` are relative to that registry directory. The method list on [docs.rs: `iroh::endpoint::Connection` 1.0.2](https://docs.rs/iroh/1.0.2/iroh/endpoint/struct.Connection.html) matches the source. Nothing was run; no production code was changed.

## TL;DR

1. **Direct vs Relayed: there is a push API, no polling needed.** `Connection::path_events()` yields a `'static` stream of `PathEvent`s (`Opened`, `Closed`, `Selected`, `Lagged`). `Connection::paths()` gives a snapshot `PathList`, where each `Path` has `is_selected()`, `is_ip()` and `is_relay()`. "Direct connection" should mean **the selected path is an IP path**. The relay path usually stays open as a backup after hole-punching succeeds, so "has any IP path" isn't the same thing.
2. **Relay identity: the URL, yes. A region field, no.** A relay path's `remote_addr()` is `TransportAddr::Relay(RelayUrl)`. iroh has no region type. The region is only implied by the n0 hostnames (`use1-1`, `usw1-1`, `euc1-1`, `aps1-1` `.relay.n0.iroh.link`).
3. **Stats: per path and per connection.**
   - Per path, `Path::stats() -> PathStats` gives `rtt`, `cwnd`, UDP datagrams and bytes each way, `lost_packets`, `lost_bytes`, congestion events, MTU and frame counts.
   - Per connection, `Connection::stats() -> ConnectionStats` sums the counters over all paths, past and present. It has **no RTT and no cwnd**; those exist only per path.
   - **No retransmission counter exists.** `lost_packets` and `congestion_events` are the closest proxies.
4. **Our setup doesn't disable anything.** `Endpoint::builder(N0)` keeps iroh's defaults:
   - IPv4 and IPv6 wildcard sockets (the comment in `runtime.rs` mentions only IPv4);
   - the portmapper (UPnP/PCP/NAT-PMP);
   - QUIC NAT traversal with multipath;
   - relay treated as backup;
   - hole-punch retries every 5 s when address candidates change, and an upgrade check every 60 s.

   One built-in limit: only the **dialing (client) side** starts hole-punching. Whatever keeps a pair relayed is the network (NAT type, firewall), not our config.

---

## 1. Direct or Relayed, and noticing a change

### 1.1 The API

All of this is on `iroh::endpoint::Connection` (= `Connection<HandshakeCompleted>`, `iroh-1.0.2/src/endpoint/connection.rs:737`), which is the type `PeerConnection.connection` already holds (`crates/iroh-transport/src/connection.rs:88`).

| Method | Returns | Behaviour |
|---|---|---|
| `paths(&self)` | `PathList<'_>` | Snapshot of the currently open paths. It doesn't update and leaves out closed paths (`connection.rs:1131-1146`). |
| `paths_stream(&self)` | `PathListStream<'_>` (a `Stream<Item = PathList>`) | Yields the current snapshot first, then a new one "whenever the open paths or the selected path change". Ends when the connection closes. **It borrows the `Connection`**, so move a clone into the task and call it there (`connection.rs:1148-1159`). |
| `path_events(&self)` | `PathEventStream` (a `Stream<Item = PathEvent>`, `'static`) | One event per change. Ends when the connection closes, and doesn't borrow, so it can be moved into a task (`connection.rs:1161-1178`). |

`PathEvent` is `#[non_exhaustive]`, and so is each of its variants (`iroh-1.0.2/src/socket/remote_map/remote_state/path_watcher.rs:52-100`):

- `Opened { id: PathId, remote_addr: TransportAddr, local_addr: LocalTransportAddr }`
- `Closed { id, remote_addr, local_addr, last_stats: Box<PathStats> }`. Final stats arrive here, because `PathList` drops closed paths (`path_watcher.rs:9-10`, `:347-352`).
- `Selected { id, remote_addr, local_addr }`: "This path was selected for transmission of application data."
- `Lagged { missed: u64 }`. The broadcast buffer holds only **8** events (`path_watcher.rs:50`). After a lag, re-read `conn.paths()`.

`Path<'a>` (`path_watcher.rs:446-497`) has `id()`, `remote_addr() -> &TransportAddr`, `local_addr()`, `is_selected()`, `is_ip()`, `is_relay()`, `stats() -> PathStats` and `rtt() -> Duration`. A `Path` borrows the connection, so copy out what you need before crossing a task boundary (`path_watcher.rs:437-443`).

The module doc recommends subscribing to events **before** reading the snapshot, so that no change is missed in between (`path_watcher.rs:4-7`).

All of these types are re-exported from `iroh::endpoint` (`iroh-1.0.2/src/endpoint.rs:51-58`). `PathStats`, `ConnectionStats` and `PathId` come from the same module (`endpoint.rs:96-117`).

### 1.2 Why "selected" matters

iroh keeps several paths open. After hole-punching, the selected path is marked `Available` and **every other path, including the relay path, becomes `Backup`** but stays open (`iroh-1.0.2/src/socket/remote_map/remote_state.rs:679-681`, `:729`, `:1073-1079`). The default selector sorts by "biased RTT (with IPv6 preferred over IPv4 and relay treated as backup)" and is "sticky to avoid flapping" (`endpoint.rs:820-823`).

So:

- **Direct connection** = the path with `is_selected()` has `is_ip()`.
- **Relayed connection** = the selected path has `is_relay()`.
- The selection can go back to the relay if the direct path dies. A `Selected` event covers that case too.

### 1.3 Sketch

Iterating the stream needs a `StreamExt`. The streams implement `tokio_stream::Stream` (`path_watcher.rs:37-40`, `:514`, `:548`). `iroh-transport` doesn't depend on a stream-ext crate today, so it would need to add `n0-future` (0.3.2 is already in `Cargo.lock`) or `tokio-stream` (0.1.18 is also locked).

```rust
use iroh::endpoint::{Connection, PathEvent};
use iroh::TransportAddr;
use n0_future::StreamExt;

#[derive(Debug, Clone)]
enum Route { Direct(std::net::SocketAddr), Relayed(iroh::RelayUrl), Unknown }

fn current_route(conn: &Connection) -> Route {
    match conn.paths().iter().find(|p| p.is_selected()).map(|p| p.remote_addr().clone()) {
        Some(TransportAddr::Ip(addr)) => Route::Direct(addr),
        Some(TransportAddr::Relay(url)) => Route::Relayed(url),
        _ => Route::Unknown, // no selection yet, or a custom transport (TransportAddr is #[non_exhaustive])
    }
}

// Spawned once per friend, next to the existing connection handler.
fn spawn_route_watcher(conn: Connection, on_change: impl Fn(Route) + Send + 'static) {
    tokio::spawn(async move {
        let mut events = conn.path_events();          // subscribe first...
        on_change(current_route(&conn));              // ...then read the snapshot
        while let Some(ev) = events.next().await {
            match ev {
                PathEvent::Selected { .. } | PathEvent::Lagged { .. } => on_change(current_route(&conn)),
                _ => {}
            }
        }
        // stream ended: connection closed
    });
}
```

This follows the pattern in iroh's own `examples/transfer.rs:937-975`: `path_events()` plus `paths().iter().find(|p| p.is_selected())` on each `Selected`. iroh's own holepunch test waits for direct with `paths_stream()` and `infos.iter().any(|i| i.is_ip())` (`endpoint.rs:2426-2433`). That's fine for a test, but for the reason in 1.2, the Helper should check `is_selected()`.

### 1.4 Other observation points

- `Endpoint::remote_info(EndpointId) -> Option<RemoteInfo>` is async (`endpoint.rs:1611-1625`). It returns every known `TransportAddrInfo { addr, usage: Active | Inactive }` for a remote (`remote_info.rs:56-93`). It's a snapshot, "not a watcher", and doesn't say which path is selected. It's less useful than `paths()`.
- Endpoint-wide counters are available through `Endpoint::metrics()`; the `metrics` feature is on by default (`iroh-1.0.2/Cargo.toml` `[features] default`, `endpoint.rs:1607-1609`). They include `socket.holepunch_attempts` (client side only), `paths_direct`, `paths_relay` and `num_conns_direct` (`iroh-1.0.2/src/socket/metrics.rs:44-65`). They aren't per friend, so they're only useful for logs.
- **There's no `ConnectionType`/`conn_type` watcher in 1.0.2.** A grep of `iroh-1.0.2/src` finds none. The path APIs above replace it.

## 2. Which relay server is used

- A relay path's `remote_addr()` is `TransportAddr::Relay(RelayUrl)` (`iroh-base-1.0.2/src/endpoint_addr.rs:50-62`). Its `local_addr()` is `LocalTransportAddr::Relay(RelayUrl)`, documented as "The relay over which this network path is connected" (`iroh-1.0.2/src/socket/transports.rs:884-894`). For a relay path, both come from the same URL (`transports.rs:902-918`).
- So **the URL is available**: `Path::remote_addr()` from `paths()`, or `remote_addr` on `PathEvent::Opened` and `PathEvent::Selected`.
- **There is no region field or region API.** `RelayUrl` is a wrapper around `Url` (`iroh-base-1.0.2/src/relay_url.rs:21`). The N0 preset uses `RelayMode::Default` (`iroh-1.0.2/src/endpoint/presets.rs:115-140`), which is these four servers (`iroh-1.0.2/src/defaults.rs:26-43`):
  - `use1-1.relay.n0.iroh.link` (NA east)
  - `usw1-1.relay.n0.iroh.link` (NA west)
  - `euc1-1.relay.n0.iroh.link` (EU)
  - `aps1-1.relay.n0.iroh.link` (Asia-Pacific)

  A friendly name like "EU relay" would have to come from our own hostname-to-label table.
- The Helper's **own** home relay can be read separately via `Endpoint::addr()` or `watch_addr()` (the `Relay` entries; `endpoint.rs:1184-1196`, `:1270`, `:1297-1305`).
- Relay latencies are in `Endpoint::net_report`, which is gated behind the `unstable-net-report` feature (`endpoint.rs:1392`, `iroh-1.0.2/src/lib.rs:295-301`). The Helper doesn't enable it.

## 3. Per-connection and per-path stats

### 3.1 Per path: `PathStats` (`noq-proto-1.0.1/src/connection/stats.rs:213-246`, `#[non_exhaustive]`)

| Field | Meaning |
|---|---|
| `rtt: Duration` | RTT estimate for this path |
| `udp_tx`, `udp_rx: UdpStats` | `datagrams`, `bytes`, `ios` each way (`stats.rs:10-25`) |
| `frame_tx`, `frame_rx: FrameStats` | count per QUIC frame type (`stream`, `acks`, `ping`, `path_challenge`, …) (`stats.rs:36-75`) |
| `cwnd: u64` | current congestion window |
| `congestion_events`, `spurious_congestion_events: u64` | |
| `lost_packets`, `lost_bytes: u64` | packets/bytes declared lost on this path |
| `sent_plpmtud_probes`, `lost_plpmtud_probes`, `black_holes_detected: u64`, `current_mtu: u16` | MTU discovery |

How to read them:

- `Path::stats()` and `Path::rtt()` on a `PathList` entry (`path_watcher.rs:484-497`). These are live for open paths, and noq keeps the final value for a path that closed after the snapshot (`noq-1.0.1/src/path.rs:177-186`).
- `PathEvent::Closed { last_stats }` for paths that have gone.
- `Connection::rtt(path_id: PathId) -> Option<Duration>` and `Connection::congestion_state(path_id) -> Option<Box<dyn Controller>>` (`iroh-1.0.2/src/endpoint/connection.rs:1014-1030`). `Controller::window()` is the cwnd (`noq-proto-1.0.1/src/congestion.rs:101-102`). Both need a `PathId`, which you get from `Path::id()`.
- **noq's `path_stats(PathId)` is not exposed on iroh's `Connection`.** iroh wraps `noq::Connection` privately. `Path::stats()` is the only route.

### 3.2 Per connection: `Connection::stats() -> ConnectionStats` (`iroh-1.0.2/src/endpoint/connection.rs:1020-1024`)

`ConnectionStats` (`noq-proto-1.0.1/src/connection/stats.rs:251-269`, `#[non_exhaustive]`) contains `udp_tx`, `udp_rx`, `frame_tx`, `frame_rx`, `lost_packets` and `lost_bytes`. These are "a sum of the respective fields in the `PathStats` for all the paths that exist as well as all paths that previously existed". The sum deliberately leaves out `rtt`, `cwnd`, `current_mtu` and the congestion and MTU counters (`stats.rs:271-300`; `noq-proto-1.0.1/src/connection/mod.rs:2629-2641`).

- **Bytes and packets sent/received:** yes, per connection and per path. Packets are counted as UDP datagrams. On a relay path these are QUIC datagrams sent through the relay, not raw HTTPS bytes to the relay server.
- **Lost packets:** yes (`lost_packets`, `lost_bytes`).
- **Retransmitted packets:** **don't exist in 1.0.2.** No stats struct has a retransmit counter (a grep for `retrans` in `noq-proto-1.0.1/src/connection/stats.rs` finds nothing). QUIC re-sends lost *frames* in new packets, so `lost_packets` and `congestion_events` are the usable signals.
- **RTT and congestion window:** **per path only.** For "the friend's RTT", read the selected path's `stats().rtt`.

### 3.3 Sketch

```rust
fn snapshot(conn: &Connection) -> Option<(bool, std::time::Duration, u64, u64, u64)> {
    let paths = conn.paths();
    let sel = paths.iter().find(|p| p.is_selected())?;
    let s = sel.stats();                        // PathStats, selected path only
    let c = conn.stats();                       // ConnectionStats, all paths ever
    Some((sel.is_ip(), s.rtt, s.cwnd, c.lost_packets, c.udp_tx.bytes))
}
```

All of these calls are cheap and synchronous. They take short internal locks and don't await, so a periodic status tick (e.g. 1 s) can poll them for the "current numbers", while `path_events()` covers the route changes.

## 4. Does our endpoint setup keep connections relayed?

The only endpoint construction is `crates/iroh-transport/src/runtime.rs:134-138`:

```rust
Endpoint::builder(iroh::endpoint::presets::N0)
    .secret_key(secret_key)
    .alpns(vec![peer_protocol_alpn(...)])
    .bind()
```

A grep of `crates/` finds no `relay_mode`, `clear_ip_transports`, `bind_addr`, `portmapper_config` or `transport_config`. `iroh = "1.0"` uses default features (`Cargo.toml:21`). So everything below is iroh's default.

| Item | Default in 1.0.2 | Source |
|---|---|---|
| IP sockets | IPv4 `0.0.0.0` **and** IPv6 `[::]`, both wildcard. The IPv6 bind "is allowed to fail". The comment at `runtime.rs:132-133` mentions only IPv4 and is incomplete. | `iroh-1.0.2/src/endpoint.rs:190-196`, `:318-325` |
| Relay | n0 production relays (4 regions) | `presets.rs:136`, `defaults.rs:36-43` |
| Address lookup | n0 DNS/pkarr. The publisher publishes **relay URL only** by default, to avoid leaking IPs. | `presets.rs:118-134`, `address_lookup/pkarr.rs:168`, `:262-263` |
| Port mapping | `PortmapperConfig::Enabled` (UPnP, PCP, NAT-PMP); the `portmapper` feature is on by default | `endpoint.rs:781-790`, `Cargo.toml` features |
| QUIC NAT traversal + multipath | on: up to 8 multipath paths and 32 NAT candidates; keep-alive 5 s; path idle timeout 15 s (relay path 30 s) | `iroh-1.0.2/src/endpoint/quic.rs:150-163`, `src/socket.rs:109-145` |
| Path choice | `BiasedRttPathSelector`: IPv6 > IPv4, relay is backup, sticky | `endpoint.rs:214`, `:820-823` |
| Hole-punch schedule | Triggered on connect and whenever local or remote candidate addresses change. Without new candidates, at most every **5 s**. Every **60 s**, a check re-triggers it if no IP path has RTT ≤ 10 ms. | `remote_state.rs:47-65`, `:252`, `:285-316`, `:504-568`, `:747-783` |

So **nothing in our configuration disables or limits hole-punching or IPv6.** Points that do affect it:

1. **Only the client (dialing) side starts hole-punching** (`remote_state.rs:504-531`; the metrics doc says "only the client-side of a connection will increment this counter", `socket/metrics.rs:44-48`). Each friend pair in the Helper has one connection, and the dialer is the client (`connection.rs:343-386`), so every pair has an initiator. Hole-punching is not blocked. When diagnosing, though, expect `holepunch_attempts` logs only on the side that dialed.
2. **Our Ticket is made before the endpoint is online.** `endpoint.addr()` is read straight after `bind()` (`runtime.rs:152-153`). iroh says to await `Endpoint::online()` first if the address should be "dialable by a remote endpoint over the internet" (`endpoint.rs:1184-1196`, `:1308-1312`). So the Ticket usually has only local interface addresses and no relay URL. The dialer then finds the relay URL via DNS lookup (relay-only, see table). This affects how the **first** path is found, and adds a lookup on the critical path. It doesn't stop the later upgrade to direct: NAT-traversal candidates travel in-band over the QUIC connection (`conn.get_remote_nat_traversal_addresses()` / `initiate_nat_traversal_round()`, `remote_state.rs:532-538`, `:924-927`). This is an inference from the source, not something iroh documents.
3. **What remains is environmental, not configuration.** Hole-punching can still fail behind symmetric NAT or CGNAT on both ends, with no UPnP/PCP on the router, or when a host firewall blocks inbound UDP. Those friends will stay Relayed whatever the Helper does, and the `PathEvent` stream is how the Helper will see it.

## Appendix: what doesn't exist in 1.0.2

- A `ConnectionType` enum or `conn_type()` watcher. It's replaced by `paths()`, `paths_stream()` and `path_events()`.
- A relay *region* field. There's only the `RelayUrl`.
- Connection-level RTT or cwnd. These are per path only.
- A retransmitted-packets counter.
- `path_stats(PathId)` on iroh's `Connection`. Use `Path::stats()`.
- Relay latency reporting without the `unstable-net-report` feature.
