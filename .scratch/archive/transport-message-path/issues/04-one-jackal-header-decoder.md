# 04: One JACKAL header decoder

**What to build:** The Transport has one decoder for JACKAL's header, which is the game's own reliability layer. It has named classifications:

- **carries reliable data:** any data bit, as the capture and the Turn sync detector use today
- **is exactly reliable data `0x0004`:** for Early acks
- **is exactly an ack `0x0002`:** for Early acks

The capture and the Turn sync detector both use it in place of their own parsing. Setup kinds `0x14`/`0x12` keep their current classification everywhere. The DLL's own trace parsing is out of scope. See the spec, [The friend pipeline](../spec.md#the-friend-pipeline).

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Table tests for each classification, including setup kinds `0x14`/`0x12`, kinds `0x0004` and `0x0002`, and payloads shorter than the header
- [x] The capture and the Turn sync detector have no header parsing of their own
- [x] The Turn sync detector's fixture tests pass unchanged, so the reports are identical
- [x] The capture's lines are unchanged (format 1)

## Comments

Built `crates/datalink-transport/src/jackal.rs`: `Jackal::decode` plus `carries_reliable_data`, `is_exactly_reliable_data`, `is_exactly_ack` and `carries_ack` (ack bit without data bit, which the Turn sync detector uses, so `0x12` stays an ack there). Moved the decoder and its tests out of `capture.rs`; the Turn sync detector now calls it instead of parsing kind, seq and type itself. Its fixture tests and the capture tests pass unchanged.
