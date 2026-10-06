# 06: What makes a Turn sync slow, and what do we fix?

Type: grilling
Status: resolved
Blocked by: 05

## Question

Given [the capture's analysis](05-analyse-the-capture.md): what dominates a slow Turn sync (the relay, the bytes, round trips, or Helper queueing), and which fixes does 0.2.0 pursue? Choose from ADR-0004's compression (step 2) and sending only what changed (step 3), running our own relay, a latency fix, or a mix. Choose by expected gain on the measured budget, not by the ADR's order.

The analysis found that none of these dominates. 67–78% of a Direct Turn sync is the game's own compute, and round trips plus the DLL's poll are 21–32%. So the choice is how much of that share to win, and the target should be set against that share, not the whole Turn sync. The options also include an ack that costs no round trip and the DLL's 10 ms poll, which are in the map's fog.

Also settle:

- the numeric target for a Turn sync on the reference setup, now that there's a baseline
- whether ADR-0004 needs amending or superseding

Each fix chosen graduates the map's "design of each fix" fog into its own ticket.

## Answer

2026-10-06. Decided from [the decision brief](../analysis/turn-sync-fixes.md), which replays each captured Turn sync message by message under each fix. Its model keeps the measured compute and causal order and reproduces today's Turn syncs to within 0.3 s.

**What dominates:** the game's compute (67–78%), which no Helper fix touches. Of the network share, almost all is **per-message round trips** from JACKAL's stop-and-wait. It isn't bytes, the relay or Helper queueing. The winnable share is 1.2–3.3 s of a normal Direct Turn sync, 7.8 s of a resync turn and 21 s of the game start, and it roughly doubles on a Relayed connection (projected).

**Pursued:**

1. **The Helper acks for the friend's game** (the main fix). When its game sends a reliable message and the frame is on the friend's stream, the Helper hands its game the ack the friend's game would send.
   - **Feasibility:** each of the game's 989 acks is the data message's first 12 bytes with the kind word rewritten (`analysis/check_acks.py`).
   - **Gain:** 13–15% of a normal Turn sync, 27% of a resync turn (43% Relayed, projected) and 18.8 s of the game start. That's 64–90% of the winnable share.
   - **What it touches:** the Helper only, with no Peer protocol change. It works when only the host has upgraded.
   - **Rules from the start, none optional:**
     - keep each message until the friend's real ack, and replay it after a reconnect
     - stop acking for a friend whose connection is down
     - in games of three or more, hold when the recipients change
     - cap the messages in flight
     - drop a resend whose original is still in flight on the same stream
     - swallow only matching real acks
     - an off switch, plus capture lines
   - **Release:** a 0.1.x patch, **opt-in in its first patch**. It changes reliability semantics the game has relied on, and a bug there shows as a desync or a hung game, which is worse than slowness. It turns on by default in a later patch once real games show no new desyncs. The brief recommended on by default from the start.
2. **The DLL wakes the game at once:** it signals the game's event straight after each `Send`, and polls every 1 ms for inbound messages. It's timing only and DLL-only, so a 0.1.x patch, and it adds 0.1–0.6 s on top of fix 1.

**Not pursued:**
- **A realistic `dwLatency`:** the game would see it, and fix 1 makes it redundant.
- **Delta encoding (ADR step 3):** it saves about 0.05 s.
- **Our own relay, or chasing Direct connections:** after fix 1 they'd save under 1 s per Turn sync.
- **Compression (ADR step 2)** is conditional: build it only if a Relayed capture taken after fix 1 shows bytes still add ≥ 1 s to a resync turn or the game start. Without it, the speed work needs **no Peer protocol change**.

**Target**, on the reference setup and against the winnable share, not the whole Turn sync:
- network time per Turn sync ≤ 1 s Direct and ≤ 1.5 s Relayed
- game start ≤ 2 s
- The model predicts 0.4–0.9 s.
- It's measured with a new "one-way hop" bucket in `analysis/analyse_capture.py`, since a local ack stops the round-trip bucket meaning network time.

**ADR-0004 is superseded,** not amended. Its premise (bytes) and its step order are contradicted. Its constraint is reworded to let the Helper answer the friend's JACKAL acks, keep every message until the friend's real ack, and never change a game message.

**Before fix 1 ships**, not before this decision:
- a captured **Relayed** test game, which needs a dev switch that forces relaying
- a short look at **why two of five checksums mismatched** on stock 0.1.0, so a desync after fix 1 can be told apart from the existing ones

**Noted, not pursued here:** the compute share. The Linux host under Wine was the slow machine in three of four Turn syncs; a game with Windows hosting would show whether that's Wine.

Graduated into [How does the Helper ack for the friend's game?](09-helper-acks-for-the-friends-game.md), [How does the DLL wake the game at once?](10-dll-wakes-the-game-at-once.md) and [The ADR that supersedes ADR-0004](11-adr-superseding-adr-0004.md).
