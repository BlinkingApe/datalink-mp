# Research: Transport restart, status observability, and web UI coexistence

Supports ADR-0001 (embedded local web UI). This covers what the Helper has to change so the UI can go host → idle → join without a restart, report status, and run an HTTP server next to the IPC listener.

Sources: this repo at commit `326647a`, plus the locked dependency sources `iroh 1.0.2`, `noq-proto 1.0.1`, `iroh-base 1.0.2` and `tokio 1.53.0` (from `Cargo.lock`, read under `~/.cargo/registry/src`). The empirical results come from a scratch probe binary kept outside the repo. It links `iroh-transport` by path, and its results are quoted below. No production code was changed.

## TL;DR

1. **Restart in-process: yes. It is cheap, but it needs a teardown method.** Several `Transport`s already coexist in one process: the mesh tests do it, and the probe dropped one and created two more. Nothing in `iroh-transport` is global. What's missing is a graceful `Endpoint::close()` and a rule about *where* the last `Arc<Transport>` gets dropped. Effort is about 1 to 1.5 days including a controller. Re-exec is about 0.5 day, but it has a worse user-visible cost: it breaks a running game's DLL until the game restarts (see 1.5).
2. **Status: everything asked for is observable.** Each item needs a small hook: a shared `Status` struct written at 5 or 6 existing sites, plus one `Transport` accessor for peers. There's one real gap. A Peer protocol version mismatch is only detected lazily, so the detection point needs a small change (see 2.3).
3. **Ticket validation:** use `iroh_transport::Ticket::parse` (`crates/iroh-transport/src/ticket.rs:69`). The handler should trim the input first.
4. **IPC loop:** it doesn't need to become async. Move it to a `std::thread` and have it look up the *current* Transport on each request. Run axum on a **separate, UI-owned tokio runtime**. Today the only runtime is private to each `Transport` and dies with it. Two tokio panics apply here, and `panic = "abort"` makes them fatal in release. The rule that follows: **every blocking `Transport` call and every Transport drop from HTTP code goes through `spawn_blocking`.**

---

## 1. Can `Transport` be torn down and recreated in-process?

### 1.1 What a Transport owns today

- `Transport` owns its own multi-thread tokio `Runtime` with 2 workers (`crates/iroh-transport/src/runtime.rs:39`, `:68-72`), an `Arc<Endpoint>` (`:41`), a `ConnectionManager`, a `SessionManager` and message queues (`:43-53`).
- `Transport::new` generates a fresh `SecretKey` each time (`runtime.rs:97`). That means **every new Transport has a new endpoint ID and therefore a new Ticket.**
- Background tasks all run on that private runtime:
  - The accept loop (`runtime.rs:136-153`). It ends when `endpoint.accept()` yields `None`, which iroh documents as happening after `Endpoint::close` (`iroh-1.0.2/src/endpoint.rs:1160-1161`).
  - A per-connection handler (`connection.rs:503-574`). It ends when the connection closes (`:534-537`, `:561-564`).
  - Per-stream readers (`connection.rs:521`, `:548`).
  - A per-peer ordered writer (`connection.rs:70`, `:100-161`). It ends when the peer's outbox sender is dropped.
  - The reconnect driver (`runtime.rs:788-804`). It makes up to 5 attempts with 0.5 s to 2.5 s sleeps.
- **There is no `Drop` impl and no shutdown/close method.** `close_session` only broadcasts `SessionClosed` and clears session state. It deliberately leaves connections alone (`runtime.rs:343-361`, comment at `:352-358`).
- There are no statics, `Lazy` or `OnceCell` values in `crates/iroh-transport/src` (grep). Multiple instances per process are already exercised by `crates/iroh-transport/tests/mesh_networking.rs` (see e.g. `:15-18`). `test_join_and_ordered_delivery_end_to_end` passes locally.

### 1.2 What happens if you simply drop it today (probe results)

