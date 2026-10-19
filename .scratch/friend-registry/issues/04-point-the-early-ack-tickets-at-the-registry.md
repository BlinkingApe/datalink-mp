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

**Status:** ready-for-agent

- [ ] Tickets 07, 08, 10 and 17 each carry a dated comment naming the registry and the points above
- [ ] No ticket description still names the removed send functions or the routes map as where to build
- [ ] The effort is archived and the link check passes
