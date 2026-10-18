# 05: A friend pipeline per friend, per Transport

**What to build:** Each connected friend has one **friend pipeline**, held on the connection manager's entry for that friend and created and dropped with it. It sees:

- each outbound game message to the friend, as it is queued
- each inbound game message from the friend, as it is read
- "Game session began" and "connection lost"

For an outbound message it returns the outcome and the message's capture id; for an inbound message it returns the outcome. In this ticket the outcome is always "send it" or "deliver it". Early ack ticket 07 adds dropping resends, swallowing real acks and handing the game Early acks.

**What sits behind its interface:**
- the JACKAL decoder from 04
- the friend's Turn sync detector, unchanged
- the capture lines for game messages in and out, in one fixed order

Time and RTT are injected: from the friend's connection in production, from closures in tests.

**Locking:** the pipeline never takes another module's lock. Its caller acts on the result after releasing the pipeline's lock.

**What goes:** the process-wide Turn sync map and its register/unregister glue. The capture's file stays process-wide. Path tracking is untouched. There is no snapshot interface yet; tickets 09 and 12 add one.

See the spec, [The friend pipeline](../spec.md#the-friend-pipeline).

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] In-process pipeline tests: outcomes are "send" or "deliver", and capture ids pass through
- [ ] Replaying the recorded Turn sync fixtures through the pipeline gives the same reports as through the detector directly
- [ ] Two Transports in one test binary keep separate Turn sync state for the same friend
- [ ] The Turn sync log lines and the capture lines are unchanged
- [ ] No lock is held across an await or while taking another module's lock; Send stays a synchronous enqueue (ADR-0001 §2)
- [ ] `cargo test` passes, including the end-to-end tests with real endpoints
