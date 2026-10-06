# ADR-0004: Internet play speed is the focus of 0.2.0

- **Status:** Superseded by [ADR-0005](0005-internet-play-speed-helpers-answer-acks.md) (2026-10-06). The captured data showed per-message round trips, not bytes.
- **Related:** [ADR-0001](0001-web-ui-frontend.md) (the page), [ADR-0002](0002-releases-versioning-pipeline-trust.md) (versioning, the release gate).

The first game over the internet (Linux and Windows on different networks, 2026-10-01) connected and played, but syncing between turns was very slow. **0.1.0 ships unchanged**, through its release gate. **Speed is the focus of 0.2.0**, and the work happens between Helpers, invisible to the game and the DLL. In this order:

1. **Measure first.** The Helper reports whether each connection is direct or relayed, and the traffic per friend: message count and size, per turn. Without these, a slow relay can't be told apart from a large payload, and the next steps can't be judged.
2. **Compress game messages** between Helpers. The game's sync data is large blocks of game state, which should compress well.
3. **Send only what changed.** Encode a message as its difference from the previous message of the same kind, but only if the measurements show large repeated payloads that compression doesn't already shrink.
4. **Show activity on the page**, such as "Receiving turn data from your friend… 340 KB" or "Waiting for your friend", so a player knows the game is working while it syncs. Saying whose turn it is would mean reading the game's own messages, and it's only done if they can be read reliably. The "sync" box in the game is the game's own and stays as it is.

## Considered options

- **Speed work in 0.1.0.** Rejected. Steps 2 and 3 change the Peer protocol, so both players need them. Doing them now would reset the gate and delay a first release that works.
- **Patching the game to send less** (through smac-fixes or Thinker). Rejected for now. The game decides what to sync, and changing that touches game logic and risks desyncs. Work between Helpers can't desync the game, because the game gets back exactly the bytes it sent.
- **Running our own relay.** Deferred until step 1 shows the relay is the bottleneck.

## Consequences

- Steps 2 and 3 change the Peer protocol version, so they ship in a minor release (ADR-0002): 0.1.x and 0.2.x players can't play together, and the Peer protocol mismatch banner tells them so.
- Steps 1 and 4 touch only the Helper, so they can ship earlier, in a 0.1.x patch release.