| Probe step | Result |
|---|---|
| `Transport::new()` | ~20 ms |
| `drop(t1)` on a plain thread | ~9 ms. iroh logs `ERROR Endpoint dropped without calling Endpoint::close. Aborting ungracefully.` (`iroh-1.0.2/src/socket.rs:220-229`) |
| Create two new Transports afterwards and connect one to the other | OK, 39 ms |
| Dial the dropped Transport's Ticket | `Err(Connection("timed out"))` after **30.0 s** |
| Call `connect_to_peer` from inside an async task of another runtime | **panic**: `Cannot start a runtime from within a runtime` |
| Same call via `tokio::task::spawn_blocking` | OK |
| Drop a `Transport` inside an async task | **panic**: `Cannot drop a runtime in a context where blocking is not allowed` (`tokio-1.53.0/src/runtime/blocking/shutdown.rs:51-52`) |

So a plain drop already "works" functionally. The problem is that it's ungraceful. Remote peers don't get a QUIC close, so they only notice via idle timeout. iroh says this makes peers "assume connections to have failed" (`endpoint.rs:1690-1693`).

### 1.3 What a proper teardown needs

1. **`Transport::shutdown(&self)`**, a new method of about 15 lines:
   - Optionally broadcast `SessionClosed`, which is what `close_session` already does.
   - Call `self.connection_manager.disconnect_all()` (`connection.rs:359-365`) with an app close code.
   - Call `self.runtime.block_on(self.endpoint.close())`. iroh closes every connection and waits for peers to acknowledge. That's "3 seconds … in cases of bad connectivity", and usually much faster (`iroh-1.0.2/src/endpoint.rs:1671-1705`).
   - After close, `accept()` returns `None`, so the accept loop exits. Connection handlers exit on connection close. The cleanup at `connection.rs:570-573` drops the peers, which ends the writer tasks. A pending reconnect driver fails fast with `EndpointClosed` (`endpoint.rs:1096-1098`).
   - One caveat: `SessionClosed` is only *enqueued* to the writer tasks (`connection.rs:79-83`). `endpoint.close()` may close the connection before that frame goes out. If the goodbye matters, give it a short flush window (for example 100–200 ms) before closing.
2. **Runtime disposal.** `Runtime::drop` blocks until all spawned work stops, and "waits forever" for `spawn_blocking` work (`tokio-1.53.0/src/runtime/runtime.rs:39-44`, `:502-518`). Once the endpoint is closed nothing should be pending, so drop is prompt. That matches the 9 ms measured above. If you want a hard bound, add `pub fn shutdown(self)` that consumes the Transport and calls `self.runtime.shutdown_timeout(..)` (`runtime.rs:454`). But `main.rs` wraps Transport in `Arc` (`crates/smac-helper/src/main.rs:122`, `:150`), so that would need `Arc::try_unwrap`. `shutdown(&self)` plus an ordinary drop is simpler.
3. **Where the last `Arc<Transport>` is dropped.** It must never happen on an async worker thread, or it panics (table above). The release profile sets `panic = "abort"` (`Cargo.toml:51`), so that panic kills the whole Helper. Drop it in `spawn_blocking`, on the IPC `std::thread`, or on a dedicated thread.
4. **A controller** in `smac-helper`, holding something like `RwLock<Arc<Transport>>` plus the status struct (see section 2). The IPC loop must fetch the current Transport **per request**, not once per connection. Today `run_tcp_server` and `handle_client` capture one `Arc<Transport>` for their whole lifetime (`main.rs:171`, `:181`, `:195`, `:218`).

### 1.4 Simplification: "idle" is just a fresh Transport

`run_host` and `run_join` are identical except that join calls `transport.connect_to_peer(&ticket)` (`main.rs:161-164`). Hosting a session isn't done by the Helper at all. The **DLL** does it through the `CreateSession` IPC request (`main.rs:244`), and joining a session is the DLL's `JoinSessionByTicket` (`main.rs:251`). So the controller can always hold a live Transport:

