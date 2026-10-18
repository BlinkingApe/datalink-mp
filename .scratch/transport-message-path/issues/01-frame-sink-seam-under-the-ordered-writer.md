# 01: A frame-sink seam under the ordered writer

**What to build:** The ordered writer for each friend writes through a **frame sink**, a seam with two adapters. The iroh adapter does what the writer does today. The in-memory adapter records preambles and frames, and can be told to fail a chosen open or write. The writer's rule doesn't change:

- open the stream lazily on the first frame, with the magic and Peer protocol version preamble
- on a failed write, reopen once and retry that frame
- if the reopen or the retry fails, close the connection and write nothing more
- mark the capture's `written` after each frame

Production behaviour is identical. The point is that ticket 08 of internet-play-speed-build can test a reset stream without ticket 10's fault injection, and ticket 10's dev-faults adapter becomes a third adapter at this seam. See the spec, [The frame sink seam](../spec.md#the-frame-sink-seam).

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] The writer reaches the stream only through the frame sink. The iroh adapter is the only one production uses.
- [x] In-process test with the in-memory sink: frames arrive in queue order after a single preamble
- [x] In-process test: one failed write causes a reopen, a second preamble and a retry of the same frame, then later frames in order
- [x] In-process tests: a failed reopen, and a failed retry, each close the connection and write nothing more
- [x] No lock is held across an await; the writer stays on the Transport's runtime (ADR-0001 §2)
- [x] `cargo test` passes, including the end-to-end ordered delivery test with real endpoints

## Comments

Built `crates/datalink-transport/src/frame_sink.rs`: a `FrameSink` trait (open with preamble, write_frame, close, finish), the writer `run_ordered_writer` holding the reopen-once rule, the `IrohFrameSink` production adapter, and a test-only `MemoryFrameSink` that records preambles and frames and fails chosen opens or writes by index. `connection.rs` now spawns the writer over the iroh sink. Five in-process tests cover order, reopen and retry, failed reopen, failed retry and failed first open. No lock is held across an await.
