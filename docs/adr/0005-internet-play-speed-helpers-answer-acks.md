# ADR-0005: Internet play speed: Helpers answer JACKAL's acks

- **Status:** Accepted (2026-10-06)
- **Supersedes:** [ADR-0004](0004-internet-play-speed-in-0-2-0.md)
- **Related:** [ADR-0002](0002-releases-versioning-pipeline-trust.md) (versioning, the release gate). Decision trail: `.scratch/internet-play-speed/map.md`. Evidence: `.scratch/internet-play-speed/analysis/turn-sync-fixes.md`.

## Context

ADR-0004 assumed a slow **Turn sync** was bytes: big payloads or a slow relay. A captured internet game (a Direct connection, five turns) showed otherwise:

- 67–78% of a Turn sync is the game computing, with nothing in flight. No Helper fix touches it.
- Of the network share, almost all is per-message round trips. JACKAL, the game's own reliability layer, is stop-and-wait: one message in flight, and every recipient must ack it. Bytes matter only at the game start and on resyncs.
- Payloads compress to 9–16%, but delta encoding would save about 0.05 s.

So ADR-0004's premise and its step order (compress, then delta-encode) don't hold, and its constraint ("invisible to the game and the DLL") has to be reworded.

## Decision

### 1. The constraint

The friend's game receives exactly the bytes the game sent, in the order sent. The Helper never changes a game message. It may answer for the friend's JACKAL, with acks and by dropping redundant resends, because the stream already guarantees delivery. In return it keeps every message until the friend's game has sent its **real ack**, and it stops answering for the rest of the Game session once a friend's connection is lost.

The DLL's timing change (section 3) is a named exception to "invisible to the DLL". It changes when the game hears about a message, not what it hears, and needs no IPC or Peer protocol change.

### 2. Early acks (the main fix)

When the game sends a reliable message and its frame is on the friend's stream, the Helper hands the game an **Early ack** at once, instead of waiting a round trip. The friend's game still acks it, and the Helper swallows that real ack.

- **Two-player games only.** With three or more players the Helper behaves exactly as 0.1.0. The earlier "hold when recipients change" rule isn't safe: if the host broadcasts M1 then M2 to friends X and Y, X can receive M2 and message Y before M1 reaches Y, though the recipients never changed. The safe rule, for a later effort if three-player speed is wanted: early-ack only while every message in flight and the new one go to the same single friend, and otherwise hold the new message itself until the earlier ones have real acks. The gain would be small, because most three-player Turn sync traffic is host broadcasts.
- **The rules** (cap of 16 in flight, swallowing only matching real acks, dropping resends the connection already carried, clearing on a lost connection, replay on a reopened stream) are settled in `.scratch/internet-play-speed/issues/09-helper-acks-for-the-friends-game.md`, and they are the spec to build from.
- **Helper-only,** with no Peer protocol change. Nearly all the gain comes when only the host has it.
- **Target,** on the reference setup: network time per Turn sync ≤ 1 s Direct and ≤ 1.5 s Relayed, and the game start ≤ 2 s.

### 3. The DLL wakes the game at once

The DLL signals the game's event straight after each `Send`, and polls the Helper every 1 ms instead of 10. It's behind its own switch, `DATALINK_FAST_WAKE=1`. Details and the CPU budget (under 1% of a core, adaptive poll as the fallback) are in `.scratch/internet-play-speed/issues/10-dll-wakes-the-game-at-once.md`. A blocking IPC wait is rejected, since it would change the IPC version and force a minor release.

### 4. What ships, and when

All of it is a 0.1.x patch. No minor release is needed unless compression returns.

- **Patch A, measurement.** Helper-only and low-risk:
  - the Direct/Relayed badge on each friend's row ("Direct" / "Relayed (slower)"), the only figure for players
  - always-on info logging: a line per path change, and a line per Turn sync and game start
  - the dev traffic capture (off by default, `DATALINK_CAPTURE`)
  - `DATALINK_RELAY_ONLY`, undocumented for players, so a tester abroad can capture a Relayed game
- **Patch B, the fixes, opt-in.** `DATALINK_EARLY_ACKS=1` on the Helper and `DATALINK_FAST_WAKE=1` for the DLL, each testable alone. Early acks change the reliability semantics the game has relied on, and a bug there shows as a desync or a hung game, so they start off.
- **Patch C, on by default.** Both switches flip to on (and become off switches) once the six validation steps pass and at least 3 real tester games with them on, one with Windows hosting, show no hang, no `SEND MESSAGE TIME EXPIRED` and no delivery fault.
- **Page activity** ("Syncing turn N", whose game is working, and the tab title after about 10 s) is low priority, because players don't look at the Helper mid-game. It ships with Patch B or later and never holds up the fixes.

### 5. Dropped and conditional

- **Dropped:** delta encoding (about 0.05 s), our own relay or chasing Direct connections (under 1 s per Turn sync once Early acks exist), and a realistic `dwLatency` (the game would see it, and Early acks make it redundant).
- **Conditional: compression.** It returns as a new effort, with a minor release for the Peer protocol change, only if a Relayed capture taken after Early acks shows bytes still adding ≥ 1 s to a resync turn or the game start.
- **Known and not addressed:** the game's own compute share, and why two of five checksums mismatched on stock 0.1.0. Early acks are judged by a delivery check instead.

## Considered options

- **Amend ADR-0004.** Rejected: its premise and order are contradicted, and its central constraint changes meaning.
- **Early acks on by default from the first patch.** Rejected: see Patch B.
- **Early acks in games of three or more.** Rejected for now: unsafe without holding messages, and the gain is small.
- **Patching the game to send less, or running our own relay.** Still rejected, as in ADR-0004.

## Consequences

- Players on any 0.1.x can still play together. A Helper that lacks Early acks interoperates with one that has them.
- A bug in Early acks could hang or desync a game, which is why Patch B ships opt-in and Patch C waits on real games.