- **idle / Host:** a fresh Transport. Show `our_ticket()`.
- **Join:** the same Transport, plus `connect_to_peer(ticket)`, run in `spawn_blocking`.
- **Stop:** `shutdown()` the current Transport, swap in a fresh one, and drop the old one off-async.

A "soft reset" that reuses the Transport (`close_session` + `disconnect_all` + clearing queues) is *not* recommended. It would have to reset `message_queue`, `message_rx` backlog, `createplayerorgroup_sent` and `SessionManager` state (`runtime.rs:47-53`), and hold off the reconnect driver (`runtime.rs:784-805`) that re-dials dropped peers. A fresh Transport gives the same result for free.

Side effect: after Stop, the host has a **new Ticket**, because the key is regenerated at `runtime.rs:97`. That's acceptable. If a stable Ticket is wanted, add `Transport::with_secret_key(SecretKey)` and keep the key in the controller.

### 1.5 In-process restart vs re-exec

The DLL opens the IPC connection once and never drops it on I/O errors. `IPC_CLIENT` is set once (`crates/dplayx/src/globals.rs:34-75`) and cleared only on `DLL_PROCESS_DETACH` (`globals.rs:83-85`, `crates/dplayx/src/lib.rs:134-137`). The DLL's copies of the handshake's `endpoint_id` and `our_ticket` are only logged (`globals.rs:53-54`).

| | In-process swap | Re-exec |
|---|---|---|
| IPC TCP connection to a running game | **survives** if the listener and connection stay up and only the Transport behind them is swapped | **severed**. The DLL keeps a dead socket until the game is restarted |
| Stale handshake data in the DLL | harmless (only logged) | n/a |
| UI | same URL and token; the page keeps polling | the new process must rebind the UI port and hand over the token (state file or env), and the page must re-auth/reload. The port races with the dying parent |
| Windows console | unchanged | the child inherits the console; the parent must exit only after spawning |
| Effort | about 1–1.5 days: `shutdown()` (0.25 d), controller + per-request lookup + idle handling (0.5 d), tests (a create/close/recreate/dial test modelled on `mesh_networking.rs`) (0.25–0.5 d) | about 0.5 day, plus the single-instance/state-file work the ADR needs anyway |

**Recommendation: in-process swap.** The pieces all exist, and re-exec has a real UX cost for anyone who presses Stop with the game open. Keep re-exec only as an emergency fallback, for example if `shutdown()` is found to hang.

One more thing for in-process Stop while a game is mid-session: the game still thinks it's in a session. Consider queueing a `DPSYS_SESSIONLOST` (`create_session_lost_msg`, `runtime.rs:961`) for the DLL on the fresh Transport, or just document "quit to menu first".

---

## 2. Can the Helper observe and report each status item?

Proposed mechanism: a `HelperStatus` (for example `Arc<Mutex<..>>` or atomics) owned by the controller. The IPC thread, the join path and bind code write to it, and `GET /api/status` reads it. Only non-blocking reads are made on the Transport.

### 2.1 Whether the DLL is connected on the IPC port

- **Now:** the accept loop logs `Client connected` and `Client disconnected` (`main.rs:180`, `:184`) around the blocking `handle_client` call. It serves one DLL at a time (`main.rs:176-190`).
- **Minimal change:** set `dll_connected = true` (and the peer addr) at `main.rs:180`, and `false` at `:184` (use a guard so the error path clears it too). Also set `dll_handshake_ok = true` at the `HandshakeOk` arm (`main.rs:238`). "TCP accepted" and "handshake passed" are different states, and the UI should show "connected" only after the handshake.
- Note: when a second game instance connects, it waits in the listen backlog until the first disconnects. That's fine for status purposes.

### 2.2 Connected peers (count and endpoint IDs)

