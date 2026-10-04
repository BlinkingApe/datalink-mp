# 04: Helper as a library: Session controller and IPC server under `host` and `join`

**What to build:** The Helper becomes a library with a thin binary on top, with no change in behaviour for anyone running `datalink-mp host` or `datalink-mp join --ticket`. This is the prefactor that lets the UI and the CLI drive the same session code, and it gives the Helper its first tests.

Scope, from the spec's "Modules" section:

- **Session controller.** Owns the current Transport. In this ticket its interface is: get the current Transport, and join a Ticket. Later tickets add the status snapshot, Stop and shutdown.
- **IPC server.** The existing request handling moves behind the controller. The listener is bound once at startup and runs on a plain thread for the life of the process. It asks the controller for the current Transport on every request, not once per connection, so that a later Transport swap (Stop) is picked up by a DLL connection that is already open.
- **Library entry point.** One entry point takes a configuration and returns a handle exposing the bound IPC port and a way to shut down. The configuration will grow in later tickets (Game folder path, UI port, token, browser opener); shape it so those can be added. The binary fills it from the command line; tests fill it with free ports.
- **`host` and `join`** become thin wrappers with no networking logic of their own. Their behaviour stays exactly as it is: the Ticket is the first line on standard output, the IPC port options and the `SMAC_HELPER_PORT` precedence are unchanged, a failed dial in `join` exits with an error, and a failed IPC bind is fatal. They start no HTTP server.
- Running with no subcommand keeps today's behaviour (same as `host`) until ticket 05 replaces it.

Tests:

- A **fake DLL** test helper: a TCP client that speaks the IPC protocol, modelled on the IPC protocol's round-trip unit tests. Later tickets reuse it.
- In-process tests start the library entry point on a free IPC port and drive it with the fake DLL. They assert only on IPC responses, not on the controller's internals.
- Process-level smoke tests of the built binary (the spec's seam 3) for `host` and `join`.
- Tests return early when a Transport cannot be created, as the mesh networking tests do.

**Blocked by:** 01 (Rename the Helper to `datalink-mp`)

**Status:** resolved

- [x] The Helper is a library plus a thin binary; `main` stays synchronous
- [x] The library has one entry point taking a configuration and returning a handle with the bound IPC port and a shutdown method
- [x] `host` and `join` contain no networking logic of their own and go through the Session controller
- [x] The IPC server fetches the current Transport from the controller on every request
- [x] In-process test: a fake DLL handshake on a free port gets a successful reply carrying the Helper's Ticket, and a follow-up request is answered on the same connection
- [x] In-process test: a handshake with the wrong IPC version gets an error reply
- [x] Smoke test: `host` on a free IPC port prints a parseable Ticket as its first line of standard output and answers an IPC handshake
- [x] Smoke test: `join --ticket` with text that is not a Ticket exits with an error
- [x] A failed IPC bind in `host` or `join` is still fatal
- [x] `cargo test` passes for the whole workspace

## Comments

Implemented on `main` in `84420f8` (review fixes in `b76e2ef`).

The Helper is the `datalink_mp` library (`crates/datalink-mp/src/lib.rs`, `controller.rs`, `ipc_server.rs`) with `main.rs` as the command line. `datalink_mp::start(Config)` returns a `Helper` with `ipc_port()`, `controller()`, `wait()` and `shutdown()`. `Config` has `ipc_port` (0 picks a free port) and `transport_options`. `StartError` tells a failed IPC bind (`IpcBind`) from a failed Transport (`Transport`).

**Needs a decision: `join` ordering.** The ticket says `host` and `join` behave exactly as before. The four behaviours it lists all hold, but two orderings changed, both because the listener is now bound at startup:

- `join` used to open the IPC port only after the dial succeeded. Now the port is open while the dial runs, so a DLL can handshake and then lose the Helper if the dial fails (up to 15 s later), and an open IPC port no longer means "joined". UI mode behaves this way by design; restoring the old order for the CLI alone would need a second, two-phase entry point.
- The IPC port is bound before the Transport is created. `host` or `join` on a taken port now exits at once without printing a Ticket; before, it printed the Ticket and then failed. The error text and exit code are the same.

Notes for later tickets:

- `Helper::shutdown()` shuts the Transport down and leaves the IPC listener bound, as the spec says (life of the process). In-process tests therefore leave one accept thread and one bound port behind per Helper. `Helper` has no `Drop`, because a blocking shutdown in `drop` would break the threading rules on an async thread.
- The check that the IPC server picks up a swapped Transport on an open connection cannot be tested until Stop exists (ticket 13).
- With the IPC server up during a dial, a DLL `JoinSessionByTicket` can race a dial from the CLI or the UI. `connect_by_ticket` checks for an existing connection and then dials without a guard, so a double dial to the same Helper looks possible. Not reproduced. Relevant to ticket 11.
- Test support is in `crates/datalink-mp/tests/common/mod.rs`: `FakeDll`, `poll_until`, `hold_port`, `free_port`. `tests/ipc.rs` is the seam 1 harness to extend; `tests/cli.rs` has the seam 3 smoke tests, including `test_no_subcommand_behaves_as_host`, which ticket 05 replaces.
- `helper.controller().transport().our_ticket()` in `main.rs` is the only way to read the Ticket until the status snapshot arrives in ticket 05.
