# 03: A second connection to a friend no longer kills the live one

**What to build:** Two Helpers that dial each other at the same moment keep one working connection. Join and the PlayerJoined handler both dial, so two connections to the same friend do happen. Today the second dial to complete silently replaces the friend's entry, and its pipeline is dropped without hearing "connection lost". When either connection closes, cleanup removes the live one and starts a needless re-dial.

The friend registry knows which link is current:

- **The first link to a friend is current.** A later link to the same friend, from a simultaneous dial or an incoming connection, is a spare. Its reader still delivers what arrives on it.
- **A spare link ends:** it is removed. Nothing else happens: no loss, no re-dial, and the routes stay.
- **The current link ends while a spare is open:** the spare becomes current, and the old link's pipeline hears "connection lost". This is not a lost friend: there is no `lost` line, no SESSIONLOST and no re-dial.
- **The current link ends with no spare open:** the friend is lost, handled as ticket 02 built it.

This ticket is blocked by 02 only because both rewrite the registry's "link ended" handling.

See the spec: [Links, loss and re-dial](../spec.md#links-loss-and-re-dial).

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] In-process: a second link doesn't replace the current one, and frames keep going to the first
- [ ] In-process: a spare ending changes nothing; directed sends still route, and the registry gives no re-dial answer
- [ ] In-process: the current link ending with a spare open moves frames to the spare, with no loss answer and nothing added to the inbox
- [ ] Each link's pipeline hears "connection lost" at most once, when that link stops being current
- [ ] `cargo test` passes, including the wiring tests from 02
