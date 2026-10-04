# 02: Transport: versioned ALPN, distinguishable dial errors, dial timeout

**What to build:** Two Helpers on different Peer protocol versions refuse each other at connect, and the Helper that dialled can tell why a dial failed. Today every dial failure is flattened into one string and an unreachable host takes about thirty seconds to report.

Scope, from the spec's "Transport changes" section (ADR-0002 section 1):

- **Versioned ALPN.** The ALPN is `datalink/<Peer protocol version>`, built from the one Peer protocol version constant, which is reset to 1. The dead, unused protocol version constant is removed. The stream preamble check stays as a second line of defence.
- **Distinguishable dial errors.** A dial that fails because the other side rejected the ALPN returns a Peer-protocol-mismatch error. A dial that times out or cannot connect returns a can't-reach error.
- **Dial timeout.** 15 seconds, applied inside the Transport's dial, so every caller gets it: the CLI `join`, the DLL-driven join, and later the UI join.
- **Options for construction.** A way to build a Transport with a non-default Peer protocol version and dial timeout. Production code uses the defaults. Tests use the options to create a mismatched peer and a short timeout.

The new ALPN deliberately stops datalink-mp from talking to smac-iroh builds.

Tests go beside the existing mesh networking integration tests and follow their conventions: real Transports on loopback, a polling helper with a deadline in place of sleeps, and an early return when a Transport cannot be created (sandboxed environments).

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] The ALPN is derived from the single Peer protocol version constant, whose value is 1
- [x] The unused protocol version constant is gone
- [x] Two Transports with different Peer protocol versions refuse each other, and the dialler gets the Peer-protocol-mismatch error, not the can't-reach error
- [x] A dial to a dead Ticket fails with the can't-reach error at the configured timeout
- [x] The default dial timeout is 15 seconds and applies to every dial path, including the join requested by the DLL
- [x] A Transport can be constructed with a non-default Peer protocol version and dial timeout; the default constructor's behaviour is otherwise unchanged
- [x] The existing end-to-end ordered-delivery test and the rest of the suite pass unchanged, apart from the ALPN constant

## Comments

Implemented on `main` in `cce2d33` (review fixes in `7997345`).

API for later tickets: `TransportOptions { peer_protocol_version, dial_timeout }`, `Transport::with_options`, `TransportError::PeerProtocolMismatch`, `TransportError::CantReach`. An ALPN rejection is recognised from the close error's TLS alert code (120), not from its text.

Found in review, not fixed here:

- The DLL's IPC read timeout is 5 seconds (`crates/dplayx/src/ipc_client.rs`), shorter than the 15-second dial timeout, so on a DLL-driven join to an unreachable host the DLL stops waiting before the Helper returns. Ticket 12 should decide what to do about it.
- `connect_by_ticket` reuses an existing connection without dialling, so a join over a connection whose other side vanished without closing gets neither the timeout nor `CantReach`.
- The steps of a join after the dial (`send_and_receive`) have no timeout.
- The timeout is tested through `join_session_by_ticket` and `connect_to_peer`. The mesh and reconnect paths share the same dial but have no test of their own.
