# 08: Early acks on a lost or reopened connection

**What to build:** Early acks stay safe when the connection misbehaves. On a lost connection (`FriendPipeline::connection_lost`, called from the connection manager's cleanup), the friend's pipeline state is cleared and no early acks are made for the rest of that Game session; nothing is replayed across a reconnect. When a stream is reopened while the connection lives, the ordered writer (`run_ordered_writer` in `frame_sink.rs`, which today reopens once and retries the failing frame) asks the friend pipeline for its in-flight frames and replays them first, in order, then the frame it was retrying, and the friend's JACKAL drops any seq it already has.

**Blocked by:** 07 (not 10)

**Status:** ready-for-agent

- [ ] Tests: no early acks after a loss, for the rest of the Game session
- [ ] Test: a reset stream replays in order and delivers each message once, using `MemoryFrameSink` (it fails chosen opens or writes by index), so no fault injection from ticket 10 is needed
- [ ] Existing lost-connection behaviour is unchanged

## Comments

- 2026-10-18: The reset-stream test uses the in-memory frame sink from [transport-message-path](../../archive/transport-message-path/spec.md), so this ticket no longer needs ticket 10's fault injection. Replay is the writer asking the friend pipeline for in-flight frames on reopen. The writer's outbox currently carries `(bytes, capture_id)`, and the sink has no replay hook, so this ticket adds the pipeline query to the writer.
