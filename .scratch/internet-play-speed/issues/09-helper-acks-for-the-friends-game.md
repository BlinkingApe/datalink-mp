# 09: How does the Helper ack for the friend's game?

Type: grilling
Status: needs-triage
Blocked by: 06

## Question

[What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md) chose the Helper acking for the friend's game as the main fix: when its game sends a reliable message and the frame is on the friend's stream, the Helper hands its game the ack the friend's game would send. [The decision brief](../analysis/turn-sync-fixes.md), section 2A, sketches it, with rules 1–7. Settle the design in enough detail to hand to `/to-tickets`:

- **The rules, made concrete:**
  - keeping messages until the friend's real ack, and replaying them after a reconnect
  - no acks for a friend whose connection is down
  - the hold when recipients change in a game of three or more
  - the cap on messages in flight, and its number
  - dropping a resend whose original is still in flight on the same stream
  - which acks are swallowed, given setup messages go to player 0 and the setup kind `0x12`/`0x14` exchange is left alone
- **Where in the Helper it lives,** against the reconnect driver and the existing loopback queue.
- **The opt-in switch** for its first 0.1.x patch: what the player or tester sets (a setting, an environment variable), and what's logged. Also what turns it on by default in a later patch.
- **Capture lines** for made-up and swallowed acks, and the "one-way hop" bucket in `analysis/analyse_capture.py` that measures the target: ≤ 1 s network time per Turn sync Direct, ≤ 1.5 s Relayed, ≤ 2 s at the game start.
- **Validation before release:**
  - the three-instance Wine test
  - a fault-injection test that drops the connection mid-resync
  - a captured Direct and a captured **Relayed** game, which needs a dev switch that forces relaying
  - the compression condition: whether bytes still add ≥ 1 s on a Relayed resync turn or the game start
- **The checksum baseline:** how short a look at why two of five checksums mismatched on stock 0.1.0 is enough to tell a new desync from the existing ones.