- **Now:** `ConnectionManager::peer_count()` and `get_connected_peers()` exist (`connection.rs:368-380`), backed by the `peers` map (`:202`). But `Transport.connection_manager` is private (`runtime.rs:43`) and has no accessor.
- **Minimal change:** add `pub fn connected_peers(&self) -> Vec<EndpointId>` to `Transport`, delegating to `connection_manager.get_connected_peers()`. It's a plain `RwLock` read with no `block_on`, so it's safe to call directly from an async handler. For richer display, `session_manager().get_players()` (`session.rs:483`) gives DPIDs, names and `node_id` (`session.rs:120-132`). `is_host()` and `in_session()` (`session.rs:238-243`) give the idle/hosting/joined state.
- Caveat: `peers` holds every live QUIC connection with our ALPN, including peers connected only for discovery. The label should say "connected Helpers", not "players".

### 2.3 Peer protocol version mismatch

- **Now:** there are three places, and none of them report outward.
  - The ordered-stream reader checks the preamble. It logs `BAD STREAM MAGIC` or `PROTOCOL VERSION MISMATCH` and returns `TransportError::ProtocolMismatch` (`connection.rs:675-684`). The caller only `warn!`s it (`connection.rs:522-531`). The connection is **not** closed.
  - **Detection is lazy.** The writer opens the ordered stream, and sends the preamble, only when the first ordered message is enqueued (`connection.rs:94`, `:122-133`). Session discovery and join use **bidirectional** request/response streams (`send_and_receive`, `connection.rs:467-488`; `handle_bi_stream`, `:894-907`), which carry **no version preamble**. Between mismatched builds, the first failure is therefore likely a postcard decode error (`TransportError::Serialization`, `protocol.rs:265-267`) during `EnumSessions` or `JoinSessionByTicket`, *before* any preamble is checked.
  - The ALPN `dplay-iroh/1` (`crates/iroh-transport/src/lib.rs:21`) is not tied to the version. `iroh_transport::protocol::PROTOCOL_VERSION = 2` (`protocol.rs:14`) is **unused** anywhere. The only live Peer protocol version is `STREAM_PROTO_VERSION = 2` (`connection.rs:31`).
- **Minimal change, with no wire format change:**
  1. Add a status sink to `ConnectionManager`, for example `Arc<Mutex<Option<PeerVersionMismatch { peer: EndpointId, theirs: u16, ours: u16 }>>>`, or a `tokio::sync::watch`. Set it at `connection.rs:676` and `:681`, and expose it through `Transport`.
  2. **Make detection eager.** Have `ordered_writer_task` open the stream (and write the preamble) immediately when spawned, not on the first message (`connection.rs:122-133`). The bytes on the wire are the same, just sent earlier. Every connection then validates versions within one RTT of connecting, before any bi-stream request.
  3. Optional: treat a `Serialization` error from `send_and_receive` as "possibly version mismatch" in the UI text.
- Optionally, close the connection on mismatch, for example `connection.close(<code>, b"version mismatch")`. The other side then sees a clear reason instead of silently half-working.

### 2.4 "Can't reach host" when dialling a Ticket

- **Now:**
  - CLI join: `transport.connect_to_peer(&host_ticket)?` (`main.rs:163`) blocks, then exits the process on error.
  - DLL-driven join: `JoinSessionByTicket` returns `IpcResponse::Error { "Failed to join session: …" }` (`main.rs:251-257`).
  - The dial is `endpoint.connect(addr, DPLAY_ALPN)` with **no application timeout** (`connection.rs:294-297`).
  - Measured against an unreachable (closed) peer: `Err(Connection("timed out"))` after **30.0 s**. That's the QUIC default `max_idle_timeout` of 30 000 ms (`noq-proto-1.0.1/src/config/transport.rs:556`). iroh does not override it (`iroh-1.0.2/src/endpoint/quic.rs:151-162`).
  - All dial errors flatten to `TransportError::Connection(String)` (`crates/iroh-transport/src/lib.rs:67-83`).
