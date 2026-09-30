# Can the transport be restarted in-process, and are the error states observable?

Type: research
Status: resolved
Blocked by:

## Question

For the web UI's Stop button and status panel, the Helper must drive sessions from a long-lived process. Establish against the code in `crates/iroh-transport` and `crates/smac-helper`:

1. Can `Transport` (which has no shutdown/`Drop` path today, only `close_session`) be torn down and recreated in-process, so that the UI can switch host → idle → join without restarting? If not, what would it take, versus the accepted fallback of the Helper re-exec'ing itself?
2. Can the Helper observe, and report to a status API:
   - whether the DLL is currently connected on the IPC port
   - the connected peers (count, endpoint IDs)
   - a peer protocol version mismatch (today only logged at `connection.rs:682`)
   - "can't reach host" when dialling a ticket
   - "IPC port in use" at bind time
   - an IPC version mismatch from the DLL handshake
3. What is the right entry point for validating a Ticket without dialling (`iroh_transport::ticket::parse`?).
4. Does the current single-threaded, blocking `run_tcp_server` accept loop need restructuring to coexist with an HTTP server in the same process?

## Research

Findings: branch `research/transport-restart`, file `docs/research/transport-restart.md`.

## Comments

### Resolution (2026-09-29)

**In-process restart is feasible and recommended over re-exec.** Full findings: branch `research/transport-restart` (commit `3e0fb12`), `docs/research/transport-restart.md`, with file:line citations; verified with a throwaway probe binary and the mesh test.

1. **Teardown:**
   - Multiple Transports already coexist in one process, and there's no global state. A plain drop works but skips the QUIC close, so peers only notice via timeout.
   - Needed:
     - a `Transport::shutdown()` of about 15 lines (`disconnect_all` + `endpoint.close()`, ≤3 s)
     - a rule that the last `Arc<Transport>` is never dropped on an async thread, because it panics and release builds use `panic = "abort"`
     - a controller that swaps the Transport and is looked up per IPC request
   - Estimate: about 1–1.5 days in-process vs about 0.5 day for re-exec.
   - **Re-exec is worse:** the DLL never reconnects after an IPC error (`globals.rs:34-85`), so a re-exec would kill the running game's multiplayer.
   - "Idle" = a fresh Transport. **Each new Transport has a new key, so the host's Ticket changes after Stop** (`runtime.rs:97`).
2. **Observability** (one shared status struct written at existing sites):
   - **DLL connected:** `main.rs:180/184/238`.
   - **Peers:** add `Transport::connected_peers()` over `connection.rs:373`.
   - **Peer protocol version mismatch:** record at `connection.rs:676/681`. **Gap:** the version preamble goes only with the first ordered message, so mismatched builds likely fail first with a decode error during discovery or join. The fix is to send the preamble on connection open (same bytes). `protocol::PROTOCOL_VERSION` is dead code, and the ALPN `dplay-iroh/1` isn't tied to the version.
   - **Can't reach host:** the dial has no app timeout (30 s QUIC default). Needs a "Joining" state, `spawn_blocking`, and optionally a shorter timeout; also record the DLL-driven join error at `main.rs:254`.
   - **IPC port in use:** bind once at startup; on `AddrInUse`, set an error status instead of exiting.
   - **IPC version mismatch:** record at `main.rs:230`. Treat a decode failure on the first message as a likely mismatch.
3. **Ticket validation:** `iroh_transport::Ticket::parse` (`ticket.rs:69`), with no I/O. It doesn't trim whitespace, so **trim first**. Cheap extras: reject our own Ticket, and warn when it carries no addresses.
4. **Process structure:**
   - `main` is sync. Each Transport owns a private 2-worker runtime used via `block_on`.
   - axum gets its own runtime owned by `main`. The IPC loop moves to a plain thread, bound once, and fetches the current Transport per request.
   - All blocking Transport calls, and any drop, go inside `spawn_blocking` (the probe confirmed a nested-runtime panic otherwise).
