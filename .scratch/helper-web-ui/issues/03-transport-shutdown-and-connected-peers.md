# 03: Transport: `shutdown()` and `connected_peers()`

**What to build:** A Transport can be shut down gracefully so that the other side notices promptly, and it can report which Helpers are connected to it. Stop, Quit and the peer list on the page are all built on these two calls.

Scope, from the spec's "Transport changes" section:

- **`shutdown()`**: close every peer connection and the endpoint gracefully, bounded at 3 seconds.
- **`connected_peers()`**: the endpoint IDs of the connected Helpers. It is a plain lock read, safe to call from async code, because status requests may use only non-blocking reads.

A process must be able to create a second Transport after shutting the first one down; Stop depends on this.

Tests go beside the existing mesh networking integration tests and follow their conventions: real Transports on loopback, a polling helper with a deadline in place of sleeps, and an early return when a Transport cannot be created.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `shutdown()` returns within its 3-second bound, including when a peer is connected
- [x] A connected peer notices the close promptly after `shutdown()`
- [x] A new Transport can be created and dialled in the same process after an earlier one is shut down
- [x] `connected_peers()` lists a peer after it connects and drops it after it disconnects, on both the dialling and the accepting side
- [x] `connected_peers()` does not block on the Transport's runtime and can be called from an async context
- [x] The existing test suite passes unchanged

## Comments

Implemented on `main` in `8825d2f` (review fixes in `7997345`).

`Transport::shutdown(&self)` blocks for at most `Transport::SHUTDOWN_TIMEOUT` (3 s) and must run in `spawn_blocking` when called from async code, as must dropping the last reference. It takes about one second on loopback when a connection existed, and the full three when a peer vanished without closing. It does not send `SessionClosed`; peers see a connection loss.

Notes for later tickets:

- The test of the 3-second bound would not fail if the timeout were removed, because iroh's own close takes about as long. The bound holds by construction.
- Pre-existing race, left alone: the connection handler removes a peer's map entry by endpoint ID without checking it is the same connection, so if two Helpers dial each other at once `connected_peers()` can under-report.
