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

**Status:** resolved

- [x] In-process: a second link doesn't replace the current one, and frames keep going to the first
- [x] In-process: a spare ending changes nothing; directed sends still route, and the registry gives no re-dial answer
- [x] In-process: the current link ending with a spare open moves frames to the spare, with no loss answer and nothing added to the inbox
- [x] Each link's pipeline hears "connection lost" at most once, when that link stops being current
- [x] `cargo test` passes, including the wiring tests from 02

## Comments

**2026-10-19:** Built on branch `friend-registry-03`, commit `5a30db7`.

- Each friend record keeps its spares, oldest first, beside the current link. A later link, dialled or accepted, becomes a spare. The 01 rule where a dialled link replaced the current one is gone.
- A spare ending is removed. The current link ending with a spare open promotes the oldest spare, and the old pipeline hears "connection lost". There is no `lost` line, SESSIONLOST or re-dial. With no spare, the loss is handled as ticket 02 built it.
- "Connection lost at most once" holds by construction. It is called only on the link taken out of the current slot, and a link never goes back to being a spare. The pipeline hook is still empty, so no test can observe it.
- Disconnecting a friend now hands back every link's handle, spares included, so the connection manager closes them all. `drop_connection_to` in the wiring tests therefore still forces a real loss and re-dial. `disconnect_all` includes spares too.
- In-process tests in `friend_registry.rs` cover a second link, a spare ending, hand-over (a Joiner and the host), and disconnect. No new wiring test was added. The mesh-networking suite passed 8 runs in a row.
