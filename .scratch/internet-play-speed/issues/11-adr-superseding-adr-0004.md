# 11: The ADR that supersedes ADR-0004

Type: grilling
Status: resolved
Blocked by: 06, 07, 09, 10

## Question

[What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md) decided to supersede [ADR-0004](../../../docs/adr/0004-internet-play-speed-in-0-2-0.md), not amend it. Its premise (bytes) and its step order are contradicted by the data. Write the new ADR's decision:

- **The reworded constraint:** the friend's game receives exactly the bytes sent, in order. The Helper never changes a game message. It may answer for the friend's JACKAL with acks and by dropping redundant resends, keeps every message until the friend's real ack, and stops answering for the rest of the Game session once a friend's connection is lost. The [brief's](../analysis/turn-sync-fixes.md) section 3 has a draft.
- **Early acks are for two-player games only.** [How does the Helper ack for the friend's game?](09-helper-acks-for-the-friends-game.md) found the brief's rule 3 unsafe with three players. Record why, and the rule that would make three-player games safe.
- **What ships besides the fix:** the traffic capture (off by default) and `DATALINK_RELAY_ONLY`, both from that ticket.
- **The DLL's timing change,** which crosses "invisible to the DLL".
- **Which release carries what:**
  - the Helper's acks, opt-in, then on by default
  - the DLL wake-up
  - step 1's reporting, from [What does the Helper report…](07-what-the-helper-reports.md)
  - step 4's page activity, at low priority, since players don't look at the Helper mid-game
  - and the fact that no minor release is needed unless compression returns
- **What's dropped and what's conditional:** delta encoding, our own relay, `dwLatency`, and compression after a Relayed capture.
- **ADR-0004's status line,** and the `CONTEXT.md` terms this introduces, if any.

## Answer

2026-10-06. Decided with the maintainer: all six proposals accepted.

Written as [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md). ADR-0004 is now "Superseded by ADR-0005". `CONTEXT.md` gains **Real ack**.

- **Constraint:** the friend's game gets exactly the bytes sent, in order. The Helper may ack and drop redundant resends, and keeps every message until the real ack. It stops answering for the rest of the Game session on a lost connection. The DLL's timing change is a named exception.
- **Early acks:** two-player only, with the reason and the safe three-player rule recorded.
- **Releases, all 0.1.x:** Patch A is measurement (badge, logs, capture, `DATALINK_RELAY_ONLY`). Patch B is opt-in Early acks and fast wake, and page activity rides with it or later. Patch C turns them on by default after validation and at least 3 tester games. No minor release unless compression returns.
- **Dropped:** delta encoding, our own relay, `dwLatency`. **Conditional:** compression after a Relayed capture.
