# 17: Release Patch B (opt-in)

**What to build:** Release the fixes as a 0.1.x patch with both switches off by default: `DATALINK_EARLY_ACKS=1` (Helper) and `DATALINK_FAST_WAKE=1` (DLL), each testable alone, plus the page activity. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 4. Release notes say how testers turn them on and what to send back (the log, and a capture when asked). They also carry this line from [transport-message-path ticket 02](../../archive/transport-message-path/issues/02-act-on-a-lost-connection-when-it-happens.md): the reconnect to a friend now starts as soon as the loss is seen, and no longer gets lost if you click Host Game or Join Game just after the drop.

**Blocked by:** 12, 13, 14, 15

**Status:** ready-for-agent

- [ ] Version bumped as a patch (no IPC or Peer protocol change) and notes written
- [ ] Both switches default off
- [ ] Release notes carry the earlier-reconnect line
- [ ] The release gate passes
