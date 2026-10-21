# 10: dev-faults cargo feature

**What to build:** A dev-only cargo feature `dev-faults`, absent from release builds, lets a tester inject faults during a game: `DATALINK_FAULT=drop:N` closes the friend's connection after N early acks, and `reset:N` resets the stream. They are for the fault-injection validation, run during the game start's state sync. `reset:N` is a third adapter at the frame-sink seam (`FrameSink` in `crates/datalink-transport/src/frame_sink.rs`), wrapping `IrohFrameSink` beside the test-only `MemoryFrameSink`. `drop:N` is counted where Early acks are made, because the frame sink can't see them (see the comment below).

**Blocked by:** 07, 08

**Status:** resolved

- [x] Release builds don't contain the feature (checked)
- [x] `drop:N` and `reset:N` behave as described in a Transport test
- [x] They log when they fire

## Comments

- 2026-10-18: The dev-faults adapter is a third adapter at the frame-sink seam from [transport-message-path](../../archive/transport-message-path/spec.md), wrapping the iroh one.
- 2026-10-19: From [friend-registry](../../archive/friend-registry/spec.md) (candidate 5), the two faults sit in different places:
  - **`drop:N` is counted where Early acks are made.** That is the friend pipeline (`friend_pipeline.rs`), or the friend registry acting on its outcome: `queue_game` in `crates/datalink-transport/src/friend_registry.rs`, where ticket 07 matches the "give the game an Early ack" outcome. The frame sink only sees frames, so it can't count Early acks. The registry does no I/O. To close the connection, it hands the link handles back to the connection manager, as `FriendRegistry::disconnect` already does for `ConnectionManager::disconnect`. The loss is then handled when the reader's cleanup calls `link_ended`, like any other loss.
  - **Only `reset:N` wraps the frame sink:** a `FrameSink` adapter around `IrohFrameSink`.

- 2026-10-21: Resolved. `dev-faults` feature on `datalink-transport` (passed through by `datalink-mp`); `DATALINK_FAULT=drop:N,reset:N` read in `faults.rs`. `reset:N` is `ResetFrameSink` around `IrohFrameSink` (fails one write after N frames; the writer reopens and replays). `drop:N` is counted in `queue_game` after each Early ack made (released withheld acks are not counted); the registry holds the friend's handles in `take_fault_handles`, which `Transport::send` closes. Release check: the feature is in no default set, `cargo tree -e features` shows no `dev-faults`, and the release workflow never passes it.
