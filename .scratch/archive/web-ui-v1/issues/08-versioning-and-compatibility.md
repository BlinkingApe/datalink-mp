# Versioning and compatibility policy

Type: grilling
Status: resolved
Blocked by:

## Question

How do versions and identity behave across releases and sessions? Three linked decisions:

1. **Release version bump policy.** How does the Release version relate to the IPC version and Peer protocol version (see `CONTEXT.md`)? Who bumps what, and when? Is the Release version the sole compatibility signal users see?
2. **When the peer version check happens.** Today the version preamble is sent only with the first ordered message, so mismatched builds probably fail with a decode error first, and the mismatch banner may never fire (see ticket 01). Should the preamble be sent on connection open (same bytes)?
   - Is that a "wire change" under the map's out-of-scope line?
   - What does a new build see when it talks to an old build, and vice versa? The agent establishes these facts from the code before the user decides.
   - Should the ALPN `dplay-iroh/1` be tied to the version? The dead `protocol::PROTOCOL_VERSION` should be removed or wired up.
3. **Should the host's Ticket survive Stop?** Each new Transport generates a new key, so the Ticket changes after Stop. Keeping it requires persisting the key (a key file next to the exe?), which reopens the "no config" decision. Does keeping it matter to players, e.g. a Ticket already shared on Discord?

## Answer

Decided in a grilling session (2026-09-30).

**Facts established from the code first**
- The only version checks were the IPC handshake ([main.rs:230](../../../../crates/smac-helper/src/main.rs)) and the `SMAC`+version preamble on the ordered uni-stream ([connection.rs:676](../../../../crates/iroh-transport/src/connection.rs)). Discovery (`SessionQuery`) and `JoinRequest` travel over bi-streams as raw postcard with no preamble, so mismatched builds first meet as a decode error or a silent misread, and the mismatch banner would practically never fire.
- `protocol::PROTOCOL_VERSION` is dead code. The ALPN is a fixed `dplay-iroh/1`.
- Each Transport calls `SecretKey::generate()`, so the Ticket already changes on every launch of the Helper, not only after Stop.

**Decisions**
1. **Release version bump policy.** Semver with a compatibility rule. Pre-1.0: a change to the IPC version or the Peer protocol version means a **minor** bump; anything else is a **patch**. Players on the same `0.x` can play together. The tag `vX.Y.Z` equals the workspace version, and CI checks that they match. The Release version is the only number players compare; the IPC and Peer protocol versions are small print.
2. **Where the peer check happens.** The ALPN carries the Peer protocol version: `dplay-iroh/<STREAM_PROTO_VERSION>`, which is `dplay-iroh/2` today. Mismatched builds are rejected in the QUIC handshake, and the joiner maps that rejection to the "Peer protocol version mismatch" banner. The banner says "different release" without naming the friend's release. The stream preamble stays as a second line of defence, and dead `protocol::PROTOCOL_VERSION` is removed.
   - This is the version mechanism that the "no wire changes beyond version bumps" out-of-scope line already permits; the line is reworded to say so.
   - Consequence: the new ALPN breaks compatibility with today's `/1` builds (including upstream's until it takes the PR), so under rule 1 **the fork's first release is `0.2.0`**.
3. **The Ticket doesn't survive Stop.** No key persistence, no key file, so "no config" holds. After Stop, the UI shows the new Ticket with a short "your Ticket changed, share it again" note. A stable Ticket across Stop or launches is out of scope.
4. **No version in the Ticket.** The Ticket format is unchanged. The ALPN rejection surfaces within about a second of pressing Join, which is soon enough.

## Comments

**2026-09-30, later:** the `0.2.0` here is superseded by [Fork or standalone project?](10-fork-or-standalone.md). The first release is `0.1.0` and the ALPN is `datalink/<peer version>`, reset to 1.
