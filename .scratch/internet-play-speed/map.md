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
- [Play a captured internet game](issues/04-play-a-captured-internet-game.md): five turns played, one player at a time, Linux hosting on the hotspot. Captured in `captures/capture-1791300326-1036127.jsonl`; it's complete and readable. The whole game ran on a **Direct** IPv6 connection (RTT median 68 ms, p90 124 ms), so no Relayed game has been captured yet. Turn-end clock times were noted, but not which turns felt slow.
- [Analyse the capture](issues/05-analyse-the-capture.md): a Turn sync is mostly **the game computing**: 67–78% of four Turn syncs (6–25 s) had nothing in flight. Round trips took 19–29% (one message in flight, ack = QUIC RTT + 5–8 ms), the DLL's 10 ms poll 1–3%, and Helper queueing nil. Bytes matter only on the game start and on resyncs after a checksum mismatch (110 KB in the slowest Turn sync). Payloads compress to 9–16%. 70% of the host's messages were spuriously resent. A Turn sync is found by message type (`0x8301`/`0x4301` … `0x4309`). The summary and its script are in `analysis/`.
- [What makes a Turn sync slow, and what do we fix?](issues/06-what-makes-turn-sync-slow.md): per-message round trips, not bytes. The fix is **the Helper acking for the friend's game** (Helper-only, 13–15% of a normal Turn sync and 27% of a resync turn; opt-in in its first 0.1.x patch) plus the DLL waking the game at once. Delta encoding, our own relay and `dwLatency` are dropped; compression waits on a Relayed capture after the fix. The target is ≤ 1 s network time per Direct Turn sync (1.5 s Relayed). ADR-0004 is superseded. The [decision brief](analysis/turn-sync-fixes.md) has the replay model behind the numbers.
- [What does the page show while a Turn sync runs?](issues/08-activity-on-the-page-during-turn-sync.md): low priority, since players don't look at the Helper mid-game. It's "Syncing turn N" and whose game is working on the friend's row, never "Waiting for your friend". Elapsed seconds always, bytes only on a resync, and the tab title only after about 10 s. The prototype is on branch `prototype/turn-sync-activity`.
- [How does the Helper ack for the friend's game?](issues/09-helper-acks-for-the-friends-game.md): **Early acks** in two-player games only, since the brief's rule 3 isn't safe with three players. They're made at enqueue, swallow matching real acks, drop resends the connection already carried, and cap at 16 in flight (ack withheld at the cap). They stop for the session on a lost connection, and a reopened stream replays. They live in a new `early_ack.rs`, switched on by `DATALINK_EARLY_ACKS=1`. The capture ships with new lines, a hop bucket and a delivery check. That check stands in for explaining the existing checksum mismatches. Six validation steps, including `dev-faults` and a shipped `DATALINK_RELAY_ONLY`.
- [What does the Helper report about each connection, to whom, and where?](issues/07-what-the-helper-reports.md): only Direct/Relayed is for the player, as plain text on each friend's row ("Direct" / "Relayed (slower)"). The rest is always-on info logging: a line per path change, and a line per Turn sync and game start with raw counts, bytes, duration, idle and hop time, RTT and Early ack counts. One Turn sync detector shared with the page activity. Ships in the same 0.1.x patch as Early acks.
- [How does the DLL wake the game at once?](issues/10-dll-wakes-the-game-at-once.md): same 0.1.x patch as Early acks, behind its own `DATALINK_FAST_WAKE=1` (DLL env var, default off). It signals the game's event after each `Send` and polls at 1 ms instead of 10; no IPC change. CPU budget is under 1% of a core, with an adaptive poll as the fallback. The Windows 15.6 ms tick is measured during the build. A blocking IPC wait is rejected (it would bump the IPC version).
- [The ADR that supersedes ADR-0004](issues/11-adr-superseding-adr-0004.md): written as ADR-0005. Constraint reworded (DLL timing a named exception), Early acks two-player only. Patch A measurement, Patch B opt-in fixes, Patch C on by default; no minor release unless compression returns. Delta encoding, own relay and `dwLatency` dropped.

## Not yet specified

- **The game's compute share** (67–78% of a Turn sync), which no Helper fix touches. The Linux host under Wine was the slow machine in three of four Turn syncs. A game with Windows hosting would show whether that's Wine. It may belong to another effort, since patching the game is out of scope here.
- **Why two of five checksums mismatched on stock 0.1.0.** Early acks no longer need the answer: [How does the Helper ack…](issues/09-helper-acks-for-the-friends-game.md) judges them by a delivery check instead. It may belong with the compute share, in another effort.

## Out of scope

- **Patching the game to send less** (through smac-fixes or Thinker). ADR-0004 rejects it: changing what the game syncs risks desyncs.
- **Designing compression.** [What makes a Turn sync slow…](issues/06-what-makes-turn-sync-slow.md) made it conditional on a Relayed capture taken after the Helper's acks are built (bytes still adding ≥ 1 s), and that comes after this map ends.
- **Early acks in games of three or more.** [How does the Helper ack…](issues/09-helper-acks-for-the-friends-game.md) limited them to two-player games, and records the safe rule in case three-player speed is wanted later. The gain there would be small, because most of that Turn sync traffic is host broadcasts.
- **Building the shipped features.** This map ends at a plan; `/to-tickets` and the build come after.
