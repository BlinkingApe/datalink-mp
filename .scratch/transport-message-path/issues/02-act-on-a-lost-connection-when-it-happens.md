# 02: Act on a lost connection when it happens

**What to build:** When a friend's connection ends, the Helper acts at once, from the connection manager's cleanup on the runtime. It no longer waits for the game's next poll of the inbox. Cleanup:

- writes the capture's `lost` line
- if the lost friend was the host, queues "session lost" for the game
- otherwise starts the existing five-attempt reconnect driver, unchanged

This fixes a bug. When a Game session begins, the inbox is emptied, and today that throws away a pending connection-lost notice, together with its reconnect and its `lost` line. The trigger is a friend's connection dropping just before the player clicks Host Game or Join Game: the friend is never re-dialled. It also means the reconnect now starts as soon as the loss is seen.

The inbox stays two-stage in this ticket; [03](03-one-game-inbox.md) replaces it. See the spec, [Connection-loss policy moves to the loss](../spec.md#connection-loss-policy-moves-to-the-loss).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Draining the inbox no longer runs any connection-loss policy
- [ ] A Joiner whose host's connection is lost still gets session lost, as today
- [ ] Regression test with real endpoints (as in the Transport's mesh tests): a friend's connection is lost, the host's game starts a new Game session before polling, and the host still reconnects to the friend
- [ ] The capture's `lost` and `reconnect` lines are written as before (format 1 unchanged)
- [ ] Existing Joiner-leaving and closed-session tests pass unchanged
- [ ] A one-line note for the next release notes: the reconnect starts when the loss is seen
