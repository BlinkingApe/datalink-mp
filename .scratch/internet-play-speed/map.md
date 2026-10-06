# Map: Internet play speed

Label: wayfinder:map

## Destination

A decided plan for the speed work in [ADR-0004](../../docs/adr/0004-internet-play-speed-in-0-2-0.md), handed to `/to-tickets`. It says what the Helper reports about each connection and its traffic (step 1), which fixes ship for a slow **Turn sync** and how they work, and what the page shows while one runs (step 4). Every fix's expected gain must be justified by data measured in a real internet game. Building is a separate effort.

## Notes

- Domain: `CONTEXT.md` (Helper, DLL, Direct connection, Relayed connection, Peer protocol version, **Turn sync**), [ADR-0004](../../docs/adr/0004-internet-play-speed-in-0-2-0.md) and [ADR-0002](../../docs/adr/0002-releases-versioning-pipeline-trust.md) (a Peer protocol change means a minor release). `docs/ARCHITECTURE.md` covers the transport's one ordered stream per friend and the game's own JACKAL reliability layer.
- Tracker: local markdown, this directory.
- Skills for grilling tickets: `grilling` + `domain-modeling`.
- **Override: one piece of doing.** The map may build a dev-only traffic capture (off by default, behind an environment variable) and play one captured game, because steps 2 and 3 can't be judged without real payloads. This isn't the shipped step 1 feature, and nothing else in this map is built.
- Raw captures are the maintainer's game state and can be large. They live in `captures/` here, which is gitignored. Only analysis summaries are committed.
- Reference setup: the maintainer's own Windows and Linux machines, one on the mobile-data hotspot. Add a game with a friend abroad if one can be scheduled, since connection type matters and the hotspot may be a Direct connection.

### Decided while charting

- **The plan may go beyond ADR-0004's four steps.** The ADR assumes the slowness is bytes (big payloads or a slow relay). If the data shows something else, such as a latency-bound Turn sync where the game waits on its own acks, the map records it and amends or supersedes ADR-0004. Its hard constraint stays: the work is between Helpers, and the game gets back exactly the bytes it sent.
- **Capture on one side only.** In a two-player game, one Helper sees every message in both directions. The Linux machine runs a local build with the capture, and the Windows machine stays on stock `v0.1.0` (the Peer protocol is unchanged, so they still play together).
- **The capture times both ends of each inbound message:** when it arrives from the friend, and when the DLL hands it to the game. That separates network latency, Helper queueing and the game's own pacing. The analysis also looks for repeated identical payloads, a sign of JACKAL retransmits.
- **The numeric target is set after the baseline,** not before.
- **Term: Turn sync**, added to `CONTEXT.md`. How the Helper detects one (an idle gap, or reading message types) is for the analysis to find.

## Decisions so far

<!-- one line per closed ticket -->

- [How does a Helper tell a Direct from a Relayed connection, and what traffic stats does Iroh give?](issues/01-iroh-connection-type-and-stats.md): `path_events()` pushes path changes, and a connection is Direct when its selected path is an IP path. `Path::stats()` gives RTT, bytes, losses and congestion window per path; there's no retransmit counter. Our N0 setup doesn't force relaying.
- [What is publicly known about SMAC's network layer and its Turn sync?](issues/02-what-is-known-about-jackal-and-turn-sync.md): nothing public, so it was read from `terranx.exe`. JACKAL is stop-and-wait (one message in flight, every recipient must ack), and a Turn sync is lockstep barriers plus small messages (≤2 KB). Likely latency-bound, not byte-bound. The first retransmit timeout is our DLL's hardcoded `dwLatency = 50` ms, so slow links get spurious whole-message resends. The header is readable: kind, sequence number, type, faction, turn.
- [A dev-only traffic capture in the Helper](issues/03-dev-only-traffic-capture.md): built on branch `capture/traffic-capture`. `DATALINK_CAPTURE=<dir>` on the Linux Helper writes JSON Lines with every game message's payload and JACKAL fields, its queued, written, arrived and drained times, and each connection's path changes and per-second stats. The format is in `docs/traffic-capture.md`. It also records session settings, system messages handed to the game, failed sends and lost connections, for two questions in other efforts.

## Not yet specified

- **The design of each fix the diagnosis picks.** Depending on the answer to [What makes a Turn sync slow, and what do we fix?](issues/06-what-makes-turn-sync-slow.md), this is some of:
  - **For latency, now the likelier lever** (from the JACKAL research):
    - shortening the round trip: a Direct connection more often, or a nearer relay of our own
    - stopping spurious retransmits: a realistic `dwLatency` from `GetCaps`, or the Helper dropping a resend whose original is still in flight
    - the Helper's queueing, and the DLL's one-request-at-a-time drain
  - **For bytes, likely small gains on ≤2 KB messages:** compression (algorithm, framing, whether small messages skip it, the Peer protocol version bump), and delta encoding against the previous message of the same kind.
  - Changing `dwLatency` is a DLL change, and the game sees it, though not a byte of its messages. That would cross ADR-0004's line that the work is invisible to the game and the DLL, so it's to be weighed there, not assumed.
- **Which release carries what.** ADR-0004 puts steps 1 and 4 in a `0.1.x` patch and Peer protocol changes in `0.2.0`. That holds unless the fixes chosen change the picture, for example a Helper-only latency fix that could also ship in `0.1.x`.
- **Amending ADR-0004**, if the diagnosis contradicts its ordered steps.

## Out of scope

- **Patching the game to send less** (through smac-fixes or Thinker). ADR-0004 rejects it: changing what the game syncs risks desyncs.
- **Building the shipped features.** This map ends at a plan; `/to-tickets` and the build come after.
