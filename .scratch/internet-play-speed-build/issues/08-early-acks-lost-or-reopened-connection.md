# 08: Early acks on a lost or reopened connection

**What to build:** Early acks stay safe when the connection misbehaves. On a lost connection, the friend's state is cleared and no early acks are made for the rest of that Game session; nothing is replayed across a reconnect. When a stream is reopened while the connection lives, the writer replays the in-flight frames first, in order, then the frame it was retrying, and the friend's JACKAL drops any seq it already has.

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] Tests: no early acks after a loss, for the rest of the Game session
- [ ] Test: a reset stream replays in order and delivers each message once
- [ ] Existing lost-connection behaviour is unchanged

## Comments

- 2026-10-18: The reset-stream test uses the in-memory frame sink from [transport-message-path](../../transport-message-path/spec.md), so this ticket no longer needs ticket 10's fault injection. Replay is the writer asking the friend pipeline for in-flight frames on reopen.
