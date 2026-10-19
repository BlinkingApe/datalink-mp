# 02: A re-dialled friend receives directed messages again

**What to build:** When a friend's connection is lost and the Helper re-dials them, the game's directed messages reach that friend again once the new connection is up. Today cleanup removes the friend's routes and the re-dial never restores them, so a Send to the friend's player most likely fails with "player not found" for the rest of the game. This bites Joiner-to-Joiner connections in games of three or more.

The loss policy moves into the friend registry. When a friend's link ends, the old link's pipeline hears "connection lost". The caller passes the roster facts (was this friend the host, the friend's ticket from the roster), and the registry decides:

- **the host:** write the capture's `lost` line, add "session lost" to the game inbox, and forget the friend
- **a friend with players and a ticket:**
  - write the `lost` line
  - keep the record with no current link, so directed sends to its players fail as today while it is re-dialled
  - answer "re-dial with this ticket"
- **anyone else** (a discovery-only friend): write the `lost` line and forget them

The re-dial driver stays in the connection manager: the same five attempts, backoff and `reconnect` capture lines, now started from the registry's answer.

- **Re-attach:** when a link opens for a friend whose record has no current link, it becomes current. If a local player exists, a Hello is queued on it before any other frame. The friend's DPIDs are still on the record, so directed sends route again. The friend's Helper re-attaches the incoming link to its own retained record the same way.
- **The re-dial gave up:** the registry hears it and forgets the friend. Game-session-sync ticket 05 may later make this event tell the game.
- **Disconnecting:**
  - Disconnecting one friend closes its link, and the loss is handled when the link ends, as today. The test of a Joiner being told its session was lost relies on this.
  - Disconnect-all at shutdown is unchanged.
- **Dialling** checks "is this friend connected" (has a current link), not "has a record".

See the spec: [Links, loss and re-dial](../spec.md#links-loss-and-re-dial).

**Blocked by:** 01

**Status:** resolved

- [x] In-process:
  - losing the host's link puts SESSIONLOST in the inbox and forgets the host
  - losing a friend with players answers "re-dial with this ticket", and directed sends to its players fail
  - losing a discovery-only friend answers nothing and forgets them
- [x] In-process:
  - a new link for a friend being re-dialled carries Hello as its first frame
  - after that, directed sends to the friend's players arrive on the new outbox
  - after "re-dial gave up", they fail
- [x] Transport wiring test, extending "friend lost just before a new Game session is still re-dialled": after the re-dial, the host's game sends to the Joiner's player directly and the Joiner's game receives it
- [x] Transport wiring test with three Helpers (a host and two Joiners in one Game session): the Joiner-to-Joiner connection is dropped and re-dialled, then each Joiner's game sends to the other's player directly and it arrives
- [x] The existing tests pass unchanged: ordered delivery, closed-session messages, a Joiner leaving, a Joiner whose host's connection is lost, and the re-dial test
- [x] Capture lines are unchanged: `lost`, `reconnect`, `send_failed`

## Comments

**2026-10-19:** Built on branch `friend-registry-02`, commit `db9d9da`.

- The registry owns the loss policy:
  - `link_ended(friend, link, RosterFacts)` returns `LinkEnded::{Nothing, Redial { ticket }}`. The caller passes the roster's facts: `was_host`, and the ticket from `get_player_ticket_by_node`.
  - The registry now writes the `lost` line and the SESSIONLOST.
  - `act_on_lost_connection` is gone. The reader's cleanup acts on the answer with `spawn_redial`: the same five attempts, backoff and `reconnect` lines, then `redial_gave_up` if every attempt fails.
- Re-attach: `link_opened` now takes `LocalFacts` (our player and ticket). A link opening onto a record with no link gets our Hello as its first frame, queued before the link becomes visible.
  - `establish_mesh_connection` now routes before it dials and no longer sends its own Hello. That drops the old `Hello { player_id: 0 }` sent when there was no local player.
- `disconnect` keeps the record, so the loss is handled when the link ends. `disconnect_all` still forgets everyone. A link of a friend with no record still writes the `lost` line, and SESSIONLOST for the host, but is never re-dialled.
- A link that is not the friend's current one ending now changes nothing. Ticket 03 builds its spares on this.
- In-process tests in `friend_registry.rs` cover loss (host, re-dialled, discovery-only, disconnected), re-attach (dialled and incoming) and giving up.
  - The two Transport wiring tests are new tests in `tests/mesh_networking.rs`, next to the existing re-dial test. That test is left unchanged.
  - Both new tests fail on ticket 01's code.
