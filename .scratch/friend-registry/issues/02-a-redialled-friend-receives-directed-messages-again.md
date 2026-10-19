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

**Status:** ready-for-agent

- [ ] In-process:
  - losing the host's link puts SESSIONLOST in the inbox and forgets the host
  - losing a friend with players answers "re-dial with this ticket", and directed sends to its players fail
  - losing a discovery-only friend answers nothing and forgets them
- [ ] In-process:
  - a new link for a friend being re-dialled carries Hello as its first frame
  - after that, directed sends to the friend's players arrive on the new outbox
  - after "re-dial gave up", they fail
- [ ] Transport wiring test, extending "friend lost just before a new Game session is still re-dialled": after the re-dial, the host's game sends to the Joiner's player directly and the Joiner's game receives it
- [ ] Transport wiring test with three Helpers (a host and two Joiners in one Game session): the Joiner-to-Joiner connection is dropped and re-dialled, then each Joiner's game sends to the other's player directly and it arrives
- [ ] The existing tests pass unchanged: ordered delivery, closed-session messages, a Joiner leaving, a Joiner whose host's connection is lost, and the re-dial test
- [ ] Capture lines are unchanged: `lost`, `reconnect`, `send_failed`
