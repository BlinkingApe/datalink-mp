# 04: Point the Early ack tickets at the registry

**What to build:** The next agent picking up Early acks builds inside the friend registry instead of adding hooks. Docs only. Append a dated comment to each of these tickets in [internet-play-speed-build](../../internet-play-speed-build/issues/), and fix their descriptions where they now name code that's gone:

- **07, Early acks core:**
  - The registry carries out the pipeline's outcomes: "hand the game this Early ack" adds to the inbox, "drop this resend" queues nothing, and the inbound outcomes "swallow" and "release withheld Early acks" are handled in the same place.
  - Fix the line saying the Game session start isn't wired to the pipelines; it is.
  - The two-player fact is passed in with the Send's other facts.
  - The friend's player DPIDs are on the friend's record.
- **08, lost or reopened connection:**
  - "No Early acks for the rest of the Game session after a loss" lives in the friend record's Game-session slot, which survives a re-dial.
  - Pipelines are per link, and each hears "connection lost" once.
  - Add the criterion from the spec's candidate 5: the receiving side dispatches nothing from a reopened stream until the old stream's reader has finished, tested at whatever seam 08 settles on.
- **10, dev faults:**
  - `drop:N` is counted where Early acks are made (the pipeline, or the registry acting on its outcome).
  - Only `reset:N` wraps the frame sink.
- **17, release Patch B:** the release notes mention that directed messages reach a re-dialled friend again.

Then archive this effort per the tracker's "finished efforts" convention: `git mv` the folder under `.scratch/archive/`, fix inbound links, and run the link check.

See the spec: [Candidates 3–7, explored](../spec.md#candidates-37-explored) and [Further Notes](../spec.md#further-notes).

**Blocked by:** 01, 02, 03

**Status:** resolved

- [x] Tickets 07, 08, 10 and 17 each carry a dated comment naming the registry and the points above
- [x] No ticket description still names the removed send functions or the routes map as where to build
- [x] The effort is archived and the link check passes

## Comments

**2026-10-19:** Docs only, on branch `friend-registry-04`.

- Tickets 07, 08, 10 and 17 in [internet-play-speed-build](../../../internet-play-speed-build/issues/) each carry a dated comment that names the registry's real types and functions from `crates/datalink-transport/src/friend_registry.rs`: `FriendRegistry::send_game`, `queue_game`, `receive_game`, `link_ended`, `game_session_began`, `SendFacts`, `Record::players`, `Record::session` and `GameSessionFacts`.
- Descriptions fixed:
  - 07 no longer says `game_session_began` isn't called, or that the connection manager calls `connection_lost`. The registry acts on the outcomes, and the pipeline is per link.
  - 08 names `FriendRegistry::link_ended` as the caller of `connection_lost`, and gains candidate 5's criterion as a checklist item.
  - 10 puts only `reset:N` at the frame sink.
  - 17 gains the release-note line about directed messages reaching a re-dialled friend, with a checklist item.
- No open ticket named the removed send functions or the routes map. Only resolved ticket 05 does, and it is history.
- Ticket 08's comment also raises a gap from ticket 03. `game_session_began` reaches only current links, so a spare promoted mid-session has a pipeline that never heard the session began. Ticket 08 decides whether that matters, for example by notifying spares too.
- The effort is archived under `.scratch/archive/friend-registry/`.