- **Minimal change:**
  - Run `connect_to_peer` in `spawn_blocking`. Set status `Joining` before the call, and `Joined` or `Error(CannotReachHost(msg))` after it.
  - Record the error in the `JoinSessionByTicket` arm at `main.rs:254` too.
  - Optionally wrap the dial in a `tokio::time::timeout` (for example 15 s) inside `Transport::connect_to_peer` for snappier feedback.
  - Parse the ticket first (section 3), so "invalid ticket" and "can't reach" are never confused.

### 2.5 "IPC port in use" at bind

- **Now:** `TcpListener::bind(&addr).context("Failed to bind TCP listener")?` (`main.rs:173`). It's fatal, and it only happens *after* the Transport was created and (in join mode) after dialling (`main.rs:121-134`, `:149-167`).
- **Minimal change:** in UI mode, bind the IPC listener **once at startup**, independent of host/join/stop, and keep it for the process lifetime. On `Err(e)` with `e.kind() == std::io::ErrorKind::AddrInUse`, don't exit. Set status `Error(IpcPortInUse(port))` and keep serving the UI; retrying on user request is optional. Most likely cause: another Helper is already running. The ADR's single-instance mechanism should catch that first.

### 2.6 IPC version mismatch from the DLL handshake

- **Now:** `handle_request` compares `protocol_version` with `ipc_protocol::PROTOCOL_VERSION` (`= 3`, `crates/ipc-protocol/src/lib.rs:18`). On mismatch it returns `IpcResponse::Error` (`main.rs:229-237`), without logging on the Helper side. The DLL logs it and drops the client (`crates/dplayx/src/ipc_client.rs:77-80`), then retries with backoff up to 30 s (`globals.rs:23`, `:64-68`). So the Helper sees a new connection plus an error every ≤30 s.
- **Minimal change:** at `main.rs:230`, record `Error(IpcVersionMismatch { dll: got, helper: PROTOCOL_VERSION })` in status. Also treat a `decode_request` failure on the **first** message of a connection (`main.rs:212`, logged at `:182`) as a likely IPC version mismatch, because a DLL with a different framing never reaches the version check. Clear the error when a later handshake succeeds.

---

## 3. Validating a Ticket without dialling

- **Entry point:** `iroh_transport::Ticket::parse(&str) -> Result<Ticket, TicketError>` (`crates/iroh-transport/src/ticket.rs:69-86`). It's re-exported at the crate root (`lib.rs:10`, `:16`) and also implemented as `FromStr` (`ticket.rs:95-101`). It does no I/O. `ConnectionManager::parse_ticket` is a private wrapper around it (`connection.rs:275-279`), so call `Ticket::parse` directly.
- Errors: `MissingPrefix`, `InvalidBase32`, `InvalidPostcard` (`ticket.rs:15-22`). Probe: `"hello"` → `MissingPrefix`, `"smac!!"` → `InvalidBase32`, `"smacaaaa"` → `InvalidPostcard`.
- **It does not trim.** In the probe, a leading space gave `MissingPrefix` and a trailing `\n` gave `InvalidBase32`. Pasted text often has these, so the `/api/validate-ticket` (or `/api/join`) handler should `.trim()` first.
- Useful extra checks after parsing, all without dialling:
  - `ticket.addr().id == transport.endpoint_id()`. The user pasted their own Ticket; reject it.
  - `ticket.addr().is_empty()` means the Ticket has only an endpoint ID with no relay or direct addresses (`iroh-base-1.0.2/src/endpoint_addr.rs:131-134`). It can still be dialled via address lookup, so warn rather than reject.

---

## 4. Does `run_tcp_server` need restructuring to coexist with an HTTP server?

### 4.1 How the runtime is owned now

