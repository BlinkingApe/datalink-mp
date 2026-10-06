# 11: The ADR that supersedes ADR-0004

Type: grilling
Status: needs-triage
Blocked by: 06, 07, 09, 10

## Question

[What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md) decided to supersede [ADR-0004](../../../docs/adr/0004-internet-play-speed-in-0-2-0.md), not amend it. Its premise (bytes) and its step order are contradicted by the data. Write the new ADR's decision:

- **The reworded constraint:** the friend's game receives exactly the bytes sent, in order. The Helper never changes a game message. It may answer for the friend's JACKAL with acks and by dropping redundant resends, keeps every message until the friend's real ack, and stops answering for a friend whose connection is down. The [brief's](../analysis/turn-sync-fixes.md) section 3 has a draft.
- **The DLL's timing change,** which crosses "invisible to the DLL".
- **Which release carries what:**
  - the Helper's acks, opt-in, then on by default
  - the DLL wake-up
  - step 1's reporting, from [What does the Helper report…](07-what-the-helper-reports.md)
  - step 4's page activity, at low priority, since players don't look at the Helper mid-game
  - and the fact that no minor release is needed unless compression returns
- **What's dropped and what's conditional:** delta encoding, our own relay, `dwLatency`, and compression after a Relayed capture.
- **ADR-0004's status line,** and the `CONTEXT.md` terms this introduces, if any.