- `main` is a plain synchronous `fn main()` (`main.rs:95`). It creates no tokio runtime.
- The only runtime lives **inside `Transport`** (`runtime.rs:39`, `:68-72`). It's private, and its handle isn't exposed. Every blocking Transport API wraps `self.runtime.block_on(..)` (e.g. `runtime.rs:194`, `:321`, `:846`, `:887`, `:895`).
- `run_tcp_server` runs on the **main thread**. It's a blocking `std::net::TcpListener` loop that serves one client at a time and calls Transport methods synchronously (`main.rs:171-193`, `:195-225`).

### 4.2 Consequences for adding axum

- axum can't use Transport's runtime. That runtime isn't exposed, and it **dies on every Stop/restart**. The HTTP server therefore needs its **own runtime**, owned by `main` and outliving all Transports. Either `#[tokio::main]` or an explicit `Builder` works; `current_thread` is plenty.
- **The IPC accept loop does not need to be rewritten as async.** It should:
  1. move off the main thread onto a `std::thread::spawn` (main now runs the UI runtime);
  2. take a controller handle instead of a fixed `Arc<Transport>`, and fetch the current Transport per request (see 1.3). While a swap is in progress it can block briefly or return `IpcResponse::Error`. The DLL's socket timeout is 5 s (`ipc_client.rs:47-49`), longer than `endpoint.close()`'s typical worst case of about 3 s;
  3. write the status hooks from section 2.
  
  Because it's a plain OS thread, its synchronous `block_on` calls into Transport stay legal, just as they are today.
- **Hazards for HTTP handlers**, both confirmed by the probe:
  - Calling any blocking `Transport` method (`connect_to_peer`, `join_session_by_ticket`, `enum_sessions`, `shutdown`) directly in an async handler panics with `Cannot start a runtime from within a runtime` (`tokio-1.53.0/src/runtime/context/runtime.rs:69`). Use `tokio::task::spawn_blocking`, which the probe verified works.
  - Dropping the last `Arc<Transport>` in an async handler panics with `Cannot drop a runtime…`. Do the Stop swap and the drop inside `spawn_blocking`.
  - With `panic = "abort"` in `[profile.release]` (`Cargo.toml:51`), either panic kills the Helper, including the DLL's IPC link.
  - Non-blocking accessors (`our_ticket`, `endpoint_id`, `session_manager().*`, and the proposed `connected_peers()`) are plain lock reads and are safe to call from async code.
- `Transport` is `Send + Sync`. The probe moved an `Arc<Transport>` into `spawn_blocking` and it compiled. Sharing it across the IPC thread and HTTP handlers is fine. HTTP should stick to read-only accessors, plus the join and stop commands, so it doesn't race the DLL's `Receive` draining (`runtime.rs:659-698`).
- Dependencies: `smac-helper` already pulls tokio with `net`, `io-util`, `fs`, `process` (`crates/smac-helper/Cargo.toml`), so adding axum brings no new runtime family.

### 4.3 Suggested shape

```
main (UI tokio runtime, axum on 127.0.0.1:<ui-port>)
 ├─ Controller { transport: RwLock<Arc<Transport>>, status: Mutex<HelperStatus> }
 ├─ std::thread: IPC listener (bound once at startup) → per request: controller.current()
 └─ handlers: status (non-blocking reads) | join/stop (spawn_blocking)
CLI host/join: unchanged, or thin wrappers over the same Controller without the UI.
```

## Appendix: probe

The probe was a throwaway binary in the session scratchpad and is not committed. It linked `iroh-transport` by path and did the following:

- created and dropped a Transport, then created two more and connected them;
- dialled the dropped Transport's Ticket;
- parsed malformed Tickets;
- called `connect_to_peer` and dropped a `Transport` both from async tasks and from `spawn_blocking` on a second runtime.

All figures in section 1.2 come from one run on Linux with `tokio 1.53.0` and `iroh 1.0.2`.
